//! Pull (phase-3.md 4b, decision #075): a paired desktop asks for the changes after its cursor,
//! one page at a time, to keep its encrypted copy (`storage::local`).
//!
//! Only a **paired desktop** pulls (a browser never keeps a copy, #071), and only when its
//! schema version is the server's: otherwise it reads from the server while online and is
//! told to update. The page is the person's: other people's private rows never leave
//! (`storage::sync::PullScope`); everything else of the household's comes, and the desktop's
//! screens hide what the person's role can't see with the server's own read code (#069).

use domain::device::DeviceKind;
use domain::id::DatabaseId;
use domain::log_area;
use domain::sync::{PULL_WINDOW, PullOutcome, PullPage};
use storage::Database;
use storage::sync::{PullScope, ReadPage, schema_version};

use crate::sessions::SignedIn;

/// `Ok` if `signed_in` is on a paired desktop; otherwise the refusal.
async fn paired_desktop(database: &Database, signed_in: &SignedIn) -> Result<(), PullOutcome> {
    match database.device(signed_in.device).await {
        Ok(Some(device)) if device.kind == DeviceKind::Desktop && device.public_key.is_some() => {
            Ok(())
        }
        Ok(_) => Err(PullOutcome::NotAllowed),
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "pull: can't read the device");
            Err(PullOutcome::Unavailable)
        }
    }
}

/// The page after `cursor` for `signed_in`'s desktop, whose schema version is `schema`.
pub async fn pull(
    database: &Database,
    database_id: DatabaseId,
    signed_in: &SignedIn,
    cursor: u64,
    schema: i64,
) -> PullOutcome {
    if let Err(refused) = paired_desktop(database, signed_in).await {
        return refused;
    }
    let server = schema_version();
    if schema != server {
        tracing::info!(target: log_area::SYNC, desktop = schema, server, "pull refused: another schema version");
        return PullOutcome::OtherSchema { server };
    }
    let scope = PullScope {
        user: signed_in.user,
        household: database_id,
    };
    match database.read_pull_page(&scope, cursor, PULL_WINDOW).await {
        Ok(read) => PullOutcome::Page {
            page: Box::new(page_of(read, database_id, server, cursor, signed_in)),
        },
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "pull: can't read the page");
            PullOutcome::Unavailable
        }
    }
}

/// The page to send, from what was read.
fn page_of(
    read: ReadPage,
    database_id: DatabaseId,
    schema: i64,
    cursor: u64,
    signed_in: &SignedIn,
) -> PullPage {
    if cursor > read.server_sequence {
        // A copy ahead of the server: restored from an old snapshot? Milestone 7's protective
        // mode acts on this; for now the desktop just gets nothing new.
        tracing::warn!(target: log_area::SYNC, cursor, server_sequence = read.server_sequence, "a desktop's copy is ahead of the server");
    }
    let page = PullPage {
        database_id,
        schema,
        cursor,
        next_cursor: read.next_cursor,
        server_sequence: read.server_sequence,
        tables: read.tables,
    };
    tracing::debug!(target: log_area::SYNC, device = %signed_in.device, cursor, next = page.next_cursor, rows = page.row_count(), "pull");
    page
}
