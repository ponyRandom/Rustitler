# Packaging Asset Size Tracking

Record bundle size and major offline asset size contributions here for each release candidate.

## Measurement Commands

CI release candidates:

```bash
npm run package:size-report -- --platform macos --output package-size-report-macos.md
npm run package:size-report -- --platform windows --output package-size-report-windows.md
```

`.github/workflows/offline-package.yml` runs the same report after each macOS/Windows bundle build, appends it to the GitHub Actions step summary, and uploads it with the platform bundle artifacts. On `v*` tags, the report is also attached to the GitHub Release.

macOS:

```bash
du -sh src-tauri/target/release/bundle/macos/Rustitler.app
du -sh src-tauri/resources/tessdata
du -sh src-tauri/resources/libreoffice
find src-tauri/target/release/bundle -maxdepth 3 -type f -print0 | xargs -0 du -h | sort -h
```

Windows runner:

```powershell
Get-ChildItem src-tauri\target\release\bundle -Recurse | Measure-Object -Property Length -Sum
Get-ChildItem src-tauri\resources\tessdata -Recurse | Measure-Object -Property Length -Sum
Get-ChildItem src-tauri\resources\libreoffice -Recurse | Measure-Object -Property Length -Sum
```

## Current Repository Baseline

The repository currently tracks the Simplified Chinese OCR language asset and a placeholder LibreOffice directory:

- `src-tauri/resources/tessdata/.gitkeep`
- `src-tauri/resources/tessdata/chi_sim.traineddata` — 2.4 MB file, 3.1 MB directory usage on the current macOS filesystem.
- `src-tauri/resources/libreoffice/.gitkeep`

Release bundle size reports are generated automatically by the offline package workflow. The current macOS local report, generated before bundling real LibreOffice assets, showed:

- Bundle total: 16.7 MiB
- Tessdata resources: 2.4 MiB
- LibreOffice resources: 1 B placeholder

The release workflow artifact should be treated as the authoritative number for each tagged release candidate.
