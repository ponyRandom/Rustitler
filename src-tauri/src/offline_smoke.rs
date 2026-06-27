use crate::errors::AppError;
#[cfg(feature = "extraction-ocr")]
use crate::extract::{self, OcrExtractor};
use crate::packaging::{resolve_soffice_path_for, resolve_tessdata_dir_for, RuntimeAssets};
use crate::{history, settings};
use serde::Serialize;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeTestConfig {
    pub resource_dir: PathBuf,
    pub app_data_dir: PathBuf,
    pub report_path: Option<PathBuf>,
    pub require_ocr: bool,
}

impl SmokeTestConfig {
    pub fn parse<I, S>(args: I) -> Result<Option<Self>, AppError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
        if args.is_empty() {
            return Ok(None);
        }
        args.remove(0);

        if args.first().and_then(|arg| arg.to_str()) != Some("--offline-smoke-test") {
            return Ok(None);
        }
        args.remove(0);

        let mut resource_dir = None;
        let mut app_data_dir = None;
        let mut report_path = None;
        let mut require_ocr = false;
        let mut index = 0;

        while index < args.len() {
            match args[index].to_str() {
                Some("--resource-dir") => {
                    index += 1;
                    resource_dir =
                        Some(PathBuf::from(args.get(index).ok_or_else(|| {
                            AppError::internal("--resource-dir requires a path")
                        })?));
                }
                Some("--app-data-dir") => {
                    index += 1;
                    app_data_dir =
                        Some(PathBuf::from(args.get(index).ok_or_else(|| {
                            AppError::internal("--app-data-dir requires a path")
                        })?));
                }
                Some("--report-path") => {
                    index += 1;
                    report_path =
                        Some(PathBuf::from(args.get(index).ok_or_else(|| {
                            AppError::internal("--report-path requires a path")
                        })?));
                }
                Some("--require-ocr") => {
                    require_ocr = true;
                }
                Some(other) => {
                    return Err(AppError::internal(format!(
                        "unknown offline smoke test argument: {other}"
                    )));
                }
                None => {
                    return Err(AppError::internal(
                        "offline smoke test arguments must be valid UTF-8",
                    ));
                }
            }
            index += 1;
        }

        Ok(Some(Self {
            resource_dir: resource_dir.unwrap_or_else(|| PathBuf::from("resources")),
            app_data_dir: app_data_dir.unwrap_or_else(|| PathBuf::from("rustitler-smoke-data")),
            report_path,
            require_ocr,
        }))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SmokeCheckStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmokeChecks {
    pub tessdata_present: bool,
    pub soffice_present: bool,
    pub settings_roundtrip: bool,
    pub history_database: bool,
    pub image_ocr: SmokeCheckStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmokeTestReport {
    pub status: &'static str,
    pub platform: String,
    pub resource_dir: PathBuf,
    pub app_data_dir: PathBuf,
    pub checks: SmokeChecks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceProbe {
    pub tessdata_dir: PathBuf,
    pub chi_sim_traineddata_present: bool,
    pub soffice_path: PathBuf,
    pub soffice_present: bool,
}

pub fn run_from_env() -> Result<bool, AppError> {
    let Some(config) = SmokeTestConfig::parse(std::env::args_os())? else {
        return Ok(false);
    };

    let report = run_smoke_test(&config)?;
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| AppError::internal(format!("serialize smoke report: {err}")))?;
    if let Some(report_path) = &config.report_path {
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                AppError::internal(format!(
                    "create smoke report dir '{}': {err}",
                    parent.display()
                ))
            })?;
        }
        fs::write(report_path, &json).map_err(|err| {
            AppError::internal(format!(
                "write smoke report '{}': {err}",
                report_path.display()
            ))
        })?;
    }
    println!("{json}");

    if report.status == "ok" {
        Ok(true)
    } else {
        Err(AppError::internal("offline smoke test failed"))
    }
}

