use crate::diagnostics::Diagnostics;
use crate::errors::{AppError, ErrorCategory, ErrorCode, ProcessingStage};
use crate::models::{
    BatchId, ExtractMethod, ExtractedDocument, ExtractedPage, FileJobId, FileType, LayoutBlock,
    NormalizedBox, ParagraphBlock, RawBox, Settings, SourceUnit,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ExtractRequest {
    pub batch_id: BatchId,
    pub file_job_id: FileJobId,
    pub file_type: FileType,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertedDocFormat {
    Docx,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedDoc {
    pub intermediate_path: PathBuf,
    pub format: ConvertedDocFormat,
}

#[derive(Debug, Clone)]
pub struct RawTextBlock {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: Option<f32>,
    pub bold: Option<bool>,
    pub confidence: Option<f32>,
    pub line_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct RawPdfPage {
    pub page_index: usize,
    pub width: f32,
    pub height: f32,
    pub blocks: Vec<RawTextBlock>,
}

#[derive(Debug, Clone)]
pub struct RasterizedPage {
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
    pub image_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct OcrPageInput {
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
    pub image_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct OcrPage {
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
    pub blocks: Vec<LayoutBlock>,
}

pub trait DocxTextExtractor {
    fn extract_paragraphs(&self, path: &Path) -> Result<Vec<String>, AppError>;
}

pub trait DocConverter {
    fn convert(&self, input_path: &Path, work_dir: &Path) -> Result<ConvertedDoc, AppError>;
}

pub trait PdfTextExtractor {
    fn extract_pages(&self, path: &Path, pages: &[usize]) -> Result<Vec<RawPdfPage>, AppError>;
}

pub trait PdfRasterizer {
    fn rasterize_pages(
        &self,
        path: &Path,
        pages: &[usize],
        work_dir: &Path,
    ) -> Result<Vec<RasterizedPage>, AppError>;
}

pub trait OcrExtractor {
    fn extract_pages(&self, pages: &[OcrPageInput]) -> Result<Vec<OcrPage>, AppError>;
}

pub struct ExtractionServices<'a> {
    pub docx: &'a dyn DocxTextExtractor,
    pub doc_converter: &'a dyn DocConverter,
    pub pdf: &'a dyn PdfTextExtractor,
    pub rasterizer: &'a dyn PdfRasterizer,
    pub ocr: &'a dyn OcrExtractor,
}

#[cfg(feature = "extraction-deps")]
pub struct UndocDocxTextExtractor;

#[cfg(feature = "extraction-deps")]
impl DocxTextExtractor for UndocDocxTextExtractor {
    fn extract_paragraphs(&self, path: &Path) -> Result<Vec<String>, AppError> {
        let document = undoc::parse_file(path).map_err(|err| adapter_error(path, err))?;
        Ok(document
            .sections
            .iter()
            .flat_map(|section| section.content.iter())
            .filter_map(|block| match block {
                undoc::Block::Paragraph(paragraph) => Some(paragraph.plain_text()),
                undoc::Block::Table(table) => Some(table.plain_text()),
                _ => None,
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct SofficeDocConverter {
    soffice_path: PathBuf,
}

impl SofficeDocConverter {
    pub fn new(soffice_path: impl Into<PathBuf>) -> Self {
        Self {
            soffice_path: soffice_path.into(),
        }
    }

    pub fn discover() -> Option<Self> {
        Some(Self::new(crate::packaging::resolve_soffice_path(None)))
    }

    pub fn discover_with_assets(assets: Option<&crate::packaging::RuntimeAssets>) -> Self {
        Self::new(crate::packaging::resolve_soffice_path(assets))
    }

    pub fn soffice_path(&self) -> &Path {
        &self.soffice_path
    }
}

impl DocConverter for SofficeDocConverter {
    fn convert(&self, input_path: &Path, work_dir: &Path) -> Result<ConvertedDoc, AppError> {
        fs::create_dir_all(work_dir)
            .map_err(|err| extract_io_error(ErrorCode::DocConvertFailed, work_dir, err))?;

        let output = Command::new(&self.soffice_path)
            .args(["--headless", "--convert-to", "docx", "--outdir"])
            .arg(work_dir)
            .arg(input_path)
            .output()
            .map_err(|err| extract_io_error(ErrorCode::DocConvertFailed, input_path, err))?;
        let converted_path = work_dir
            .join(input_path.file_stem().ok_or_else(|| {
                AppError::internal("DOC input path has no file stem")
                    .with_path(input_path.display().to_string())
                    .with_stage(ProcessingStage::Extract)
            })?)
            .with_extension("docx");

        if output.status.success() && converted_path.exists() {
            return Ok(ConvertedDoc {
                intermediate_path: converted_path,
                format: ConvertedDocFormat::Docx,
            });
        }

        Err(AppError {
            code: ErrorCode::DocConvertFailed,
            category: ErrorCategory::Extraction,
            user_message: "无法将 DOC 文件转换为可提取格式。".into(),
            technical_detail: Some(format!(
                "LibreOffice did not produce converted DOCX at {} (status: {}, stdout: {:?}, stderr: {:?})",
                converted_path.display(),
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )),
            retryable: true,
            file_path: Some(input_path.display().to_string()),
            stage: Some(ProcessingStage::Extract),
        })
    }
}

#[cfg(feature = "extraction-deps")]
pub struct LiteparsePdfTextExtractor;

#[cfg(feature = "extraction-deps")]
impl Default for LiteparsePdfTextExtractor {
    fn default() -> Self {
        Self
    }
}

#[cfg(feature = "extraction-deps")]
impl PdfTextExtractor for LiteparsePdfTextExtractor {
    fn extract_pages(&self, path: &Path, pages: &[usize]) -> Result<Vec<RawPdfPage>, AppError> {
        let target_pages = pages
            .iter()
            .map(|page| page.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let config = liteparse::LiteParseConfig {
            ocr_enabled: false,
            quiet: true,
            max_pages: pages.len().max(1),
            target_pages: Some(target_pages),
            ..Default::default()
        };
        let path_text = path_to_str(path)?;
        let parser = liteparse::LiteParse::new(config);
        let runtime = tokio::runtime::Runtime::new().map_err(|err| adapter_error(path, err))?;
        let result = runtime
            .block_on(parser.parse(path_text))
            .map_err(|err| adapter_error(path, err))?;

        Ok(result
            .pages
            .into_iter()
            .map(|page| RawPdfPage {
                page_index: page.page_number.saturating_sub(1),
                width: page.page_width,
                height: page.page_height,
                blocks: page
                    .text_items
                    .into_iter()
                    .map(|item| RawTextBlock {
                        text: item.text,
                        x: item.x,
                        y: item.y,
                        width: item.width,
                        height: item.height,
                        font_size: item.font_size.or(item.font_height),
                        bold: item.font_weight.map(|weight| weight >= 600),
                        confidence: item.confidence,
                        line_index: None,
                    })
                    .collect(),
            })
            .collect())
    }
}

#[cfg(feature = "extraction-deps")]
pub struct LiteparsePdfRasterizer;

#[cfg(feature = "extraction-deps")]
impl Default for LiteparsePdfRasterizer {
    fn default() -> Self {
        Self
    }
}

#[cfg(feature = "extraction-deps")]
impl PdfRasterizer for LiteparsePdfRasterizer {
    fn rasterize_pages(
        &self,
        path: &Path,
        pages: &[usize],
        work_dir: &Path,
    ) -> Result<Vec<RasterizedPage>, AppError> {
        fs::create_dir_all(work_dir)
            .map_err(|err| extract_io_error(ErrorCode::PdfOcrFallbackFailed, work_dir, err))?;
        let page_numbers = pages.iter().map(|page| *page as u32).collect::<Vec<_>>();
        let path_text = path_to_str(path)?;
        let parser = liteparse::LiteParse::new(liteparse::LiteParseConfig {
            ocr_enabled: false,
            quiet: true,
            max_pages: pages.len().max(1),
            ..Default::default()
        });
        let runtime = tokio::runtime::Runtime::new().map_err(|err| adapter_error(path, err))?;
        let screenshots = runtime
            .block_on(parser.screenshot(path_text, Some(page_numbers)))
            .map_err(|err| adapter_error(path, err))?;

        screenshots
            .into_iter()
            .map(|screenshot| {
                let image_path = work_dir.join(format!("page-{}.png", screenshot.page_num));
                fs::write(&image_path, &screenshot.image_bytes).map_err(|err| {
                    extract_io_error(ErrorCode::PdfOcrFallbackFailed, &image_path, err)
                })?;
                Ok(RasterizedPage {
                    page_index: screenshot.page_num.saturating_sub(1) as usize,
                    width: screenshot.width,
                    height: screenshot.height,
                    image_path,
                })
            })
            .collect()
    }
}

pub struct BatchTempDir {
    path: PathBuf,
}

impl BatchTempDir {
    pub fn new(batch_id: &str) -> Result<Self, AppError> {
        let path =
            std::env::temp_dir().join(format!("rustitler-{batch_id}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path)
            .map_err(|err| extract_io_error(ErrorCode::FileReadFailed, &path, err))?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for BatchTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn extract_document_with_services(
    request: &ExtractRequest,
    services: &ExtractionServices<'_>,
    work_dir: &Path,
) -> Result<ExtractedDocument, AppError> {
    match request.file_type {
        FileType::Docx => extract_docx(
            request,
            services,
            &request.source_path,
            ExtractMethod::WordUndoc,
        ),
        FileType::Doc => extract_doc(request, services, work_dir),
        FileType::Pdf => extract_pdf(request, services, work_dir),
        FileType::Png | FileType::Jpg | FileType::Jpeg => extract_image(request, services),
        FileType::Unsupported => Err(unsupported_format_error(&request.source_path)),
    }
}

pub fn extract_document_with_diagnostics(
    request: &ExtractRequest,
    services: &ExtractionServices<'_>,
    work_dir: &Path,
    diagnostics: &Diagnostics,
    settings: &Settings,
) -> Result<ExtractedDocument, AppError> {
    let mut document = extract_document_with_services(request, services, work_dir)?;
    document.diagnostics_ref = diagnostics.save_extracted_document(
        settings,
        &request.batch_id,
        &request.file_job_id,
        &document,
    )?;
    Ok(document)
}

fn extract_docx(
    request: &ExtractRequest,
    services: &ExtractionServices<'_>,
    path: &Path,
    method: ExtractMethod,
) -> Result<ExtractedDocument, AppError> {
    let raw_paragraphs = services
        .docx
        .extract_paragraphs(path)
        .map_err(|err| word_extract_error(path, err))?;
    let paragraphs = first_ten_non_empty_paragraphs(raw_paragraphs);

    Ok(ExtractedDocument {
        source_type: request.file_type.clone(),
        extract_method: method,
        pages: vec![],
        paragraphs,
        diagnostics_ref: None,
    })
}

fn extract_doc(
    request: &ExtractRequest,
    services: &ExtractionServices<'_>,
    work_dir: &Path,
) -> Result<ExtractedDocument, AppError> {
    let converted = services
        .doc_converter
        .convert(&request.source_path, work_dir)
        .map_err(|err| doc_convert_error(&request.source_path, err))?;

    match converted.format {
        ConvertedDocFormat::Docx => extract_docx(
            request,
            services,
            &converted.intermediate_path,
            ExtractMethod::DocConvertedUndoc,
        ),
    }
}

fn extract_pdf(
    request: &ExtractRequest,
    services: &ExtractionServices<'_>,
    work_dir: &Path,
) -> Result<ExtractedDocument, AppError> {
    match services.pdf.extract_pages(&request.source_path, &[1, 2, 3]) {
        Ok(pages) if pages_have_text(&pages) => Ok(ExtractedDocument {
            source_type: FileType::Pdf,
            extract_method: ExtractMethod::PdfNativeLiteparse,
            pages: pages.into_iter().map(pdf_page_from_raw).collect(),
            paragraphs: vec![],
            diagnostics_ref: None,
        }),
        Ok(_) => extract_pdf_ocr_fallback(request, services, work_dir),
        Err(_) => extract_pdf_ocr_fallback(request, services, work_dir),
    }
}

fn extract_pdf_ocr_fallback(
    request: &ExtractRequest,
    services: &ExtractionServices<'_>,
    work_dir: &Path,
) -> Result<ExtractedDocument, AppError> {
    let rasterized_pages = services
        .rasterizer
        .rasterize_pages(&request.source_path, &[1, 2, 3], work_dir)
        .map_err(|err| pdf_ocr_error(&request.source_path, err))?;
    let pages = ocr_rasterized_pages(services, rasterized_pages)
        .map_err(|err| pdf_ocr_error(&request.source_path, err))?;

    Ok(ExtractedDocument {
        source_type: FileType::Pdf,
        extract_method: ExtractMethod::PdfOcrFallbackTesseract,
        pages,
        paragraphs: vec![],
        diagnostics_ref: None,
    })
}

fn extract_image(
    request: &ExtractRequest,
    services: &ExtractionServices<'_>,
) -> Result<ExtractedDocument, AppError> {
    let inputs = vec![OcrPageInput {
        page_index: 0,
        width: 0,
        height: 0,
        image_path: request.source_path.clone(),
    }];
    let pages = services
        .ocr
        .extract_pages(&inputs)
        .map_err(|err| ocr_error(&request.source_path, err))?
        .into_iter()
        .map(extracted_page_from_ocr)
        .collect();

    Ok(ExtractedDocument {
        source_type: request.file_type.clone(),
        extract_method: ExtractMethod::ImageOcrTesseract,
        pages,
        paragraphs: vec![],
        diagnostics_ref: None,
    })
}

fn ocr_rasterized_pages(
    services: &ExtractionServices<'_>,
    rasterized_pages: Vec<RasterizedPage>,
) -> Result<Vec<ExtractedPage>, AppError> {
    let inputs = rasterized_pages
        .into_iter()
        .map(|page| OcrPageInput {
            page_index: page.page_index,
            width: page.width,
            height: page.height,
            image_path: page.image_path,
        })
        .collect::<Vec<_>>();
    services
        .ocr
        .extract_pages(&inputs)
        .map(|pages| pages.into_iter().map(extracted_page_from_ocr).collect())
}

fn first_ten_non_empty_paragraphs(raw_paragraphs: Vec<String>) -> Vec<ParagraphBlock> {
    raw_paragraphs
        .into_iter()
        .filter_map(|text| {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .take(10)
        .enumerate()
        .map(|(paragraph_index, text)| ParagraphBlock {
            text,
            paragraph_index,
        })
        .collect()
}

fn pages_have_text(pages: &[RawPdfPage]) -> bool {
    pages
        .iter()
        .flat_map(|page| &page.blocks)
        .any(|block| !block.text.trim().is_empty())
}

fn pdf_page_from_raw(page: RawPdfPage) -> ExtractedPage {
    ExtractedPage {
        page_index: page.page_index,
        width: page.width,
        height: page.height,
        unit: SourceUnit::PdfPoint,
        blocks: page
            .blocks
            .into_iter()
            .map(|block| layout_block_from_raw(block, page.width, page.height))
            .collect(),
    }
}

fn layout_block_from_raw(block: RawTextBlock, page_width: f32, page_height: f32) -> LayoutBlock {
    let raw_bbox = RawBox {
        x0: block.x,
        y0: block.y,
        x1: block.x + block.width,
        y1: block.y + block.height,
    };

    LayoutBlock {
        text: block.text,
        bbox: normalize_box(&raw_bbox, page_width, page_height),
        raw_bbox: Some(raw_bbox),
        font_size: block.font_size,
        bold: block.bold,
        ocr_confidence: block.confidence,
        line_index: block.line_index,
    }
}

fn normalize_box(raw: &RawBox, page_width: f32, page_height: f32) -> NormalizedBox {
    let width = page_width.max(1.0);
    let height = page_height.max(1.0);
    NormalizedBox {
        x0: clamp_unit(raw.x0 / width),
        y0: clamp_unit(raw.y0 / height),
        x1: clamp_unit(raw.x1 / width),
        y1: clamp_unit(raw.y1 / height),
    }
}

fn clamp_unit(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn extracted_page_from_ocr(page: OcrPage) -> ExtractedPage {
    ExtractedPage {
        page_index: page.page_index,
        width: page.width as f32,
        height: page.height as f32,
        unit: SourceUnit::Pixel,
        blocks: page.blocks,
    }
}

fn unsupported_format_error(path: &Path) -> AppError {
    AppError {
        code: ErrorCode::UnsupportedFormat,
        category: ErrorCategory::Input,
        user_message: "不支持的文件格式，无法提取内容。".into(),
        technical_detail: None,
        retryable: false,
        file_path: Some(path.display().to_string()),
        stage: Some(ProcessingStage::Extract),
    }
}

fn doc_convert_error(path: &Path, source: AppError) -> AppError {
    extraction_error(
        ErrorCode::DocConvertFailed,
        "无法将 DOC 文件转换为可提取格式。",
        path,
        source,
    )
}

fn word_extract_error(path: &Path, source: AppError) -> AppError {
    extraction_error(
        ErrorCode::WordExtractFailed,
        "无法提取 Word 文档文本。",
        path,
        source,
    )
}

fn pdf_ocr_error(path: &Path, source: AppError) -> AppError {
    extraction_error(
        ErrorCode::PdfOcrFallbackFailed,
        "无法对扫描 PDF 执行 OCR 兜底提取。",
        path,
        source,
    )
}

fn ocr_error(path: &Path, source: AppError) -> AppError {
    extraction_error(
        ErrorCode::OcrEngineFailed,
        "无法执行图片 OCR。",
        path,
        source,
    )
}

fn extraction_error(code: ErrorCode, message: &str, path: &Path, source: AppError) -> AppError {
    AppError {
        code,
        category: ErrorCategory::Extraction,
        user_message: message.into(),
        technical_detail: source.technical_detail.or(Some(source.user_message)),
        retryable: source.retryable,
        file_path: Some(path.display().to_string()),
        stage: Some(ProcessingStage::Extract),
    }
}

fn extract_io_error(code: ErrorCode, path: &Path, err: io::Error) -> AppError {
    AppError {
        code,
        category: ErrorCategory::Extraction,
        user_message: "提取临时目录不可用。".into(),
        technical_detail: Some(format!("failed to prepare '{}': {err}", path.display())),
        retryable: true,
        file_path: Some(path.display().to_string()),
        stage: Some(ProcessingStage::Extract),
    }
}

#[cfg(feature = "extraction-deps")]
fn path_to_str(path: &Path) -> Result<&str, AppError> {
    path.to_str().ok_or_else(|| {
        AppError::internal("path is not valid UTF-8")
            .with_path(path.display().to_string())
            .with_stage(ProcessingStage::Extract)
    })
}

#[cfg(feature = "extraction-deps")]
fn adapter_error(path: &Path, err: impl std::fmt::Display) -> AppError {
    AppError {
        code: ErrorCode::FileReadFailed,
        category: ErrorCategory::Extraction,
        user_message: "提取依赖执行失败。".into(),
        technical_detail: Some(err.to_string()),
        retryable: true,
        file_path: Some(path.display().to_string()),
        stage: Some(ProcessingStage::Extract),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{ErrorCategory, ErrorCode, ProcessingStage};
    use crate::models::{
        ExtractMethod, ExtractedPage, NormalizedBox, RawBox, Settings, SourceUnit,
    };
    use std::cell::RefCell;
    use std::fs;

    #[test]
    fn docx_extracts_first_ten_non_empty_paragraphs() {
        let services = test_services().with_docx_paragraphs(vec![
            "  ".into(),
            "关于召开年度工作会议的通知".into(),
            "正文第一段".into(),
            "\n".into(),
            "正文第二段".into(),
            "第四段".into(),
            "第五段".into(),
            "第六段".into(),
            "第七段".into(),
            "第八段".into(),
            "第九段".into(),
            "第十段".into(),
            "第十一段".into(),
            "第十二段不会进入候选范围".into(),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let request = request(FileType::Docx, dir.path().join("sample.docx"));

        let doc = extract_document_with_services(&request, &services.refs(), dir.path()).unwrap();

        assert_eq!(doc.source_type, FileType::Docx);
        assert_eq!(doc.extract_method, ExtractMethod::WordUndoc);
        assert!(doc.pages.is_empty());
        assert_eq!(doc.paragraphs.len(), 10);
        assert_eq!(doc.paragraphs[0].paragraph_index, 0);
        assert_eq!(doc.paragraphs[0].text, "关于召开年度工作会议的通知");
        assert_eq!(doc.paragraphs[9].text, "第十段");
    }

    #[test]
    fn doc_uses_converter_before_word_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let converted = dir.path().join("converted.docx");
        let services = test_services()
            .with_converted_doc(converted.clone())
            .with_docx_paragraphs(vec!["转换后的标题".into()]);
        let request = request(FileType::Doc, dir.path().join("legacy.doc"));

        let doc = extract_document_with_services(&request, &services.refs(), dir.path()).unwrap();

        assert_eq!(doc.source_type, FileType::Doc);
        assert_eq!(doc.extract_method, ExtractMethod::DocConvertedUndoc);
        assert_eq!(doc.paragraphs[0].text, "转换后的标题");
        assert_eq!(
            services.converter.last_input.borrow().as_deref(),
            Some(request.source_path.as_path())
        );
        assert_eq!(
            services.docx.last_path.borrow().as_deref(),
            Some(converted.as_path())
        );
    }

    #[test]
    fn doc_conversion_failure_maps_to_doc_convert_failed() {
        let dir = tempfile::tempdir().unwrap();
        let services = test_services().with_converter_error(app_error(
            ErrorCode::Internal,
            ErrorCategory::System,
            "LibreOffice did not produce converted DOCX",
            ProcessingStage::Extract,
        ));
        let request = request(FileType::Doc, dir.path().join("bad.doc"));

        let err = extract_document_with_services(&request, &services.refs(), dir.path())
            .expect_err("conversion should fail");

        assert_eq!(err.code, ErrorCode::DocConvertFailed);
        assert_eq!(err.category, ErrorCategory::Extraction);
        assert_eq!(err.stage, Some(ProcessingStage::Extract));
        assert!(err
            .technical_detail
            .as_deref()
            .unwrap()
            .contains("LibreOffice did not produce"));
    }

    #[test]
    fn pdf_maps_native_layout_blocks() {
        let services = test_services().with_pdf_pages(vec![RawPdfPage {
            page_index: 0,
            width: 600.0,
            height: 800.0,
            blocks: vec![RawTextBlock {
                text: "关于召开年度会议的通知".into(),
                x: 150.0,
                y: 80.0,
                width: 300.0,
                height: 40.0,
                font_size: Some(20.0),
                bold: Some(true),
                confidence: None,
                line_index: Some(0),
            }],
        }]);
        let dir = tempfile::tempdir().unwrap();
        let request = request(FileType::Pdf, dir.path().join("sample.pdf"));

        let doc = extract_document_with_services(&request, &services.refs(), dir.path()).unwrap();

        assert_eq!(doc.extract_method, ExtractMethod::PdfNativeLiteparse);
        assert_eq!(doc.pages.len(), 1);
        assert!(matches!(doc.pages[0].unit, SourceUnit::PdfPoint));
        let block = &doc.pages[0].blocks[0];
        assert_eq!(block.text, "关于召开年度会议的通知");
        assert_eq!(block.raw_bbox.as_ref().unwrap().x0, 150.0);
        assert_eq!(block.bbox.x0, 0.25);
        assert_eq!(block.bbox.y0, 0.1);
        assert_eq!(block.bbox.x1, 0.75);
        assert_eq!(block.bbox.y1, 0.15);
        assert_eq!(block.font_size, Some(20.0));
        assert_eq!(block.bold, Some(true));
        assert_eq!(block.line_index, Some(0));
    }

    #[test]
    fn pdf_empty_native_text_uses_ocr_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let image_path = dir.path().join("page-1.png");
        let services = test_services()
            .with_pdf_pages(vec![RawPdfPage {
                page_index: 0,
                width: 600.0,
                height: 800.0,
                blocks: vec![],
            }])
            .with_rasterized_pages(vec![RasterizedPage {
                page_index: 0,
                width: 1200,
                height: 1600,
                image_path: image_path.clone(),
            }])
            .with_ocr_pages(vec![OcrPage {
                page_index: 0,
                width: 1200,
                height: 1600,
                blocks: vec![ocr_block("扫描件标题", 0.2, 0.1, 0.8, 0.16, 88.0)],
            }]);
        let request = request(FileType::Pdf, dir.path().join("scan.pdf"));

        let doc = extract_document_with_services(&request, &services.refs(), dir.path()).unwrap();

        assert_eq!(doc.extract_method, ExtractMethod::PdfOcrFallbackTesseract);
        assert!(matches!(doc.pages[0].unit, SourceUnit::Pixel));
        assert_eq!(doc.pages[0].blocks[0].text, "扫描件标题");
        assert_eq!(
            services.rasterizer.last_pages.borrow().as_slice(),
            &[1, 2, 3]
        );
        assert_eq!(
            services.ocr.last_inputs.borrow()[0].image_path.as_path(),
            image_path.as_path()
        );
    }

    #[test]
    fn image_uses_ocr_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("image.png");
        let services = test_services().with_ocr_pages(vec![OcrPage {
            page_index: 0,
            width: 800,
            height: 600,
            blocks: vec![ocr_block("图片标题", 0.1, 0.2, 0.7, 0.3, 90.0)],
        }]);
        let request = request(FileType::Png, source.clone());

        let doc = extract_document_with_services(&request, &services.refs(), dir.path()).unwrap();

        assert_eq!(doc.source_type, FileType::Png);
        assert_eq!(doc.extract_method, ExtractMethod::ImageOcrTesseract);
        assert_eq!(doc.pages[0].width, 800.0);
        assert_eq!(doc.pages[0].height, 600.0);
        assert_eq!(doc.pages[0].blocks[0].ocr_confidence, Some(90.0));
        assert_eq!(services.ocr.last_inputs.borrow()[0].image_path, source);
    }

    #[test]
    fn unsupported_file_type_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let services = test_services();
        let request = request(FileType::Unsupported, dir.path().join("sheet.xlsx"));

        let err = extract_document_with_services(&request, &services.refs(), dir.path())
            .expect_err("unsupported type should fail");

        assert_eq!(err.code, ErrorCode::UnsupportedFormat);
        assert_eq!(err.category, ErrorCategory::Input);
        assert_eq!(err.stage, Some(ProcessingStage::Extract));
    }

    #[test]
    fn batch_temp_dir_cleans_up_on_drop() {
        let path = {
            let temp_dir = BatchTempDir::new("batch-001").unwrap();
            let marker = temp_dir.path().join("page.png");
            fs::write(&marker, b"png").unwrap();
            assert!(marker.exists());
            temp_dir.path().to_path_buf()
        };

        assert!(!path.exists());
    }

    #[test]
    fn debug_mode_saves_extracted_document() {
        let dir = tempfile::tempdir().unwrap();
        let diagnostics = Diagnostics::new(dir.path()).unwrap();
        let services = test_services().with_docx_paragraphs(vec!["调试标题".into()]);
        let request = request(FileType::Docx, dir.path().join("sample.docx"));
        let settings = Settings {
            debug_mode: true,
            ..Settings::default()
        };

        let doc = extract_document_with_diagnostics(
            &request,
            &services.refs(),
            dir.path(),
            &diagnostics,
            &settings,
        )
        .unwrap();

        assert!(doc
            .diagnostics_ref
            .as_deref()
            .unwrap()
            .starts_with("debug://batch-001/file-001/"));
        assert!(dir
            .path()
            .join("debug")
            .join("batch-001")
            .join("file-001")
            .join("extracted-document.json")
            .exists());
    }

    struct TestServices {
        docx: FakeDocx,
        converter: FakeDocConverter,
        pdf: FakePdf,
        rasterizer: FakeRasterizer,
        ocr: FakeOcr,
    }

    impl TestServices {
        fn refs(&self) -> ExtractionServices<'_> {
            ExtractionServices {
                docx: &self.docx,
                doc_converter: &self.converter,
                pdf: &self.pdf,
                rasterizer: &self.rasterizer,
                ocr: &self.ocr,
            }
        }

        fn with_docx_paragraphs(mut self, paragraphs: Vec<String>) -> Self {
            self.docx.paragraphs = paragraphs;
            self
        }

        fn with_converted_doc(mut self, path: PathBuf) -> Self {
            self.converter.converted = Some(ConvertedDoc {
                intermediate_path: path,
                format: ConvertedDocFormat::Docx,
            });
            self
        }

        fn with_converter_error(mut self, error: AppError) -> Self {
            self.converter.error = Some(error);
            self
        }

        fn with_pdf_pages(mut self, pages: Vec<RawPdfPage>) -> Self {
            self.pdf.pages = pages;
            self
        }

        fn with_rasterized_pages(mut self, pages: Vec<RasterizedPage>) -> Self {
            self.rasterizer.pages = pages;
            self
        }

        fn with_ocr_pages(mut self, pages: Vec<OcrPage>) -> Self {
            self.ocr.pages = pages;
            self
        }
    }

    fn test_services() -> TestServices {
        TestServices {
            docx: FakeDocx::default(),
            converter: FakeDocConverter::default(),
            pdf: FakePdf::default(),
            rasterizer: FakeRasterizer::default(),
            ocr: FakeOcr::default(),
        }
    }

    #[derive(Default)]
    struct FakeDocx {
        paragraphs: Vec<String>,
        last_path: RefCell<Option<PathBuf>>,
    }

    impl DocxTextExtractor for FakeDocx {
        fn extract_paragraphs(&self, path: &Path) -> Result<Vec<String>, AppError> {
            self.last_path.replace(Some(path.to_path_buf()));
            Ok(self.paragraphs.clone())
        }
    }

    #[derive(Default)]
    struct FakeDocConverter {
        converted: Option<ConvertedDoc>,
        error: Option<AppError>,
        last_input: RefCell<Option<PathBuf>>,
    }

    impl DocConverter for FakeDocConverter {
        fn convert(&self, input_path: &Path, _work_dir: &Path) -> Result<ConvertedDoc, AppError> {
            self.last_input.replace(Some(input_path.to_path_buf()));
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            Ok(self.converted.clone().unwrap_or_else(|| ConvertedDoc {
                intermediate_path: input_path.with_extension("docx"),
                format: ConvertedDocFormat::Docx,
            }))
        }
    }

    #[derive(Default)]
    struct FakePdf {
        pages: Vec<RawPdfPage>,
    }

    impl PdfTextExtractor for FakePdf {
        fn extract_pages(
            &self,
            _path: &Path,
            _pages: &[usize],
        ) -> Result<Vec<RawPdfPage>, AppError> {
            Ok(self.pages.clone())
        }
    }

    #[derive(Default)]
    struct FakeRasterizer {
        pages: Vec<RasterizedPage>,
        last_pages: RefCell<Vec<usize>>,
    }

    impl PdfRasterizer for FakeRasterizer {
        fn rasterize_pages(
            &self,
            _path: &Path,
            pages: &[usize],
            _work_dir: &Path,
        ) -> Result<Vec<RasterizedPage>, AppError> {
            self.last_pages.replace(pages.to_vec());
            Ok(self.pages.clone())
        }
    }

    #[derive(Default)]
    struct FakeOcr {
        pages: Vec<OcrPage>,
        last_inputs: RefCell<Vec<OcrPageInput>>,
    }

    impl OcrExtractor for FakeOcr {
        fn extract_pages(&self, pages: &[OcrPageInput]) -> Result<Vec<OcrPage>, AppError> {
            self.last_inputs.replace(pages.to_vec());
            Ok(self.pages.clone())
        }
    }

    fn request(file_type: FileType, source_path: PathBuf) -> ExtractRequest {
        ExtractRequest {
            batch_id: BatchId("batch-001".into()),
            file_job_id: FileJobId("file-001".into()),
            file_type,
            source_path,
        }
    }

    fn ocr_block(text: &str, x0: f32, y0: f32, x1: f32, y1: f32, confidence: f32) -> LayoutBlock {
        LayoutBlock {
            text: text.into(),
            bbox: NormalizedBox { x0, y0, x1, y1 },
            raw_bbox: None,
            font_size: None,
            bold: None,
            ocr_confidence: Some(confidence),
            line_index: Some(0),
        }
    }

    fn app_error(
        code: ErrorCode,
        category: ErrorCategory,
        detail: &str,
        stage: ProcessingStage,
    ) -> AppError {
        AppError {
            code,
            category,
            user_message: detail.into(),
            technical_detail: Some(detail.into()),
            retryable: false,
            file_path: None,
            stage: Some(stage),
        }
    }

    #[allow(dead_code)]
    fn extracted_page(blocks: Vec<LayoutBlock>) -> ExtractedPage {
        ExtractedPage {
            page_index: 0,
            width: 1.0,
            height: 1.0,
            unit: SourceUnit::Unknown,
            blocks,
        }
    }

    #[allow(dead_code)]
    fn raw_box(x0: f32, y0: f32, x1: f32, y1: f32) -> RawBox {
        RawBox { x0, y0, x1, y1 }
    }
}
