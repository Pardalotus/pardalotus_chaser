
mod crossref_api_client;
mod crossref_source;
mod sources;
mod sqlite;

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    if let Some(mut connection) = sqlite::get_connection() {
        sqlite::init(&connection);

        crossref_source::run_latest_harvest(&mut connection);
    } else {
        log::error!("Wasn't able to run due to database error.")
    }
}