pub fn run_smoke_test(config: &SmokeTestConfig) -> Result<SmokeTestReport, AppError> {
    let assets = RuntimeAssets::new(&config.resource_dir);
    let resources = probe_resources(&assets);
    fs::create_dir_all(&config.app_data_dir).map_err(|err| {
        AppError::internal(format!(
            "create smoke app data dir '{}': {err}",
            config.app_data_dir.display()
        ))
    })?;

    let settings_roundtrip = settings::save_settings(&config.app_data_dir, &Default::default())
        .and_then(|_| settings::load_settings(&config.app_data_dir))
        .is_ok();
    let history_database = history::open_history(&config.app_data_dir).is_ok();
    let image_ocr = run_image_ocr_check(&assets, config.require_ocr)?;

    let status = if resources.chi_sim_traineddata_present
        && settings_roundtrip
        && history_database
        && (!config.require_ocr || image_ocr == SmokeCheckStatus::Passed)
    {
        "ok"
    } else {
        "failed"
    };

    Ok(SmokeTestReport {
        status,
        platform: std::env::consts::OS.into(),
        resource_dir: config.resource_dir.clone(),
        app_data_dir: config.app_data_dir.clone(),
        checks: SmokeChecks {
            tessdata_present: resources.chi_sim_traineddata_present,
            soffice_present: resources.soffice_present,
            settings_roundtrip,
            history_database,
            image_ocr,
        },
    })
}

pub fn probe_resources(assets: &RuntimeAssets) -> ResourceProbe {
    let tessdata_dir = resolve_tessdata_dir_for(None, None, Some(assets));
    let soffice_path = resolve_soffice_path_for(None, Some(assets), &[]);
    ResourceProbe {
        chi_sim_traineddata_present: tessdata_dir.join("chi_sim.traineddata").is_file(),
        tessdata_dir,
        soffice_present: soffice_path.is_file(),
        soffice_path,
    }
}

#[cfg(feature = "extraction-ocr")]
fn run_image_ocr_check(
    assets: &RuntimeAssets,
    require_ocr: bool,
) -> Result<SmokeCheckStatus, AppError> {
    let tessdata_dir = resolve_tessdata_dir_for(None, None, Some(assets));
    if !tessdata_dir.join("chi_sim.traineddata").is_file() {
        return Ok(if require_ocr {
            SmokeCheckStatus::Failed
        } else {
            SmokeCheckStatus::Skipped
        });
    }

    let temp = tempfile::tempdir()
        .map_err(|err| AppError::internal(format!("create OCR smoke tempdir: {err}")))?;
    let image_path = temp.path().join("ocr-smoke.png");
    write_smoke_png(&image_path)?;

    let extractor = extract::TesseractOcrExtractor::new(tessdata_dir);
    let pages = match extractor.extract_pages(&[extract::OcrPageInput {
        page_index: 0,
        width: 0,
        height: 0,
        image_path,
    }]) {
        Ok(pages) => pages,
        Err(_) => return Ok(SmokeCheckStatus::Failed),
    };

    if pages
        .first()
        .map(|page| page.width > 0 && page.height > 0 && !page.blocks.is_empty())
        == Some(true)
    {
        Ok(SmokeCheckStatus::Passed)
    } else {
        Ok(SmokeCheckStatus::Failed)
    }
}

#[cfg(not(feature = "extraction-ocr"))]
fn run_image_ocr_check(
    _assets: &RuntimeAssets,
    require_ocr: bool,
) -> Result<SmokeCheckStatus, AppError> {
    Ok(if require_ocr {
        SmokeCheckStatus::Failed
    } else {
        SmokeCheckStatus::Skipped
    })
}

#[cfg(feature = "extraction-ocr")]
fn write_smoke_png(path: &std::path::Path) -> Result<(), AppError> {
    let mut image = image::RgbImage::from_pixel(240, 120, image::Rgb([255, 255, 255]));
    for origin_x in [20, 95, 170] {
        draw_digit_five(&mut image, origin_x, 18, 9);
    }
    image
        .save(path)
        .map_err(|err| AppError::internal(format!("write OCR smoke PNG: {err}")))
}

