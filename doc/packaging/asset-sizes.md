# Packaging Asset Size Tracking

Record bundle size and major offline asset size contributions here for each release candidate.

## Measurement Commands

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

The repository currently tracks only placeholder resource directories:

- `src-tauri/resources/tessdata/.gitkeep`
- `src-tauri/resources/libreoffice/.gitkeep`

No release bundle size is recorded yet because real OCR language data and LibreOffice runtime assets are not checked into the repository. PK-16 remains pending until macOS and Windows release artifacts are built with real assets.
