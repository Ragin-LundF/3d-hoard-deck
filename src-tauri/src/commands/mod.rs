//! Tauri commands: deserialize, call one use case, map to a DTO. Nothing else.

pub mod browse;
pub mod collections;
pub mod index;
pub mod library;
pub mod organize;
pub mod rules;
pub mod slicers;

use std::sync::Arc;

use hoarddeck_app::App;
use tauri::State;

type AppState<'a> = State<'a, Arc<App>>;
