//! 3D Hoard Deck desktop shell.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod dto;
mod events;
mod protocol;

use std::sync::Arc;

use hoarddeck_app::App;
use tauri::Manager;
use tauri_specta::{Builder, collect_commands, collect_events};

/// Parallel folder reads. Network shares answer several requests at once, so a scan waits less on each one;
/// still modest, so a NAS stays usable for others meanwhile.
const SCAN_THREADS: usize = 12;

fn main() {
    let specta = specta_builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta.invoke_handler())
        .register_asynchronous_uri_scheme_protocol(protocol::SCHEME, protocol::handle)
        .setup(move |tauri_app| {
            specta.mount_events(tauri_app);
            let index_dir = hoarddeck_infra_fs::resolve_index_dir()?;
            let events = Arc::new(events::TauriEvents::new(tauri_app.handle().clone()));
            let app = App::open(&index_dir, SCAN_THREADS, events)?;
            tauri_app.manage(app.thumbnails.clone());
            let auto_rescan = Arc::clone(&app.auto_rescan);
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    if let Err(error) = auto_rescan.tick(std::time::Instant::now()) {
                        eprintln!("automatic rescan: {error}");
                    }
                }
            });
            tauri_app.manage(Arc::new(app));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start 3D Hoard Deck");
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::library::list_libraries,
            commands::library::add_library,
            commands::library::update_library_settings,
            commands::index::index_location,
            commands::index::move_index_location,
            commands::library::scan_library,
            commands::library::scan_folder,
            commands::browse::browse_folder,
            commands::browse::folder_children,
            commands::browse::search_models,
            commands::rules::set_folder_role,
            commands::browse::get_model,
            commands::browse::archive_contents,
            commands::slicers::list_slicers,
            commands::collections::list_collections,
            commands::collections::create_collection,
            commands::collections::rename_collection,
            commands::collections::delete_collection,
            commands::collections::add_to_collection,
            commands::collections::remove_from_collection,
            commands::collections::move_in_collection,
            commands::collections::collection_models,
            commands::organize::update_model_meta,
            commands::tags::list_tags,
            commands::tags::tag_facets,
            commands::tags::rename_tags,
            commands::tags::delete_tags,
            commands::tags::set_tags_group,
            commands::tags::set_tags_pinned,
            commands::tags::list_tag_groups,
            commands::tags::save_tag_group,
            commands::tags::delete_tag_group,
            commands::ai::ai_settings,
            commands::ai::save_ai_settings,
            commands::ai::test_ai_connection,
            commands::ai::suggest_tags,
            commands::ai::tag_suggestions,
            commands::ai::resolve_tag_suggestion,
            commands::ai::start_tag_suggestions,
            commands::ai::cancel_tag_suggestions,
            commands::ai::suggested_tag_counts,
            commands::ai::resolve_suggested_tag,
            commands::tag_rules::tag_rules,
            commands::tag_rules::tag_rule_presets,
            commands::tag_rules::save_tag_rules,
            commands::tag_rules::test_tag_rules,
            commands::tag_rules::count_tag_rules,
            commands::organize::rebuild_from_sidecars,
            commands::organize::import_orynt3d,
            commands::organize::find_duplicates,
            commands::organize::set_folder_cover,
            commands::organize::cancel_duplicates,
            commands::files::move_model,
            commands::files::undo_file_operation,
            commands::files::file_journal,
            commands::files::save_screenshot,
            commands::rules::folder_rules,
            commands::rules::remove_folder_rule,
            commands::rules::propose_presets,
            commands::rules::accept_preset,
            commands::slicers::save_slicers,
            commands::slicers::detect_slicers,
            commands::slicers::open_in_slicer,
        ])
        .events(collect_events![
            events::ScanProgressEvent,
            events::DuplicatesProgressEvent,
            events::SuggestProgressEvent
        ])
}

#[cfg(test)]
mod tests {
    const BINDINGS: &str = "../ui/src/ipc/bindings.ts";

    /// Fails when `ui/src/ipc/bindings.ts` is out of date; `UPDATE_BINDINGS=1` rewrites it.
    /// A changed DTO then breaks the TypeScript build, not the running app.
    #[test]
    fn export_bindings() {
        let fresh = std::env::temp_dir().join(format!("hoarddeck-bindings-{}.ts", std::process::id()));
        super::specta_builder()
            .export(specta_typescript::Typescript::default(), &fresh)
            .expect("export TypeScript bindings");
        let generated = std::fs::read_to_string(&fresh).expect("read generated bindings");
        let _ = std::fs::remove_file(&fresh);
        if std::env::var_os("UPDATE_BINDINGS").is_some() {
            std::fs::write(BINDINGS, generated).expect("write bindings");
            return;
        }
        let current = std::fs::read_to_string(BINDINGS).unwrap_or_default();
        assert!(
            current == generated,
            "{BINDINGS} is stale: run `UPDATE_BINDINGS=1 cargo test -p hoarddeck-desktop export_bindings`"
        );
    }
}
