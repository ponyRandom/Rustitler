pub mod errors;
pub mod models;
pub mod offline_smoke;
pub mod packaging;

// Module stubs — implemented in subsequent tasks per progress.md
pub mod batch_scheduler;
pub mod commands;
pub mod diagnostics;
pub mod extract;
pub mod history;
pub mod ingest;
pub mod rename;
pub mod scoring;
pub mod settings;

#[cfg(feature = "spikes")]
pub mod spikes;

// Tauri app entry is in main.rs. lib.rs exposes modules for testing.
