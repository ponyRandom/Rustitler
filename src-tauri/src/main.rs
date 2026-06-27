#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    match rustitler_lib::offline_smoke::run_from_env() {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }

    tauri::Builder::default()
        .setup(|app| {
            use rustitler_lib::commands::{AppState, TauriEventEmitter};
            use rustitler_lib::packaging::runtime_assets_from_resource_dir;
            use tauri::Manager;

            let app_data_dir = app.path().app_data_dir()?;
            let resource_dir = app.path().resource_dir()?;
            let state = AppState::new(
                app_data_dir,
                std::sync::Arc::new(TauriEventEmitter::new(app.handle().clone())),
            )?
            .with_runtime_assets(runtime_assets_from_resource_dir(resource_dir));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            rustitler_lib::commands::start_batch,
            rustitler_lib::commands::cancel_batch,
            rustitler_lib::commands::get_batch_state,
            rustitler_lib::commands::confirm_pending_output,
            rustitler_lib::commands::undo_batch,
            rustitler_lib::commands::list_history,
            rustitler_lib::commands::get_history_batch,
            rustitler_lib::commands::load_settings,
            rustitler_lib::commands::save_settings,
            rustitler_lib::commands::import_settings,
            rustitler_lib::commands::export_settings,
            rustitler_lib::commands::reset_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rustitler");
}
