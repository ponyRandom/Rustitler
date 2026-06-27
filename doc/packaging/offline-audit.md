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
- Optional extraction dependencies: `liteparse`, `undoc`, `tesseract-rs`, `image`

The default feature set does not enable extraction adapters. `extraction-deps` enables PDF/DOCX/DOC extraction via `liteparse` and `undoc`; `extraction-ocr` additionally enables `tesseract-rs` plus PNG/JPEG decoding through `image`; `offline-bundle` is the release packaging feature that currently aliases `extraction-ocr`.

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
- OCR runtime: `tesseract-rs` builds and links Tesseract/Leptonica as static native libraries at Cargo build time. There is no separate `tesseract` CLI executable to bundle.

Tauri bundle resources are configured in `src-tauri/tauri.conf.json`:

- `resources/tessdata` -> `tessdata`
- `resources/libreoffice` -> `libreoffice`

The repository tracks the Simplified Chinese OCR language asset and placeholder LibreOffice directory. Release packaging must prepare or verify real assets before bundle creation:

- `src-tauri/resources/tessdata/chi_sim.traineddata`
- macOS LibreOffice path: `src-tauri/resources/libreoffice/LibreOffice.app/Contents/MacOS/soffice`
- Windows LibreOffice path: `src-tauri/resources/libreoffice/program/soffice.exe`

`npm run prepare:offline-assets` copies `chi_sim.traineddata` from `RUSTITLER_TESSDATA`, `TESSDATA_PREFIX`, or the platform `tesseract-rs` cache into `src-tauri/resources/tessdata`. Use `npm run prepare:offline-assets -- --download` only in an online release/CI preparation step.

Offline release builds must use:

```bash
npm run tauri:build:offline
```

That command prepares OCR assets and invokes `tauri build --features offline-bundle`.

## CI Package Validation

GitHub-hosted runners cannot be treated as physically disconnected machines because the runner must still communicate with GitHub for logs and artifacts. The automated release check therefore uses a packaged-binary smoke test instead of claiming full network isolation.

`.github/workflows/offline-package.yml` builds real macOS and Windows Tauri bundles with:

```bash
npm run tauri:build:offline:ci
```

After bundling, the workflow runs the packaged app binary with:

```bash
Rustitler --offline-smoke-test --resource-dir <packaged-resources> --app-data-dir <temp-dir> --report-path <json> --require-ocr
```

The smoke test verifies bundled `chi_sim.traineddata`, runs image OCR through the real Tesseract adapter, and checks settings/history persistence in an isolated app data directory. It reports whether packaged LibreOffice is present, but `.doc` conversion remains pending until real LibreOffice runtime assets are included instead of the placeholder directory.

Pushes and pull requests upload temporary GitHub Actions artifacts only. A tag matching `v*` additionally creates or updates the GitHub Release and uploads the package installers, smoke-test JSON reports, and package-size Markdown reports as Release assets.
