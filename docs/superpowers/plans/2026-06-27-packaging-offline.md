# Packaging Offline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the testable packaging/offline groundwork in `doc/tasks/packaging-offline.md`.

**Architecture:** Keep runtime asset resolution in Rust code that can be unit tested without a bundled app. Add packaging resources under `src-tauri/resources`, configure Tauri to include them, and document permission/dependency/license/size audit results. Leave platform acceptance tasks pending until actual macOS and Windows bundles are exercised offline.

**Tech Stack:** Rust unit tests, Tauri v2 config/capabilities, `tesseract-rs`, `liteparse`, `undoc`, LibreOffice headless discovery, npm/Cargo dependency audits, and Markdown packaging docs.

---

### Task 1: Security and Dependency Audit

**Files:**
- Modify: `src-tauri/capabilities/default.json`
- Create: `doc/packaging/offline-audit.md`

- [ ] **Step 1: Audit Tauri permissions**

Confirm capabilities contain only core path/event/window permissions needed by drag/drop and events; no shell, HTTP, updater, filesystem plugin, or network permission is enabled.

- [ ] **Step 2: Audit Rust and frontend dependencies**

Record direct runtime dependencies and note that `reqwest` is a transitive dependency under optional extraction features from `liteparse`, not an app-level network client.

### Task 2: OCR and DOC Runtime Asset Resolution

**Files:**
- Create: `src-tauri/src/packaging.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/extract.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving bundled tessdata, env tessdata, bundled soffice, env soffice, and system fallback paths resolve in the intended order.

- [ ] **Step 2: Run tests to verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml packaging::tests extract::tests::soffice_discovery_prefers_packaging_paths`

Expected: FAIL because `packaging` module and new discovery APIs do not exist.

- [ ] **Step 3: Implement resolver**

Add `RuntimeAssets`, `resolve_tessdata_dir`, `resolve_soffice_path`, and tests. Use `RUSTITLER_TESSDATA`, `TESSDATA_PREFIX`, and `RUSTITLER_SOFFICE` as override env vars, then bundled resources, then known system locations.

### Task 3: Bundle Resources and Documentation

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Create: `src-tauri/resources/tessdata/.gitkeep`
- Create: `src-tauri/resources/libreoffice/.gitkeep`
- Create: `doc/packaging/licenses.md`
- Create: `doc/packaging/asset-sizes.md`

- [ ] **Step 1: Configure bundle resources**

Include `resources/tessdata/*` and `resources/libreoffice/*` in the Tauri bundle.

- [ ] **Step 2: Document licenses and size tracking**

Record Tesseract, tessdata_fast, LibreOffice, liteparse, undoc, rusqlite, and tesseract-rs license/packaging notes and commands for measuring bundle size.

### Task 4: Verification and Task Status

**Files:**
- Modify: `doc/tasks/packaging-offline.md`
- Modify: `doc/tasks/progress.md`

- [ ] **Step 1: Run verification**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && npm test && npm run build`.

- [ ] **Step 2: Mark only verified packaging tasks complete**

Mark PK-01, PK-02, PK-03, PK-08, PK-09, and PK-15 complete if the audits, resolver tests, resource config, and docs are in place. Leave platform bundle validation tasks pending.
