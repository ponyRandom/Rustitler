//! Dependency spike validation tests.
//!
//! Run all spikes (requires external crates to exist on crates.io):
//!   cargo test --features spikes -- spikes:: --nocapture 2>&1 | tee doc/tasks/spike-results.txt
//!
//! Run only known-good spikes (rusqlite, sha2):
//!   cargo test -- spikes:: --nocapture

// ── DS-16: SQLite basic connectivity ──────────────────────────────────────

#[cfg(test)]
mod ds16_sqlite {
    use rusqlite::{params, Connection};

    #[test]
    fn sqlite_create_insert_query() {
        let conn = Connection::open_in_memory().expect("DS-16: open in-memory SQLite");
        conn.execute_batch("CREATE TABLE batches (id TEXT PRIMARY KEY, created_at TEXT NOT NULL);")
            .expect("DS-16: CREATE TABLE");
        conn.execute(
            "INSERT INTO batches (id, created_at) VALUES (?1, ?2)",
            params!["batch-001", "2026-06-24T00:00:00Z"],
        )
        .expect("DS-16: INSERT");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM batches", [], |row| row.get(0))
            .expect("DS-16: SELECT COUNT");
        assert_eq!(count, 1, "DS-16 PASS: SQLite bundled read-write works");
    }

    #[test]
    fn sqlite_app_data_dir_writable() {
        // Validates that we can open a file-backed DB (simulates history.sqlite).
        let dir = tempfile::tempdir().expect("DS-16: tempdir");
        let db_path = dir.path().join("history.sqlite");
        let conn = Connection::open(&db_path).expect("DS-16: open file SQLite");
        conn.execute_batch("CREATE TABLE t (x INTEGER);")
            .expect("DS-16: create");
        drop(conn);
        assert!(db_path.exists(), "DS-16 PASS: file-backed SQLite created");
    }
}

// ── DS-17: large-file hash performance ────────────────────────────────────

#[cfg(test)]
mod ds17_hash_perf {
    use sha2::{Digest, Sha256};

    #[test]
    fn hash_10mb_in_reasonable_time() {
        // 10 MB synthetic file — undo output-modified check must complete fast.
        let data = vec![0xABu8; 10 * 1024 * 1024];
        let start = std::time::Instant::now();
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let _digest = hex::encode(hasher.finalize());
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 2,
            "DS-17 PASS: 10 MB SHA-256 in {:?} (< 2 s)",
            elapsed
        );
    }
}

// ── DS-01..04: liteparse — PDF extraction ─────────────────────────────────

#[cfg(all(test, feature = "spikes"))]
mod ds01_liteparse {
    // Validates that liteparse exists, compiles, and can:
    //   DS-01  read Chinese PDF text
    //   DS-02  expose page size, page number, block coordinates
    //   DS-03  return a classifiable error on empty / extraction failure
    //   DS-04  map output to LayoutBlock fields

    fn parser_without_ocr() -> liteparse::LiteParse {
        let config = liteparse::LiteParseConfig {
            ocr_enabled: false,
            quiet: true,
            ..Default::default()
        };
        liteparse::LiteParse::new(config)
    }

