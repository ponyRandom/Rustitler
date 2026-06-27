# CI Offline Package Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add macOS/Windows GitHub Actions package validation that builds offline bundles, runs a packaged-binary smoke test, and records bundle sizes.

**Architecture:** The packaged app binary gains a pre-Tauri CLI smoke-test mode so CI can validate runtime assets and core offline modules without GUI automation. A Node report script measures package/resource sizes and publishes Markdown in Actions. A dedicated workflow runs this on macOS and Windows while documentation clearly distinguishes CI smoke validation from final manual offline testing.

**Tech Stack:** Rust/Tauri v2, `offline-bundle` Cargo feature, Node `node:test`, GitHub Actions matrix jobs, Markdown task tracking.

---

### Task 1: Rust Offline Smoke Test CLI

**Files:**
- Create: `src-tauri/src/offline_smoke.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] Add failing Rust tests for parsing `--offline-smoke-test`, resource checks, and JSON output.
- [ ] Run targeted cargo test and confirm tests fail because the module does not exist.
- [ ] Implement the smoke test module and wire it into `main.rs` before `tauri::Builder::default()`.
- [ ] Run targeted cargo test with `--features offline-bundle --lib`.

### Task 2: Package Size Report Script

**Files:**
- Create: `scripts/package-size-report.mjs`
- Create: `scripts/package-size-report.node-test.mjs`
- Modify: `package.json`

- [ ] Add failing Node tests for byte formatting, recursive directory totals, and Markdown report content.
- [ ] Run `node --test scripts/package-size-report.node-test.mjs` and confirm it fails.
- [ ] Implement package size report helpers and CLI.
- [ ] Add `test:package-size-report` and `package:size-report` npm scripts.
- [ ] Run the package size report tests.

### Task 3: GitHub Actions Package Workflow

**Files:**
- Create: `.github/workflows/offline-package.yml`
- Modify: `.github/workflows/windows-ci.yml`

- [ ] Add a dedicated `offline-package.yml` workflow with `macos-latest` and `windows-latest` jobs.
- [ ] Build with `npm run tauri:build:offline`.
- [ ] Run the built platform binary with `--offline-smoke-test --resource-dir src-tauri/resources --app-data-dir <temp> --require-ocr`.
- [ ] Generate and upload package size reports and bundle artifacts.
- [ ] Keep existing `windows-ci.yml` focused on Windows CI, but avoid duplicating package validation steps where possible.

### Task 4: Docs and Task Tracking

**Files:**
- Modify: `doc/tasks/packaging-offline.md`
- Modify: `doc/tasks/progress.md`
- Modify: `doc/packaging/asset-sizes.md`
- Modify: `doc/packaging/offline-audit.md`

- [ ] Document CI smoke validation commands and limitations.
- [ ] Mark PK-10 and PK-11 as CI-covered once the OCR smoke path exists.
- [ ] Leave PK-12 and PK-13 pending until real LibreOffice runtime assets are present in bundles.
- [ ] Mark PK-16 as CI-reporting configured, but note release numbers are filled by workflow artifacts.

### Task 5: Verification

- [ ] Run Rust format check.
- [ ] Run Rust tests for default and offline-bundle configurations.
- [ ] Run Node tests.
- [ ] Run frontend tests and build.
- [ ] Run `git diff --check`.

