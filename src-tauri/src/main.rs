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

/// Parallel folder reads; modest because the main library sits on a network share.
const SCAN_THREADS: usize = 6;

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
            tauri_app.manage(Arc::new(App::open(&index_dir, SCAN_THREADS, events)?));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start 3D Hoard Deck");
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::list_libraries,
            commands::add_library,
            commands::scan_library,
            commands::browse_folder,
            commands::folder_children,
            commands::search_models,
            commands::set_folder_role,
            commands::get_model,
        ])
        .events(collect_events![events::ScanProgressEvent])
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
