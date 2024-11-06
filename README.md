# Pardalotus Chaser

Keep up to date with scholarly metadata. Pardalotus Chaser will keep a local
SQLite database up to date with recently added or updated scholarly metadata from
Crossref.

When you run the tool it will create or update a SQLite database. On each run it
it will retrieve data since the previous run, with a 1-hour overlap to account
for jitter.

The tool doesn't attempt to retrieve historical data, only newly updated records.

The content of the database is what was returned from the Crossref API. No
attempt is made to interpret the metadata, beyond extracting the DOI and index
date.

# How to use

You may need to install libssl:

```sh
sudo apt-get install pkg-config libssl-dev
```

To run directly from this repo:
```sh
cargo run
```

To install directly from cargo:

```sh
cargo install pardalotus_chaser
```

Then run:

```
pardalotus_chaser
```

It will create a SQLite database in the current working directory.

Because SQLite is a local database, it's not suited to concurrent access. This
tool uses SQLite's WAL (Write-Ahead Log) feature to allow you to read the
database whilst it's writing.

Nonetheless, the intended use-case is that you run the tool periodically to
update the database rather than keep it running.

If you want to keep your database continually updated, you can set a cron job to
run once an hour or so.

When you run the tool it will always retrieve at least 1 hour's worth of data.
So don't run it in a tight loop.

Feature requests welcome, [open an issue](https://github.com/Pardalotus/pardalotus_chaser/issues)!

# Structure of the database

**This code is pre-release, and the structure may change.**

The database contains a work_history table:

```sql
TABLE works_history (
        pk INT PRIMARY KEY,
        identifier TEXT,
        identifier_type INT,
        source INT,
        hash TEXT,
        json JSONB,
        updated INTEGER,
        UNIQUE(identifier, identifier_type, source, hash));
```

 - `identifier` is the DOI. There may be other work types in future.
 - `identifier_type` is 1, to indicate that it's a DOI.
 - `source` is 1 for Crossref. There may be future sources.
 - `hash` is the SHA-1 of the JSON. This is stored for future convenience. It is used to deduplicate repeated inputs.
 - `json` is a JSONB representation of the work JSON.
 - `updated` is a Unix datestamp.

# License

This code is MIT Licensed, Copyright 2024 Joe Wass.
