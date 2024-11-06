use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use rusqlite::{Connection, Transaction};
use time::{Duration, OffsetDateTime};

use crate::{
    crossref_api_client::{get_index_date, harvest_to_channel},
    sources::MetadataSource,
    sqlite,
};

use scholarly_identifiers;

/// Date value for checkpointing the harvest.
const CROSSREF_NB: &str = "crossref-not-before";

/// Retrieve all new Crossref data since the last run.
pub(crate) fn run_latest_harvest(connection: &mut Connection) {
    let transaction = connection.transaction().unwrap();

    // Start from most recent run, now.
    // Add 1 hour margin for jitter. This results in duplicate fetches but they are de-duplicated in the database.
    let saturating_sub = sqlite::get_date(&transaction, CROSSREF_NB)
        .unwrap_or(OffsetDateTime::now_utc())
        .saturating_sub(Duration::HOUR);
    let after = saturating_sub;

    let new_after = harvest(&after, &transaction);

    sqlite::set_date(&transaction, CROSSREF_NB, new_after);

    transaction.commit().unwrap();
}

/// Harvest data until the given date, returning the index date of the most recent.
/// If none were retrieved, the `after` date is returned, so it can be attepmted again next time.
pub(crate) fn harvest(after: &OffsetDateTime, connection: &Transaction) -> OffsetDateTime {
    let (tx, rx): (Sender<serde_json::Value>, Receiver<serde_json::Value>) = mpsc::channel();

    let after_a = *after;
    let child = thread::spawn(move || {
        harvest_to_channel(tx, &after_a);
    });

    let mut latest_date = *after;

    log::info!("Start harvest after {}", after);
    let mut count = 0;
    for item in rx {
        if let Some(indexed) = get_index_date(&item) {
            latest_date = indexed.max(latest_date);

            if let Some(doi) = &item["DOI"].as_str() {
                // Normalise and identify the type of the identifier.
                // For Crossref records, this will be the DOI type ID.
                let (identifier_val, identifier_type) =
                    scholarly_identifiers::identifiers::Identifier::parse(doi).to_id_string_pair();

                if let Ok(json) = serde_json::to_string(&item) {
                    count += 1;
                    if (count % 1000) == 0 {
                        log::info!("Done {} items.", count);
                    }

                    sqlite::insert_work(
                        connection,
                        MetadataSource::Crossref,
                        &identifier_val,
                        identifier_type,
                        &json,
                        &indexed,
                    );
                }
            }
        }
    }
    log::info!("Stop harvest, got {}, latest {}", count, latest_date);

    child.join().expect("Child join");

    latest_date
}
