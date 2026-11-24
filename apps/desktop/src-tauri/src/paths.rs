use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub log_dir: PathBuf,
    pub core_dir: PathBuf,
    pub xray_config: PathBuf,
    pub xray_log: PathBuf,
    pub singbox_config: PathBuf,
    pub singbox_log: PathBuf,
    pub front_config: PathBuf,
    pub front_log: PathBuf,
}

impl AppPaths {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .context("could not resolve XDG data directory")?
            .join("m3vpn");
        let log_dir = data_dir.join("logs");
        let core_dir = data_dir.join("core");
        let db_path = data_dir.join("m3vpn.db");
        Ok(Self {
            xray_config: core_dir.join("xray.json"),
            xray_log: core_dir.join("xray.log"),
            singbox_config: core_dir.join("singbox.json"),
            singbox_log: core_dir.join("singbox.log"),
            front_config: core_dir.join("front.json"),
            front_log: core_dir.join("front.log"),
            data_dir,
            db_path,
            log_dir,
            core_dir,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.log_dir)?;
        std::fs::create_dir_all(&self.core_dir)?;
        restrict(&self.data_dir, 0o700)?;
        restrict(&self.log_dir, 0o700)?;
        restrict(&self.core_dir, 0o700)?;
        Ok(())
    }
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