    fn write_chinese_text_pdf(dir: &std::path::Path) -> std::path::PathBuf {
        let txt_path = dir.join("sample_chinese.txt");
        let pdf_path = dir.join("sample_chinese.pdf");
        std::fs::write(
            &txt_path,
            "关于召开年度工作会议的通知\n正文第一段\n第二页不会用于首页验证\n",
        )
        .expect("DS-01: write text fixture");
        let output = std::process::Command::new("cupsfilter")
            .arg(&txt_path)
            .output()
            .expect("DS-01: run cupsfilter to create PDF fixture");
        assert!(
            output.status.success(),
            "DS-01: cupsfilter failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(&pdf_path, output.stdout).expect("DS-01: write generated PDF");
        pdf_path
    }

    #[tokio::test]
    async fn liteparse_reads_generated_chinese_pdf_text() {
        // DS-01: parse a real, locally generated Chinese PDF without network services.
        let dir = tempfile::tempdir().expect("DS-01: tempdir");
        let pdf_path = write_chinese_text_pdf(dir.path());
        let parser = parser_without_ocr();
        let result = parser
            .parse(pdf_path.to_str().expect("DS-01: UTF-8 temp path"))
            .await
            .expect("DS-01: parse generated Chinese PDF");
        assert!(
            result.text.contains("关于召开年度工作会议的通知"),
            "DS-01: extracted text should retain Chinese title, got {:?}",
            result.text
        );
        println!(
            "DS-01 PASS: extracted {} chars from generated Chinese PDF",
            result.text.len()
        );
    }

    #[tokio::test]
    async fn liteparse_page_size_and_blocks() {
        // DS-02: page dimensions and block-level coordinates.
        let dir = tempfile::tempdir().expect("DS-02: tempdir");
        let pdf_path = write_chinese_text_pdf(dir.path());
        let result = parser_without_ocr()
            .parse(pdf_path.to_str().expect("DS-02: UTF-8 temp path"))
            .await
            .expect("DS-02: parse PDF");
        let page = result.pages.first().expect("DS-02: first page");
        assert!(page.page_width > 0.0, "DS-02: page width");
        assert!(page.page_height > 0.0, "DS-02: page height");
        assert!(!page.text_items.is_empty(), "DS-02: text items non-empty");
        let b = page
            .text_items
            .iter()
            .find(|item| item.text.contains("关于召开年度工作会议的通知"))
            .unwrap_or(&page.text_items[0]);
        assert!(!b.text.trim().is_empty(), "DS-04: text field maps");
        assert!(b.x >= 0.0, "DS-04: x maps");
        assert!(b.y >= 0.0, "DS-04: y maps");
        assert!(b.width > 0.0, "DS-04: width maps");
        assert!(b.height > 0.0, "DS-04: height maps");
        println!(
            "DS-02/04 PASS: {:.0}x{:.0} pts, {} text items",
            page.page_width,
            page.page_height,
            page.text_items.len()
        );
    }

    #[tokio::test]
    async fn liteparse_empty_pdf_error() {
        // DS-03: classifiable error on bad input.
        let dir = tempfile::tempdir().expect("DS-03: tempdir");
        let bad_pdf = dir.path().join("empty.pdf");
        std::fs::write(&bad_pdf, b"%PDF-1.7\n% truncated\n").expect("DS-03: write bad PDF");
        let result = parser_without_ocr()
            .parse(bad_pdf.to_str().expect("DS-03: UTF-8 temp path"))
            .await;
        match result {
            Err(e) => println!("DS-03 PASS: error type = {:?}", e),
            Ok(_) => panic!("DS-03: malformed PDF should return an error"),
        }
    }
}

// ── DS-05..07: undoc — Word/DOCX extraction ───────────────────────────────

#[cfg(all(test, feature = "spikes"))]
mod ds05_undoc {
    fn write_chinese_docx(dir: &std::path::Path) -> std::path::PathBuf {
        let txt_path = dir.join("sample_chinese.txt");
        let docx_path = dir.join("sample_chinese.docx");
        std::fs::write(
            &txt_path,
            "关于召开年度工作会议的通知\n正文第一段\n正文第二段\n",
        )
        .expect("DS-05: write text fixture");
        let output = std::process::Command::new("textutil")
            .args(["-convert", "docx"])
            .arg(&txt_path)
            .arg("-output")
            .arg(&docx_path)
            .output()
            .expect("DS-05: run textutil to create DOCX fixture");
        assert!(
            output.status.success(),
            "DS-05: textutil failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        docx_path
    }

    #[test]
    fn undoc_extracts_generated_chinese_docx_text() {
        // DS-05: parse a real, locally generated DOCX fixture.
        let dir = tempfile::tempdir().expect("DS-05: tempdir");
        let docx_path = write_chinese_docx(dir.path());
        let doc = undoc::parse_file(&docx_path).expect("DS-05: parse generated docx");
        let text = doc.plain_text();
        assert!(
            text.contains("关于召开年度工作会议的通知"),
            "DS-05: Chinese title retained in plain text, got {:?}",
            text
        );
        println!("DS-05 PASS: extracted {} chars", text.len());
    }

    #[test]
    fn undoc_chinese_paragraphs() {
        // DS-06: paragraph/block boundaries are retained well enough for scoring.
        let dir = tempfile::tempdir().expect("DS-06: tempdir");
        let docx_path = write_chinese_docx(dir.path());
        let doc = undoc::parse_file(&docx_path).expect("DS-06: parse docx");
        let text = doc.plain_text();
        assert!(!doc.is_empty(), "DS-06: non-empty document");
        let has_chinese = text.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c));
        assert!(has_chinese, "DS-06: Chinese characters retained");
        assert!(
            doc.total_blocks() >= 3,
            "DS-06: expected at least three paragraph-like blocks, got {}",
            doc.total_blocks()
        );
        println!(
            "DS-05/06 PASS: {} chars, {} blocks",
            text.len(),
            doc.total_blocks()
        );
    }