#[cfg(feature = "extraction-ocr")]
fn draw_digit_five(image: &mut image::RgbImage, origin_x: u32, origin_y: u32, scale: u32) {
    const DIGIT_FIVE: [&str; 7] = [
        "11111", "10000", "10000", "11110", "00001", "00001", "11110",
    ];

    for (row, pattern) in DIGIT_FIVE.iter().enumerate() {
        for (column, pixel) in pattern.chars().enumerate() {
            if pixel == '1' {
                fill_rect(
                    image,
                    origin_x + column as u32 * scale,
                    origin_y + row as u32 * scale,
                    scale,
                    scale,
                );
            }
        }
    }
}

#[cfg(feature = "extraction-ocr")]
fn fill_rect(image: &mut image::RgbImage, x: u32, y: u32, width: u32, height: u32) {
    let max_x = (x + width).min(image.width());
    let max_y = (y + height).min(image.height());
    for yy in y..max_y {
        for xx in x..max_x {
            image.put_pixel(xx, yy, image::Rgb([0, 0, 0]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packaging::RuntimeAssets;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn args_parse_smoke_test_paths_and_require_ocr() {
        let args = [
            "rustitler",
            "--offline-smoke-test",
            "--resource-dir",
            "/bundle/resources",
            "--app-data-dir",
            "/tmp/rustitler-smoke",
            "--report-path",
            "/tmp/rustitler-smoke-report.json",
            "--require-ocr",
        ];

        let config = SmokeTestConfig::parse(args).unwrap().unwrap();

        assert_eq!(config.resource_dir, PathBuf::from("/bundle/resources"));
        assert_eq!(config.app_data_dir, PathBuf::from("/tmp/rustitler-smoke"));
        assert_eq!(
            config.report_path,
            Some(PathBuf::from("/tmp/rustitler-smoke-report.json"))
        );
        assert!(config.require_ocr);
    }

    #[test]
    fn args_return_none_without_smoke_test_flag() {
        assert!(SmokeTestConfig::parse(["rustitler"]).unwrap().is_none());
    }

    #[test]
    fn resource_probe_reports_tessdata_and_libreoffice_paths() {
        let dir = tempfile::tempdir().unwrap();
        let resource_dir = dir.path().join("resources");
        let tessdata_dir = resource_dir.join("tessdata");
        let soffice_path = resource_dir
            .join("libreoffice")
            .join(crate::packaging::platform_soffice_relative_path());
        fs::create_dir_all(&tessdata_dir).unwrap();
        fs::create_dir_all(soffice_path.parent().unwrap()).unwrap();
        fs::write(tessdata_dir.join("chi_sim.traineddata"), b"traineddata").unwrap();
        fs::write(&soffice_path, b"soffice").unwrap();

        let resources = probe_resources(&RuntimeAssets::new(&resource_dir));

        assert_eq!(resources.tessdata_dir, tessdata_dir);
        assert!(resources.chi_sim_traineddata_present);
        assert_eq!(resources.soffice_path, soffice_path);
        assert!(resources.soffice_present);
    }

    #[test]
    fn smoke_report_serializes_status_and_checks() {
        let report = SmokeTestReport {
            status: "ok",
            platform: "test-os".into(),
            resource_dir: PathBuf::from("/resources"),
            app_data_dir: PathBuf::from("/data"),
            checks: SmokeChecks {
                tessdata_present: true,
                soffice_present: false,
                settings_roundtrip: true,
                history_database: true,
                image_ocr: SmokeCheckStatus::Skipped,
            },
        };

        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["status"], "ok");
        assert_eq!(value["checks"]["tessdataPresent"], true);
        assert_eq!(value["checks"]["sofficePresent"], false);
        assert_eq!(value["checks"]["imageOcr"], "skipped");
    }
}
