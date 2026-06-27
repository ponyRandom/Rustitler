# Offline Packaging Audit

## Tauri Permissions

Current capability file: `src-tauri/capabilities/default.json`.

Allowed permission sets:

- `core:default`
- `core:window:default`
- `core:path:default`
- `core:event:default`

No shell, HTTP, updater, filesystem plugin, opener, or remote URL capability is enabled. The frontend uses Tauri core invoke/event/window APIs only. Drag/drop is handled through the window event API; file reads/writes happen in Rust commands after the user supplies dropped paths.

## Rust Runtime Dependencies

Direct runtime dependencies in `src-tauri/Cargo.toml`:

- `tauri`, `serde`, `serde_json`, `tokio`
- `rusqlite` with bundled SQLite
- `uuid`, `chrono`, `regex`, `sha2`, `hex`, `tempfile`, `log`, `thiserror`, `anyhow`
- Optional extraction dependencies: `liteparse`, `undoc`, `tesseract-rs`

The default feature set does not enable extraction adapters. `extraction-deps` enables PDF/DOCX/DOC extraction via `liteparse` and `undoc`; `extraction-ocr` additionally enables `tesseract-rs`.

`cargo tree --features extraction-ocr -e normal` shows `reqwest` as a transitive dependency under `liteparse`. Rustitler does not call a network API in application code, and the Tauri capability set exposes no network capability to the frontend. Release validation must still run in an offline environment because the optional extraction dependency graph includes crates that are also used for network-capable libraries.

## Frontend Runtime Dependencies

Runtime dependencies in `package.json`:

- `@tauri-apps/api`
- `@tauri-apps/plugin-shell`
- `react`
- `react-dom`

The current UI imports `@tauri-apps/api` only. `@tauri-apps/plugin-shell` remains installed but unused by source code and has no matching Tauri shell capability, so it is not callable from the app. Dev/test dependencies are build-time only.

## Runtime Assets

Runtime asset resolution is implemented in `src-tauri/src/packaging.rs`:

- Tessdata priority: `RUSTITLER_TESSDATA`, `TESSDATA_PREFIX`, bundled `resources/tessdata`, platform development fallback.
- LibreOffice priority: `RUSTITLER_SOFFICE`, bundled `resources/libreoffice/<platform path>`, system `soffice` fallback.

Tauri bundle resources are configured in `src-tauri/tauri.conf.json`:

- `resources/tessdata` -> `tessdata`
- `resources/libreoffice` -> `libreoffice`

The repository tracks placeholder directories only. Release packaging must place real assets before bundle creation:

- `src-tauri/resources/tessdata/chi_sim.traineddata`
- macOS LibreOffice path: `src-tauri/resources/libreoffice/LibreOffice.app/Contents/MacOS/soffice`
- Windows LibreOffice path: `src-tauri/resources/libreoffice/program/soffice.exe`