    #[test]
    fn undoc_corrupt_file_error() {
        let dir = tempfile::tempdir().expect("DS-07: tempdir");
        let corrupt_path = dir.path().join("corrupt.docx");
        std::fs::write(&corrupt_path, b"not a zip").expect("DS-07: write corrupt fixture");
        let result = undoc::parse_file(&corrupt_path);
        match result {
            Err(e) => println!("DS-07 PASS: error = {:?}", e),
            Ok(_) => panic!("DS-07: corrupt DOCX should return an error"),
        }
    }
}

// ── DS-09..10: legacy DOC conversion via LibreOffice ──────────────────────

#[cfg(all(test, feature = "spikes", target_os = "macos"))]
mod ds09_doc_conversion {
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    struct DocConversionSpikeError {
        code: &'static str,
        message: String,
    }

    fn find_soffice() -> Option<PathBuf> {
        std::env::var_os("RUSTITLER_SOFFICE")
            .map(PathBuf::from)
            .filter(|path| path.exists())
            .or_else(|| {
                [
                    "/opt/homebrew/bin/soffice",
                    "/usr/local/bin/soffice",
                    "/Applications/LibreOffice.app/Contents/MacOS/soffice",
                ]
                .iter()
                .map(PathBuf::from)
                .find(|path| path.exists())
            })
    }

