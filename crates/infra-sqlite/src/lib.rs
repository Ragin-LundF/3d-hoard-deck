//! SQLite index adapter.

mod collections;
mod database;
mod entries;
mod export;
mod hashes;
mod journal;
mod libraries;
mod meta;
mod models;
mod queries;
mod rows;
mod schema;
mod settings;
mod suggestions;
mod tags;
mod user_data;

pub use database::SqliteIndex;
