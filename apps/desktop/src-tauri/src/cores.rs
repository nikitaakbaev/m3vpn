use crate::paths::AppPaths;
use crate::sidecars::Sidecars;
use crate::update;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const XRAY_REPO: &str = "XTLS/Xray-core";
const SINGBOX_REPO: &str = "hiddify/hiddify-sing-box";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreVersion {
    pub kind: String,
    pub installed: Option<String>,
    pub latest: Option<String>,
    pub update_available: bool,
    pub checked_at: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct InstalledCore {
    version: String,
    sha256: String,
    installed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CoreManifest {
    #[serde(default)]
    xray: Option<InstalledCore>,
    #[serde(default)]
    singbox: Option<InstalledCore>,
}

struct ReleaseAsset {
    version: String,
    url: String,
    sha256: String,
}

pub fn cores_dir(paths: &AppPaths) -> PathBuf {
    paths.data_dir.join("cores")
}

/// Checks the latest GitHub release for each bundled core. A failed lookup for
/// one core never hides the version of the other one.
pub fn check(paths: &AppPaths, sidecars: &Sidecars) -> Vec<CoreVersion> {
    let checked_at = now_secs();
    ["xray", "singbox"]
        .into_iter()
        .map(|kind| {
            let installed = installed_version(paths, sidecars, kind);
            match latest_release(kind) {
                Ok(release) => {
                    let available = installed
                        .as_deref()
                        .map(|current| update::is_newer(&release.version, current))
                        .unwrap_or(true);
                    CoreVersion {
                        kind: kind.to_string(),
                        installed,
                        latest: Some(release.version),
                        update_available: available,
                        checked_at,
                        error: None,
                    }
                }
                Err(error) => CoreVersion {
                    kind: kind.to_string(),
                    installed,
                    latest: None,
                    update_available: false,
                    checked_at,
                    error: Some(format!("{error:#}")),
                },
            }
        })
        .collect()
}

pub fn update_core(paths: &AppPaths, kind: &str) -> Result<CoreVersion> {
    let release = latest_release(kind)?;
    let dir = cores_dir(paths);
    fs::create_dir_all(&dir)?;
    let temp = dir.join(format!(".tmp-{kind}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp)?;
    let result = (|| -> Result<String> {
        let archive = temp.join("archive");
        download(&release.url, &archive)?;
        let actual = sha256_file(&archive)?;
        if actual != release.sha256 {
            bail!("checksum mismatch for {kind}");
        }
        let binary = temp.join("binary");
        match kind {
            "xray" => extract_xray(&archive, &binary)?,
            "singbox" => extract_singbox(&archive, &binary)?,
            _ => bail!("unknown core: {kind}"),
        }
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
        let version = run_version(&binary)?
            .ok_or_else(|| anyhow::anyhow!("{kind} binary did not report a version"))?;
        let destination = dir.join(binary_name(kind));
        fs::rename(&binary, &destination)?;
        write_manifest(
            paths,
            kind,
            InstalledCore {
                version: version.clone(),
                sha256: actual,
                installed_at: now_secs(),
            },
        )?;
        Ok(version)
    })();
    let _ = fs::remove_dir_all(&temp);
    let version = result?;
    let installed = Some(version.clone());
    Ok(CoreVersion {
        kind: kind.to_string(),
        installed: installed.clone(),
        latest: Some(release.version.clone()),
        update_available: installed
            .as_deref()
            .map(|current| update::is_newer(&release.version, current))
            .unwrap_or(false),
        checked_at: now_secs(),
        error: None,
    })
}

fn binary_name(kind: &str) -> &'static str {
    match kind {
        "xray" => "xray",
        _ => "hiddify-sing-box",
    }
}

fn installed_version(paths: &AppPaths, sidecars: &Sidecars, kind: &str) -> Option<String> {
    let binary = match kind {
        "xray" => sidecars.xray(),
        _ => sidecars.singbox(),
    };
    if let Ok(Some(version)) = run_version(&binary) {
        return Some(version);
    }
    manifest(paths).and_then(|manifest| match kind {
        "xray" => manifest.xray.map(|core| core.version),
        _ => manifest.singbox.map(|core| core.version),
    })
}

fn run_version(binary: &Path) -> Result<Option<String>> {
    if !binary.exists() {
        return Ok(None);
    }
    let output = std::process::Command::new(binary)
        .arg("version")
        .output()
        .with_context(|| format!("run {}", binary.display()))?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(parse_version(&String::from_utf8_lossy(&output.stdout)))
}

fn parse_version(text: &str) -> Option<String> {
    text.lines().next()?.split_whitespace().find_map(|token| {
        let token = token.trim_start_matches('v');
        let first = token.chars().next()?;
        (first.is_ascii_digit() && token.contains('.')).then(|| token.to_string())
    })
}

fn manifest(paths: &AppPaths) -> Option<CoreManifest> {
    let text = fs::read_to_string(cores_dir(paths).join("manifest.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_manifest(paths: &AppPaths, kind: &str, core: InstalledCore) -> Result<()> {
    let dir = cores_dir(paths);
    fs::create_dir_all(&dir)?;
    let mut manifest = manifest(paths).unwrap_or_default();
    match kind {
        "xray" => manifest.xray = Some(core),
        _ => manifest.singbox = Some(core),
    }
    let temp = dir.join("manifest.tmp");
    fs::write(&temp, serde_json::to_string_pretty(&manifest)?)?;
    fs::rename(temp, dir.join("manifest.json"))?;
    Ok(())
}

fn latest_release(kind: &str) -> Result<ReleaseAsset> {
    let repo = match kind {
        "xray" => XRAY_REPO,
        "singbox" => SINGBOX_REPO,
        _ => bail!("unknown core: {kind}"),
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(format!("m3vpn/{}/Linux", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build core update client")?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("fetch core release")?;
    if !response.status().is_success() {
        bail!("GitHub API answered {}", response.status());
    }
    let release: Value = response.json().context("decode core release")?;
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("release has no tag"))?;
    let version = tag.trim_start_matches('v').to_string();
    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("release has no assets"))?;
    let asset = assets
        .iter()
        .find(|asset| {
            asset
                .get("name")
                .and_then(Value::as_str)
                .map(|name| asset_name_matches(kind, name))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("no Linux asset for {kind}"))?;
    let url = asset
        .get("browser_download_url")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("https://"))
        .ok_or_else(|| anyhow::anyhow!("asset has no download URL"))?
        .to_string();
    let sha256 = asset
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .map(str::to_ascii_lowercase)
        .filter(|digest| digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()))
        .ok_or_else(|| anyhow::anyhow!("asset has no sha256 digest"))?;
    Ok(ReleaseAsset {
        version,
        url,
        sha256,
    })
}

fn asset_name_matches(kind: &str, name: &str) -> bool {
    let arch = std::env::consts::ARCH;
    match kind {
        "xray" => match arch {
            "x86_64" => name == "Xray-linux-64.zip",
            "aarch64" => name == "Xray-linux-arm64-v8a.zip",
            _ => false,
        },
        "singbox" => {
            name.ends_with(match arch {
                "x86_64" => "-linux-amd64.tar.gz",
                "aarch64" => "-linux-arm64.tar.gz",
                _ => return false,
            }) && !name.contains("-glibc")
                && !name.contains("-musl")
        }
        _ => false,
    }
}

fn download(url: &str, destination: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent(format!("m3vpn/{}/Linux", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build core download client")?;
    let mut response = client
        .get(url)
        .send()
        .context("download core")?
        .error_for_status()
        .context("core download failed")?;
    let mut file = File::create(destination)?;
    std::io::copy(&mut response, &mut file)?;
    file.flush()?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn extract_xray(archive: &Path, destination: &Path) -> Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).context("open Xray zip")?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if Path::new(&name).file_name().and_then(|name| name.to_str()) == Some("xray") {
            let mut output = File::create(destination)?;
            std::io::copy(&mut entry, &mut output)?;
            return Ok(());
        }
    }
    bail!("xray binary not found in archive")
}

fn extract_singbox(archive: &Path, destination: &Path) -> Result<()> {
    let file = File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().context("read sing-box archive")? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path()?.to_path_buf();
        if path.file_name().and_then(|name| name.to_str()) == Some("sing-box") {
            let mut output = File::create(destination)?;
            std::io::copy(&mut entry, &mut output)?;
            return Ok(());
        }
    }
    bail!("sing-box binary not found in archive")
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|delta| delta.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_core_versions() {
        assert_eq!(
            parse_version("Xray 26.11.9 (Xray, Penetrates Everything.)"),
            Some("26.11.9".to_string())
        );
        assert_eq!(
            parse_version("hiddify-sing-box version 1.13.0.h5"),
            Some("1.13.0.h5".to_string())
        );
        assert_eq!(parse_version("hiddify-sing-box version unknown"), None);
    }

    #[test]
    #[ignore = "downloads the latest Xray release from GitHub"]
    fn installs_latest_xray() {
        let root = std::env::temp_dir().join(format!("m3vpn-core-xray-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = AppPaths {
            data_dir: root.clone(),
            db_path: root.join("test.db"),
            log_dir: root.join("logs"),
            core_dir: root.join("core"),
            xray_config: root.join("core/xray.json"),
            xray_log: root.join("core/xray.log"),
            singbox_config: root.join("core/singbox.json"),
            singbox_log: root.join("core/singbox.log"),
            front_config: root.join("core/front.json"),
            front_log: root.join("core/front.log"),
            geo_dir: root.join("geo"),
        };
        let result = update_core(&paths, "xray").expect("update xray");
        assert!(result.installed.is_some());
        let binary = cores_dir(&paths).join("xray");
        assert!(binary.exists());
        assert!(run_version(&binary).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[ignore = "downloads the latest sing-box release from GitHub"]
    fn installs_latest_singbox() {
        let root = std::env::temp_dir().join(format!("m3vpn-core-singbox-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = AppPaths {
            data_dir: root.clone(),
            db_path: root.join("test.db"),
            log_dir: root.join("logs"),
            core_dir: root.join("core"),
            xray_config: root.join("core/xray.json"),
            xray_log: root.join("core/xray.log"),
            singbox_config: root.join("core/singbox.json"),
            singbox_log: root.join("core/singbox.log"),
            front_config: root.join("core/front.json"),
            front_log: root.join("core/front.log"),
            geo_dir: root.join("geo"),
        };
        let result = update_core(&paths, "singbox").expect("update singbox");
        assert!(result.installed.is_some());
        let binary = cores_dir(&paths).join("hiddify-sing-box");
        assert!(binary.exists());
        assert!(run_version(&binary).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn selects_linux_assets() {
        assert!(asset_name_matches("xray", "Xray-linux-64.zip"));
        assert!(asset_name_matches(
            "singbox",
            "sing-box-1.13.0.h5-linux-amd64.tar.gz"
        ));
        assert!(!asset_name_matches(
            "singbox",
            "sing-box-1.13.0.h5-linux-amd64-glibc.tar.gz"
        ));
        assert!(!asset_name_matches("xray", "Xray-linux-64.zip.dgst"));
    }
}
