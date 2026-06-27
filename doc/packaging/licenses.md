# Packaging License Notes

This file records the licensing surface for offline runtime assets and core extraction dependencies. It is not legal advice; release builds should include the upstream license files beside bundled third-party assets.

| Component | Purpose | License Notes |
| --- | --- | --- |
| Tesseract OCR engine / `tesseract-rs` | OCR runtime binding | Tesseract is Apache-2.0. `tesseract-rs` is a Rust binding; include its crate license from the Cargo registry in release notices. |
| `tessdata_fast/chi_sim.traineddata` | Simplified Chinese OCR language data | Official Tesseract traineddata asset. Include the upstream tessdata_fast license/notice in release bundles. |
| LibreOffice | Legacy `.doc` to `.docx` conversion | LibreOffice is primarily MPL-2.0 and includes third-party notices. Bundling the full app/runtime requires including LibreOffice license and notice files. |
| `liteparse` | PDF text extraction and PDF rasterization | Cargo dependency; include crate license in release notices. It brings `liteparse-pdfium`/PDFium runtime surface. |
| `undoc` | DOCX text extraction | Cargo dependency; include crate license in release notices. |
| `rusqlite` / bundled SQLite | Local history database | `rusqlite` license comes from the crate; SQLite itself is public domain with bundled source from `libsqlite3-sys`. |
| React / Tauri JS API | Frontend runtime | Include npm package licenses in generated frontend dependency notices if distributing a full third-party notice bundle. |

Release checklist:

- Copy upstream Tesseract/tessdata license files into the app's third-party notices.
- Copy LibreOffice `LICENSE*`, `NOTICE*`, and bundled third-party readme/license files when LibreOffice is included.
- Generate Cargo and npm dependency license manifests during release packaging.
- Keep this document updated if extraction crates or runtime assets change.
