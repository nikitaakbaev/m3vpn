use anyhow::{bail, Context, Result};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Sidecars {
    pub root: PathBuf,
}

impl Sidecars {
    pub fn discover() -> Result<Self> {
        if let Ok(root) = std::env::var("M3VPN_SIDECARS") {
            let root = PathBuf::from(root);
            if root.join("bin/xray").exists() {
                return Ok(Self { root });
            }
            bail!(
                "M3VPN_SIDECARS is set but {} does not exist",
                root.display()
            );
        }

        let candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../sidecars"),
            PathBuf::from("/usr/lib/m3vpn/sidecars"),
            PathBuf::from("/usr/share/m3vpn/sidecars"),
        ];

        for root in candidates {
            if root.join("bin/xray").exists() {
                return Ok(Self { root });
            }
        }

        bail!("sidecars not found; run scripts/build-sidecars.sh or set M3VPN_SIDECARS")
    }

    pub fn xray(&self) -> PathBuf {
        self.root.join("bin/xray")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn check(&self) -> Result<()> {
        let xray = self.xray();
        if !xray.exists() {
            bail!("xray binary is missing at {}", xray.display());
        }
        if !self.assets_dir().join("geoip.dat").exists() {
            bail!("geoip.dat is missing in {}", self.assets_dir().display());
        }
        Ok(())
    }

    pub fn version(&self) -> Result<String> {
        let output = std::process::Command::new(self.xray())
            .arg("version")
            .output()
            .context("run xray version")?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text.lines().next().unwrap_or("unknown").to_string())
    }
}