    fn write_chinese_doc_fixture(dir: &Path) -> PathBuf {
        let txt_path = dir.join("sample_chinese_doc_source.txt");
        let doc_path = dir.join("sample_chinese.doc");
        std::fs::write(
            &txt_path,
            "关于召开年度工作会议的通知\n正文第一段\n正文第二段\n",
        )
        .expect("DS-09: write text fixture");
        let output = std::process::Command::new("textutil")
            .args(["-convert", "doc"])
            .arg(&txt_path)
            .arg("-output")
            .arg(&doc_path)
            .output()
            .expect("DS-09: run textutil to create legacy DOC fixture");
        assert!(
            output.status.success(),
            "DS-09: textutil failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(doc_path.exists(), "DS-09: legacy DOC fixture exists");
        doc_path
    }

    fn convert_doc_to_docx(
        soffice: &Path,
        doc_path: &Path,
        out_dir: &Path,
    ) -> Result<PathBuf, DocConversionSpikeError> {
        let profile_dir = out_dir.join("lo-profile");
        std::fs::create_dir_all(&profile_dir).map_err(|error| DocConversionSpikeError {
            code: "docConvertFailed",
            message: format!("LibreOffice profile directory could not be created: {error}"),
        })?;
        let user_installation = format!("file://{}", profile_dir.display());
        let output = std::process::Command::new(soffice)
            .arg(format!("-env:UserInstallation={user_installation}"))
            .arg("--headless")
            .arg("--convert-to")
            .arg("docx")
            .arg("--outdir")
            .arg(out_dir)
            .arg(doc_path)
            .output()
            .map_err(|error| DocConversionSpikeError {
                code: "docConvertFailed",
                message: format!("LibreOffice failed to start: {error}"),
            })?;

        let docx_path = out_dir
            .join(doc_path.file_stem().expect("DS-09: fixture has file stem"))
            .with_extension("docx");

        if output.status.success() && docx_path.exists() {
            return Ok(docx_path);
        }

        Err(DocConversionSpikeError {
            code: "docConvertFailed",
            message: format!(
                "LibreOffice did not produce converted DOCX at {} (status: {}, stdout: {:?}, stderr: {:?})",
                docx_path.display(),
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }

    #[test]
    fn libreoffice_converts_legacy_doc_to_undoc_readable_docx() {
        let dir = tempfile::tempdir().expect("DS-09: tempdir");
        let doc_path = write_chinese_doc_fixture(dir.path());
        let soffice = find_soffice().expect("DS-09: soffice must be installed");

        let docx_path = convert_doc_to_docx(&soffice, &doc_path, dir.path())
            .expect("DS-09: convert .doc to .docx");
        let doc = undoc::parse_file(&docx_path).expect("DS-09: undoc parses converted DOCX");
        let text = doc.plain_text();

        assert!(
            text.contains("关于召开年度工作会议的通知"),
            "DS-09: converted DOCX should retain Chinese title, got {:?}",
            text
        );
    }

    #[test]
    fn libreoffice_invalid_doc_returns_stable_spike_error() {
        let dir = tempfile::tempdir().expect("DS-10: tempdir");
        let bad_doc = dir.path().join("bad.doc");
        std::fs::create_dir(&bad_doc).expect("DS-10: create invalid .doc directory fixture");
        let soffice = find_soffice().expect("DS-10: soffice must be installed");

        let error = convert_doc_to_docx(&soffice, &bad_doc, dir.path())
            .expect_err("DS-10: invalid .doc conversion should fail");

        assert_eq!(error.code, "docConvertFailed");
        assert!(
            error.message.contains("LibreOffice did not produce"),
            "DS-10: stable error text should describe missing output, got {:?}",
            error.message
        );
    }
}

// ── DS-11..13: Tesseract OCR ───────────────────────────────────────────────

#[cfg(all(test, feature = "spikes"))]
mod ds11_tesseract {
    // Uses tesseract-rs (same crate liteparse already pulls in).

    fn default_tessdata_dir() -> std::path::PathBuf {
        if let Ok(path) = std::env::var("TESSDATA_PREFIX") {
            return path.into();
        }
        let home = std::env::var("HOME").expect("DS-11: HOME not set");
        #[cfg(target_os = "macos")]
        {
            return std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("tesseract-rs")
                .join("tessdata");
        }
        #[cfg(target_os = "linux")]
        {
            return std::path::PathBuf::from(home)
                .join(".tesseract-rs")
                .join("tessdata");
        }
        #[cfg(target_os = "windows")]
        {
            return std::env::var("APPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| {
                    std::path::PathBuf::from(home)
                        .join("AppData")
                        .join("Roaming")
                })
                .join("tesseract-rs")
                .join("tessdata");
        }
        #[allow(unreachable_code)]
        std::path::PathBuf::from("tessdata")
    }

    #[test]
    fn tesseract_chi_sim_loads() {
        // DS-11: simplified Chinese lang data loads.
        // TESSDATA_PREFIX must point to bundled tessdata dir.
        let tessdata = default_tessdata_dir();
        let chi_sim = tessdata.join("chi_sim.traineddata");
        assert!(
            chi_sim.exists(),
            "DS-11: chi_sim.traineddata must exist at {}",
            chi_sim.display()
        );
        let api = tesseract_rs::TesseractAPI::new();
        api.init(&tessdata, "chi_sim")
            .expect("DS-11: init chi_sim language data");
        println!(
            "DS-11 PASS: chi_sim language data loaded from {}",
            tessdata.display()
        );
    }

    #[test]
    fn tesseract_text_and_confidence() {
        // DS-13: OCR text and confidence API on a tiny synthetic grayscale image.
        let tessdata = default_tessdata_dir();
        let eng = tessdata.join("eng.traineddata");
        assert!(
            eng.exists(),
            "DS-13: eng.traineddata must exist at {}",
            eng.display()
        );
        let api = tesseract_rs::TesseractAPI::new();
        api.init(&tessdata, "eng").expect("DS-13: init eng");
        api.set_variable("tessedit_char_whitelist", "0123456789")
            .expect("DS-13: set whitelist");
        api.set_variable("tessedit_pageseg_mode", "10")
            .expect("DS-13: set psm");

        let width = 24usize;
        let height = 24usize;
        let mut image_data = vec![255u8; width * height];
        for y in 4..19 {
            for x in 7..17 {
                if (y == 4 && (8..=15).contains(&x))
                    || (y == 11 && (8..=15).contains(&x))
                    || (y == 18 && (8..=15).contains(&x))
                    || ((4..=10).contains(&y) && x == 7)
                    || ((4..=18).contains(&y) && x == 16)
                {
                    image_data[y * width + x] = 0;
                }
            }
        }
        api.set_image(&image_data, width as i32, height as i32, 1, width as i32)
            .expect("DS-13: set image");
        let text = api.get_utf8_text().expect("DS-13: get text");
        let confidence = api.mean_text_conf().expect("DS-13: confidence");
        let iterator = api.get_iterator().expect("DS-13: get result iterator");
        let (word, left, top, right, bottom, word_confidence) = iterator
            .get_current_word()
            .expect("DS-13: get OCR word bounds");
        assert!(!text.trim().is_empty(), "DS-13: OCR text returned");
        assert!(confidence >= 0, "DS-13: confidence returned");
        assert!(!word.trim().is_empty(), "DS-13: word text returned");
        assert!(right > left, "DS-13: word bbox width");
        assert!(bottom > top, "DS-13: word bbox height");
        assert!(word_confidence >= 0.0, "DS-13: word confidence returned");
        println!(
            "DS-13 PASS: OCR returned {:?}, bbox=({}, {}, {}, {}), confidence {} / {}",
            text.trim(),
            left,
            top,
            right,
            bottom,
            confidence,
            word_confidence
        );
    }
}

// ── DS-14..15: scanned-PDF rasterization ─────────────────────────────────

#[cfg(all(test, feature = "spikes"))]
mod ds14_pdfium {
    fn write_pdf(dir: &std::path::Path) -> std::path::PathBuf {
        let txt_path = dir.join("sample_scanned_source.txt");
        let pdf_path = dir.join("sample_scanned.pdf");
        std::fs::write(&txt_path, "扫描件栅格化验证\n").expect("DS-14: write text fixture");
        let output = std::process::Command::new("cupsfilter")
            .arg(&txt_path)
            .output()
            .expect("DS-14: run cupsfilter to create PDF fixture");
        assert!(
            output.status.success(),
            "DS-14: cupsfilter failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::write(&pdf_path, output.stdout).expect("DS-14: write generated PDF");
        pdf_path
    }

    fn parser_without_ocr() -> liteparse::LiteParse {
        let config = liteparse::LiteParseConfig {
            ocr_enabled: false,
            quiet: true,
            ..Default::default()
        };
        liteparse::LiteParse::new(config)
    }

    #[tokio::test]
    async fn liteparse_first_page_screenshot_rasterizes_pdf() {
        // DS-14: rasterize first page of a PDF to PNG bytes offline.
        let dir = tempfile::tempdir().expect("DS-14: tempdir");
        let path = write_pdf(dir.path());
        let screenshots = parser_without_ocr()
            .screenshot(
                path.to_str().expect("DS-14: UTF-8 temp path"),
                Some(vec![1]),
            )
            .await
            .expect("DS-14: render first page screenshot");
        let screenshot = screenshots.first().expect("DS-14: first screenshot");
        assert!(
            !screenshot.image_bytes.is_empty(),
            "DS-14 PASS: rasterized {} PNG bytes",
            screenshot.image_bytes.len()
        );
        assert!(screenshot.width > 0, "DS-14: bitmap width");
        assert!(screenshot.height > 0, "DS-14: bitmap height");
        assert_eq!(screenshot.page_num, 1, "DS-14: rendered page number");
        println!(
            "DS-14 PASS: {}x{} PNG, {} bytes",
            screenshot.width,
            screenshot.height,
            screenshot.image_bytes.len()
        );
    }

    #[test]
    fn pdfium_temp_file_isolation() {
        // DS-15: verify temp files can be created per-batch and cleaned up.
        let dir = tempfile::tempdir().expect("DS-15: tempdir");
        let tmp_img = dir.path().join("page_0.png");
        std::fs::write(&tmp_img, b"fake-png").expect("DS-15: write");
        assert!(tmp_img.exists());
        drop(dir); // auto-cleans
        assert!(!tmp_img.exists(), "DS-15 PASS: temp dir cleaned on drop");
    }
}
