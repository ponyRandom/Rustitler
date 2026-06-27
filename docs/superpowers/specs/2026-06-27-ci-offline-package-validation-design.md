# CI Offline Package Validation Design

## Context

`packaging-offline` still has open acceptance tasks for real macOS/Windows package validation and bundle size recording. The current Windows CI builds a Tauri bundle with `offline-bundle`, but it does not exercise the packaged binary or emit a structured size report. There is no matching macOS package workflow.

## Approach

Add a CI-oriented validation path that is explicit about its limits. GitHub-hosted runners cannot be fully disconnected from the network because the runner must communicate with GitHub, so the workflow will perform offline-package smoke validation rather than claim a physically isolated machine test. The release artifact will still be the real platform bundle, and the user will perform final manual offline experience testing after release download.

## Components

- A Rust `--offline-smoke-test` CLI mode in the shipped binary. It will run before the Tauri window loop, use the same `RuntimeAssets` resolution as packaged runtime code, verify bundled OCR and DOC conversion resources, exercise settings/history persistence in an isolated app data directory, and run image OCR through the real `TesseractOcrExtractor` when `offline-bundle` is enabled.
- A Node size-report script that scans Tauri bundle output plus bundled resource directories and writes a Markdown report. It will also append the report to `$GITHUB_STEP_SUMMARY` when running in GitHub Actions.
- A new GitHub Actions workflow with macOS and Windows package jobs. Each job prepares OCR assets, builds the offline bundle, runs the packaged binary smoke test against the built resources, generates the size report, and uploads both bundles and reports.

## Acceptance

- CI package jobs build true macOS and Windows Tauri artifacts with `--features offline-bundle`.
- The packaged binary has a deterministic smoke-test mode for CI and release verification.
- Size reports are generated as artifacts and as GitHub step summaries.
- `doc/tasks/packaging-offline.md` is updated to show CI-covered package validation separately from final manual offline release experience.

