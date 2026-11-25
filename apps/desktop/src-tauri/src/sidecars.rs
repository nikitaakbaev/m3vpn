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
        user_core("xray").unwrap_or_else(|| self.root.join("bin/xray"))
    }

    pub fn singbox(&self) -> PathBuf {
        user_core("hiddify-sing-box").unwrap_or_else(|| self.root.join("bin/hiddify-sing-box"))
    }

    /// The front proxy must use the packaged build: it is patched to disable
    /// the Hiddify monitoring that can panic on the selector config. Updated
    /// user cores are for backends, not for the front.
    pub fn front_singbox(&self) -> PathBuf {
        self.root.join("bin/hiddify-sing-box")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn check(&self) -> Result<()> {
        let xray = self.xray();
        if !xray.exists() {
            bail!("xray binary is missing at {}", xray.display());
        }
        for asset in ["geoip.dat", "geosite.dat"] {
            if !self.assets_dir().join(asset).exists() {
                bail!("{asset} is missing in {}", self.assets_dir().display());
            }
        }
        Ok(())
    }

    pub fn check_singbox(&self) -> Result<()> {
        let binary = self.singbox();
        if !binary.exists() {
            bail!("hiddify-sing-box binary is missing at {}", binary.display());
        }
        Ok(())
    }

    pub fn check_front_singbox(&self) -> Result<()> {
        let binary = self.front_singbox();
        if !binary.exists() {
            bail!("front sing-box binary is missing at {}", binary.display());
        }
        Ok(())
    }

    pub fn version(&self) -> Result<String> {
        version_of(&self.xray()).context("run xray version")
    }

    pub fn singbox_version(&self) -> Result<String> {
        version_of(&self.singbox()).context("run hiddify-sing-box version")
    }
}

/// Cores updated through GitHub live in the user data directory and take
/// precedence over the copies shipped with the package.
fn user_core(name: &str) -> Option<PathBuf> {
    let path = dirs::data_dir()?.join("m3vpn/cores").join(name);
    path.metadata()
        .ok()
        .filter(|metadata| metadata.is_file() && metadata.len() > 0)
        .map(|_| path)
}

fn version_of(binary: &std::path::Path) -> Result<String> {
    let output = std::process::Command::new(binary).arg("version").output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().next().unwrap_or("unknown").to_string())
}
