//! The `SQLCipher` spike (phase-3.md 1e): opens an encrypted SQLite database through sqlx and
//! checks what milestone 4 relies on. Prints one line per check and exits non-zero if any
//! fails. Build with one of the features (`system-crypto` or `vendored-openssl`).

use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, Row};

/// The first 16 bytes of every plain SQLite file.
const PLAIN_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// A connection to `path`, keyed with `key` if given.
async fn open(path: &Path, key: Option<&str>, create: bool) -> Result<SqliteConnection, String> {
    let mut options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .map_err(|error| error.to_string())?
        .create_if_missing(create);
    if let Some(key) = key {
        // A passphrase, quoted as SQLCipher's PRAGMA key expects; sqlx sends `key` first.
        options = options.pragma("key", format!("'{key}'"));
    }
    SqliteConnection::connect_with(&options)
        .await
        .map_err(|error| error.to_string())
}

/// The rows' text, if the database can be read.
async fn read_notes(connection: &mut SqliteConnection) -> Result<Vec<String>, String> {
    let rows = sqlx::query("SELECT body FROM notes ORDER BY id")
        .fetch_all(connection)
        .await
        .map_err(|error| error.to_string())?;
    Ok(rows.iter().map(|row| row.get::<String, _>(0)).collect())
}

fn check(results: &mut Vec<(String, bool)>, name: &str, passed: bool, detail: &str) {
    println!("{} {name}: {detail}", if passed { "PASS" } else { "FAIL" });
    results.push((name.to_owned(), passed));
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let folder = std::env::temp_dir().join(format!("sqlcipher-spike-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    if let Err(error) = std::fs::create_dir_all(&folder) {
        println!("FAIL setup: {error}");
        std::process::exit(1);
    }
    let path = folder.join("device.sqlite");
    let mut results = Vec::new();

    // 1. Create an encrypted database and write to it.
    match open(&path, Some("first key"), true).await {
        Ok(mut connection) => {
            let version: Result<String, _> = sqlx::query_scalar("PRAGMA cipher_version")
                .fetch_one(&mut connection)
                .await;
            let version = version.unwrap_or_default();
            check(
                &mut results,
                "is SQLCipher",
                !version.is_empty(),
                &format!("cipher_version {version:?}"),
            );
            let wrote = sqlx::query(
                "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL) STRICT",
            )
            .execute(&mut connection)
            .await
            .and(
                sqlx::query("INSERT INTO notes (body) VALUES ('oil change at 120,450 mi')")
                    .execute(&mut connection)
                    .await,
            );
            check(
                &mut results,
                "create and write",
                wrote.is_ok(),
                &format!("{wrote:?}"),
            );
            let _ = connection.close().await;
        }
        Err(error) => check(&mut results, "create and write", false, &error),
    }

    // 2. The file on disk isn't plain SQLite, and the text isn't in it.
    let bytes = std::fs::read(&path).unwrap_or_default();
    let plain = bytes.starts_with(PLAIN_HEADER);
    let leaks = bytes.windows(10).any(|window| window == b"oil change");
    check(
        &mut results,
        "file is encrypted",
        !plain && !leaks && !bytes.is_empty(),
        &format!(
            "{} bytes, plain header {plain}, text visible {leaks}",
            bytes.len()
        ),
    );

    // 3. The right key reads it back.
    let read = match open(&path, Some("first key"), false).await {
        Ok(mut connection) => read_notes(&mut connection).await,
        Err(error) => Err(error),
    };
    check(
        &mut results,
        "right key reads",
        read.as_ref().is_ok_and(|notes| notes.len() == 1),
        &format!("{read:?}"),
    );

    // 4. A wrong key and no key are refused.
    for (name, key) in [
        ("wrong key refused", Some("wrong key")),
        ("no key refused", None),
    ] {
        let read = match open(&path, key, false).await {
            Ok(mut connection) => read_notes(&mut connection).await,
            Err(error) => Err(error),
        };
        check(&mut results, name, read.is_err(), &format!("{read:?}"));
    }

    // 5. Changing the key (PRAGMA rekey) keeps the data, and the old key stops working.
    let rekeyed = match open(&path, Some("first key"), false).await {
        Ok(mut connection) => sqlx::query("PRAGMA rekey = 'second key'")
            .execute(&mut connection)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Err(error) => Err(error),
    };
    let new_reads = match open(&path, Some("second key"), false).await {
        Ok(mut connection) => read_notes(&mut connection)
            .await
            .is_ok_and(|notes| notes.len() == 1),
        Err(_) => false,
    };
    let old_refused = match open(&path, Some("first key"), false).await {
        Ok(mut connection) => read_notes(&mut connection).await.is_err(),
        Err(_) => true,
    };
    check(
        &mut results,
        "rekey",
        rekeyed.is_ok() && new_reads && old_refused,
        &format!("rekey {rekeyed:?}, new key reads {new_reads}, old key refused {old_refused}"),
    );

    let _ = std::fs::remove_dir_all(&folder);
    let failed = results.iter().filter(|(_, passed)| !passed).count();
    println!(
        "{} of {} checks passed",
        results.len() - failed,
        results.len()
    );
    if failed > 0 {
        std::process::exit(1);
    }
}
