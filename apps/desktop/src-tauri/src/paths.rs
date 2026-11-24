use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub log_dir: PathBuf,
}

impl AppPaths {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .context("could not resolve XDG data directory")?
            .join("m3vpn");
        let log_dir = data_dir.join("logs");
        let db_path = data_dir.join("m3vpn.db");
        Ok(Self {
            data_dir,
            db_path,
            log_dir,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.log_dir)?;
        restrict(&self.data_dir, 0o700)?;
        restrict(&self.log_dir, 0o700)?;
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
