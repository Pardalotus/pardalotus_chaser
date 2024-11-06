use sha1::{Digest, Sha1};

use rusqlite::Connection;
use time::OffsetDateTime;

pub(crate) fn get_connection() -> Option<Connection> {
    match Connection::open("./db.sqlite3") {
        Ok(connection) => Some(connection),
        Err(e) => {
            log::error!("Can't open SQLite database: {}", e);
            None
        }
    }
}

use crate::sources::MetadataSource;

pub(crate) fn init(connection: &Connection) {
    let query = "
        PRAGMA journal_mode = WAL;
        ";

    let journal_mode = connection.query_row(query, (), |row| {
        let result: String = row.get(0).expect("Pragma update;");
        Ok(result)
    });
    log::debug!("Journal mode now: {:?}", journal_mode);

    // Create the works history table.
    // This stores all new versions of metadata. There may be different versions of metadata from the same source.
    let query = "
    CREATE TABLE IF NOT EXISTS works_history (
        pk INT PRIMARY KEY,
        identifier TEXT,
        identifier_type INT,
        source INT,
        hash TEXT,
        json JSONB,
        updated INTEGER,
        UNIQUE(identifier, identifier_type, source, hash));
    ";
    match connection.execute(query, ()) {
        Err(e) => {
            log::error!("Failed to create works_history table: {}", e);
            return;
        }
        _ => {}
    }

    // Create the date values table, for storing internal settings.
    let query = "
    CREATE TABLE IF NOT EXISTS date_values (
        key TEXT PRIMARY KEY,
        value INT);
    ";
    match connection.execute(query, ()) {
        Err(e) => {
            log::error!("Failed to create date_values table: {}", e);
            return;
        }
        _ => {}
    }
}

/// Get a date from the settings table for the given key.
/// Return None if non-existent or incorrectly formatted.
pub(crate) fn get_date(connection: &Connection, key: &str) -> Option<OffsetDateTime> {
    let query: rusqlite::Result<i64> = connection.query_row_and_then(
        "SELECT value FROM date_values WHERE key = ? LIMIT 1",
        [key],
        |row| row.get(0),
    );

    let result = match query {
        rusqlite::Result::Ok(value) => match OffsetDateTime::from_unix_timestamp(value) {
            Ok(timestamp) => Some(timestamp),
            Err(err) => {
                log::info!("Failed to parse config value {:?}", err);
                None
            }
        },
        _ => None,
    };

    log::debug!("Get value {} {:?}", key, result);

    result
}

pub(crate) fn insert_work(
    connection: &Connection,
    source: MetadataSource,
    identifier: &str,
    identifier_type: u32,
    json: &str,
    updated: &OffsetDateTime,
) {
    let mut hasher = Sha1::new();
    hasher.update(json);
    let hash = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("");

    let query = "
    INSERT INTO works_history (identifier, identifier_type, source, json, updated, hash)
    VALUES (?,?,?,JSONB(?),?,?)
    ON CONFLICT (identifier, identifier_type, source, hash)
    DO NOTHING;";

    let result = connection.execute(
        query,
        (
            identifier,
            identifier_type,
            source as u32,
            json,
            updated.unix_timestamp(),
            hash,
        ),
    );

    match result {
        Err(e) => {
            log::error!("Failed to insert work: {:?}", e);
        }
        _ => {}
    }
}

pub(crate) fn set_date(connection: &Connection, key: &str, value: OffsetDateTime) {
    log::debug!("Set value {} {}", key, value);
    let query =
        "INSERT INTO date_values (key, value) VALUES (?, ?) ON CONFLICT DO UPDATE SET value = ?";
    let result = connection.execute(query, (key, value.unix_timestamp(), value.unix_timestamp()));

    match result {
        Err(e) => {
            log::error!("Failed to update date_value: {:?}", e);
        }
        _ => {}
    }
}
