//! Shared setup for the server's integration tests.

#![cfg(test)]

use domain::access::{GrantTarget, Role, RoleGrant, RoleGrantPatch};
use domain::command::Change;
use domain::test_support::{ManualClock, test_id};
use domain::time::UnixMillis;
use domain::user::{User, UserPatch, Username};
use server::commands::issue;
use server::health::Health;
use server::login_limiter::LoginLimiter;
use server::passwords::hash_password;
use server::sessions::{SignedIn, sign_in};
use storage::users::{RoleGrantWriter, UserWriter};
use storage::{Database, NewDatabaseIds};

/// The tests' "now".
pub(crate) const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

/// A database with Admin `jordan` (1) and Driver `sam` (2), both signed in (Driver mode).
pub(crate) async fn setup() -> (tempfile::TempDir, Database, SignedIn, SignedIn) {
    let temp = tempfile::tempdir().unwrap();
    let ids = NewDatabaseIds {
        database_id: test_id(900),
        server_device_id: test_id(901),
    };
    let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
    let clock = ManualClock::new(NOW);
    for (n, name, role) in [(1, "jordan", Role::Admin), (2, "sam", Role::Driver)] {
        let user = User {
            username: Username::parse(name).unwrap(),
            display_name: name.into(),
            disabled: false,
            must_change_password: false,
        };
        let grant = RoleGrant {
            user: test_id(n),
            target: GrantTarget::AllAssets,
            role,
        };
        issue(
            &database,
            &UserWriter,
            Change::Create {
                id: test_id(n),
                values: UserPatch::all(user),
            },
            test_id(901),
            None,
            Health::Healthy,
            &clock,
        )
        .await
        .unwrap();
        issue(
            &database,
            &RoleGrantWriter,
            Change::Create {
                id: test_id(n + 100),
                values: RoleGrantPatch::all(grant),
            },
            test_id(901),
            None,
            Health::Healthy,
            &clock,
        )
        .await
        .unwrap();
        database
            .set_password_hash(
                test_id(n),
                &hash_password("a long password".into()).await.unwrap(),
                NOW,
            )
            .await
            .unwrap();
    }
    let limiter = LoginLimiter::new();
    let (_, jordan) = sign_in(
        &database,
        &limiter,
        "jordan",
        "a long password".into(),
        "a",
        NOW,
    )
    .await
    .unwrap();
    let (_, sam) = sign_in(
        &database,
        &limiter,
        "sam",
        "a long password".into(),
        "b",
        NOW,
    )
    .await
    .unwrap();
    (temp, database, jordan, sam)
}
