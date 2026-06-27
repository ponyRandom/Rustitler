use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAssets {
    resource_dir: PathBuf,
}

impl RuntimeAssets {
    pub fn new(resource_dir: impl Into<PathBuf>) -> Self {
        Self {
            resource_dir: resource_dir.into(),
        }
    }

    pub fn resource_dir(&self) -> &Path {
        &self.resource_dir
    }
}

pub fn runtime_assets_from_resource_dir(resource_dir: impl Into<PathBuf>) -> RuntimeAssets {
    RuntimeAssets::new(resource_dir)
}

pub fn resolve_tessdata_dir(assets: Option<&RuntimeAssets>) -> PathBuf {
    resolve_tessdata_dir_for(
        std::env::var_os("RUSTITLER_TESSDATA").map(PathBuf::from),
        std::env::var_os("TESSDATA_PREFIX").map(PathBuf::from),
        assets,
    )
}

pub fn resolve_tessdata_dir_for(
    explicit_override: Option<PathBuf>,
    tessdata_prefix: Option<PathBuf>,
    assets: Option<&RuntimeAssets>,
) -> PathBuf {
    explicit_override
        .or(tessdata_prefix)
        .or_else(|| assets.map(|assets| assets.resource_dir.join("tessdata")))
        .unwrap_or_else(default_platform_tessdata_dir)
}

pub fn resolve_soffice_path(assets: Option<&RuntimeAssets>) -> PathBuf {
    let candidates = system_soffice_candidates();
    resolve_soffice_path_for(
        std::env::var_os("RUSTITLER_SOFFICE").map(PathBuf::from),
        assets,
        &candidates,
    )
}

pub fn resolve_soffice_path_for(
    explicit_override: Option<PathBuf>,
    assets: Option<&RuntimeAssets>,
    system_candidates: &[PathBuf],
) -> PathBuf {
    if let Some(path) = explicit_override {
        return path;
    }

    if let Some(assets) = assets {
        let bundled = assets
            .resource_dir
            .join("libreoffice")
            .join(platform_soffice_relative_path());
        if bundled.exists() || system_candidates.is_empty() {
            return bundled;
        }
    }

    system_candidates
        .iter()
        .find(|path| path.exists() || path.as_os_str() == "soffice")
        .cloned()
        .unwrap_or_else(|| PathBuf::from("soffice"))
}

pub fn platform_soffice_relative_path() -> &'static str {
    if cfg!(target_os = "macos") {
        "LibreOffice.app/Contents/MacOS/soffice"
    } else if cfg!(target_os = "windows") {
        "program/soffice.exe"
    } else {
        "program/soffice"
    }
}

fn default_platform_tessdata_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| {
                home.join("Library")
                    .join("Application Support")
                    .join("tesseract-rs")
                    .join("tessdata")
            })
            .unwrap_or_else(|| PathBuf::from("tessdata"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join("AppData").join("Roaming"))
            })
            .map(|roaming| roaming.join("tesseract-rs").join("tessdata"))
            .unwrap_or_else(|| PathBuf::from("tessdata"))
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".tesseract-rs").join("tessdata"))
            .unwrap_or_else(|| PathBuf::from("tessdata"))
    }
}

fn system_soffice_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/soffice"),
        PathBuf::from("/usr/local/bin/soffice"),
        PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice"),
    ];
    if cfg!(target_os = "windows") {
        candidates.extend([
            PathBuf::from(r"C:\Program Files\LibreOffice\program\soffice.exe"),
            PathBuf::from(r"C:\Program Files (x86)\LibreOffice\program\soffice.exe"),
        ]);
    }
    candidates.push(PathBuf::from("soffice"));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_assets_exposes_resource_dir() {
        let assets = RuntimeAssets::new("/tmp/resources");

        assert_eq!(assets.resource_dir(), Path::new("/tmp/resources"));
    }

    #[test]
    fn tessdata_resolution_prefers_explicit_override_then_env_then_bundle() {
        let assets = RuntimeAssets::new("/app/resources");

        assert_eq!(
            resolve_tessdata_dir_for(
                Some(PathBuf::from("/override/tessdata")),
                Some(PathBuf::from("/env/tessdata")),
                Some(&assets),
            ),
            PathBuf::from("/override/tessdata")
        );
        assert_eq!(
            resolve_tessdata_dir_for(None, Some(PathBuf::from("/env/tessdata")), Some(&assets)),
            PathBuf::from("/env/tessdata")
        );
        assert_eq!(
            resolve_tessdata_dir_for(None, None, Some(&assets)),
            PathBuf::from("/app/resources").join("tessdata")
        );
    }

    #[test]
    fn soffice_resolution_prefers_explicit_override_then_bundle_then_system_candidates() {
        let assets = RuntimeAssets::new("/app/resources");
        let dir = tempfile::tempdir().unwrap();
        let system_soffice = dir.path().join("soffice");
        std::fs::write(&system_soffice, b"").unwrap();
        let candidates = [system_soffice.clone()];

        assert_eq!(
            resolve_soffice_path_for(
                Some(PathBuf::from("/override/soffice")),
                Some(&assets),
                &candidates
            ),
            PathBuf::from("/override/soffice")
        );
        assert_eq!(
            resolve_soffice_path_for(None, Some(&assets), &[]),
            PathBuf::from("/app/resources")
                .join("libreoffice")
                .join(platform_soffice_relative_path())
        );
        assert_eq!(
            resolve_soffice_path_for(None, Some(&assets), &candidates),
            system_soffice
        );
    }
}
