use retry::delay::Exponential;
use retry::{self};
use serde::Deserialize;
use std::sync::mpsc::Sender;
use time::format_description;
use time::format_description::well_known::Iso8601;
use time::{Duration, OffsetDateTime};

const BASE: &str = "https://api.crossref.org/v1/works";

#[derive(Deserialize, Debug)]
struct CrossrefResponse {
    status: String,
    message: CrossrefResponseMessage,
}

#[derive(Deserialize, Debug)]
struct CrossrefResponseMessage {
    //  #[serde(rename = "total-results")]
    #[serde(alias = "total-results")]
    total_results: usize,

    #[serde(alias = "next-cursor")]
    next_cursor: String,

    // Leave the work model as an opaque structure, we're not concerned with the detailed internal schema.
    items: Vec<serde_json::Value>,
}

/// Get the indexed date for the work, if present and valid.
pub(crate) fn get_index_date(item: &serde_json::Value) -> Option<OffsetDateTime> {
    if let Some(value) = &item["indexed"]["date-time"].as_str() {
        match OffsetDateTime::parse(value, &Iso8601::DEFAULT) {
            Ok(time) => Some(time),
            _ => None,
        }
    } else {
        None
    }
}

/// Fetch historical data until the given [`not_before`] date.
/// Due to lack of secondary sort, it's sensible to add extra padding.
pub(crate) fn fetch(
    rows: u32,
    cursor: &str,
    from_index_date: &str,
) -> Result<(Vec<serde_json::Value>, String), String> {
    let url = format!(
        "{}?filter=from-index-date:{}&sort=indexed&order=desc&rows={}&cursor={}",
        BASE, from_index_date, rows, cursor
    );

    log::debug!("Query {}", url);
    let result = retry::retry_with_index(Exponential::from_millis(10).take(5), |try_count| {
        if try_count > 1 {
            log::info!("Retry URL {}: {}", try_count, url);
        }

        reqwest::blocking::get(&url)
    });

    match result {
        Ok(e) => {
            if e.status() == 200 {
                let json = e.json::<CrossrefResponse>();

                match json {
                    Ok(r) => {
                        if r.status != "ok" {
                            log::error!("Didn't get OK response: {}", r.status);
                            Err(r.status)
                        } else {
                            // For the first page, log the number of results.
                            if cursor.eq("*") {
                                log::info!(
                                    "Maximum potential results: {}",
                                    r.message.total_results
                                );
                            }

                            Ok((r.message.items, r.message.next_cursor))
                        }
                    }
                    Err(e) => Err(e.to_string()),
                }
            } else {
                log::error!("Bad response: {:?}", e);
                Err(String::from("Bad response"))
            }
        }
        Err(e) => {
            log::error!("Error: {:?}", e);
            Err(e.to_string())
        }
    }
}

pub(crate) fn harvest_to_channel(chan: Sender<serde_json::Value>, after: &OffsetDateTime) {
    log::debug!("Harvest to channel");

    let rows = 1000;
    let mut cursor = String::from("*");
    let mut again = true;

    let ymd_format = format_description::parse("[year]-[month]-[day]").unwrap();

    // The API only deals in time intervals of one day, so we can't request the specific cut-off time. Instead we need to truncate it to the day boundary.
    // Choose the start of the day before the requested cut-off. This means we're not asking the API to sort the entire data set.
    // It should be sufficient to use the start of the current day, but adding an extra day avoids a potential boundary condition.
    // We won't retrieve that much data, as we finish pagination when we hit the not_before date.
    let from_index_date = after
        .saturating_sub(Duration::DAY)
        .format(&ymd_format)
        .unwrap();

    while again {
        let result = fetch(rows, &cursor, &from_index_date);
        match result {
            Ok((items, new_cursor)) => {
                let num_items = items.len();

                // Stop when there are zero results, means we reached the end of the result set.
                if num_items == 0 {
                    again = false;
                }

                // Find those items indexed after the not_before date.
                let wanted_items: Vec<serde_json::Value> = items
                    .into_iter()
                    .filter(|item| {
                        if let Some(item_indexed) = get_index_date(item) {
                            item_indexed.gt(after)
                        } else {
                            false
                        }
                    })
                    .collect();

                // Stop when there are no results after the not_before date. Results are ordered by the index date, so it's safe to stop here.
                if wanted_items.is_empty() {
                    again = false;
                }

                log::debug!(
                    "Page of {}, of which {} wanted",
                    num_items,
                    wanted_items.len(),
                );

                for item in wanted_items {
                    chan.send(item).unwrap();
                }
                cursor = new_cursor;
            }
            Err(e) => {
                log::error!("Error! {:?}", e);
                again = false;
            }
        }
    }
}
