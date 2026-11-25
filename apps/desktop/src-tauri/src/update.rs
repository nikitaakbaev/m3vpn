use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtifact {
    pub kind: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub configured: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub artifacts: Vec<UpdateArtifact>,
    pub release_url: Option<String>,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn empty_info() -> UpdateInfo {
    UpdateInfo {
        configured: false,
        current_version: current_version().to_string(),
        latest_version: None,
        available: false,
        notes: None,
        published_at: None,
        artifacts: Vec::new(),
        release_url: None,
    }
}

pub fn check(repo: &str) -> Result<UpdateInfo> {
    let repo = validate_repo(repo)?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(Policy::none())
        .user_agent(format!("m3vpn/{}/Linux", current_version()))
        .build()
        .context("build update client")?;
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("fetch GitHub release")?;
    if !response.status().is_success() {
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("репозиторий или релиз не найден: {repo}");
        }
        if response.status() == reqwest::StatusCode::FORBIDDEN {
            bail!("GitHub API: лимит запросов или нет доступа к релизам");
        }
        bail!("GitHub API ответил {}", response.status());
    }
    let release: Value = response.json().context("decode GitHub release")?;
    parse_release(&release)
}

fn validate_repo(repo: &str) -> Result<String> {
    let repo = repo.trim().trim_matches('/');
    let Some((owner, name)) = repo.split_once('/') else {
        bail!("укажи репозиторий в формате owner/repo");
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    };
    if !valid(owner) || !valid(name) {
        bail!("некорректный GitHub-репозиторий: {repo}");
    }
    Ok(format!("{owner}/{name}"))
}

fn parse_release(release: &Value) -> Result<UpdateInfo> {
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("в релизе нет tag_name"))?;
    let latest = tag
        .trim_start_matches("m3vpn-")
        .trim_start_matches('v')
        .to_string();
    let notes = release
        .get("body")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let published_at = release
        .get("published_at")
        .and_then(Value::as_str)
        .map(str::to_string);
    let release_url = release
        .get("html_url")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("https://github.com/"))
        .map(str::to_string);

    let mut artifacts = Vec::new();
    if let Some(assets) = release.get("assets").and_then(Value::as_array) {
        for asset in assets {
            let Some(name) = asset.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(kind) = artifact_kind(name) else {
                continue;
            };
            let Some(url) = asset.get("browser_download_url").and_then(Value::as_str) else {
                continue;
            };
            if !url.starts_with("https://") {
                continue;
            }
            let Some(sha256) = asset
                .get("digest")
                .and_then(Value::as_str)
                .and_then(|digest| digest.strip_prefix("sha256:"))
                .map(str::to_ascii_lowercase)
                .filter(|value| {
                    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
                })
            else {
                continue;
            };
            artifacts.push(UpdateArtifact {
                kind: kind.to_string(),
                url: url.to_string(),
                sha256,
                size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
            });
        }
    }

    Ok(UpdateInfo {
        configured: true,
        current_version: current_version().to_string(),
        available: is_newer(&latest, current_version()),
        latest_version: Some(latest),
        notes,
        published_at,
        artifacts,
        release_url,
    })
}

fn artifact_kind(name: &str) -> Option<&'static str> {
    if !name.starts_with("m3vpn-") {
        return None;
    }
    if name.ends_with(".pkg.tar.zst") {
        Some("arch")
    } else if name.ends_with(".deb") {
        Some("deb")
    } else if name.ends_with(".rpm") {
        Some("rpm")
    } else if name.ends_with(".AppImage") {
        Some("appimage")
    } else if name.ends_with(".tar.gz") {
        Some("portable")
    } else {
        None
    }
}

fn is_newer(latest: &str, current: &str) -> bool {
    let latest = version_parts(latest);
    let current = version_parts(current);
    let length = latest.len().max(current.len());
    for index in 0..length {
        let left = latest.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .take(4)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn validates_github_repository_names() {
        assert_eq!(validate_repo("owner/m3vpn").unwrap(), "owner/m3vpn");
        assert_eq!(validate_repo("/owner/m3vpn/").unwrap(), "owner/m3vpn");
        assert!(validate_repo("owner").is_err());
        assert!(validate_repo("owner/repo/extra").is_err());
        assert!(validate_repo("https://github.com/owner/repo").is_err());
    }

    #[test]
    fn parses_github_release_assets_and_digests() {
        let release = serde_json::json!({
            "tag_name": "v0.2.0",
            "published_at": "2026-12-01T12:00:00Z",
            "body": "TUN fixes",
            "html_url": "https://github.com/owner/m3vpn/releases/tag/v0.2.0",
            "assets": [
                {
                    "name": "m3vpn-0.2.0-1-x86_64.pkg.tar.zst",
                    "browser_download_url": "https://github.com/owner/m3vpn/releases/download/v0.2.0/m3vpn-0.2.0-1-x86_64.pkg.tar.zst",
                    "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "size": 123
                },
                {
                    "name": "m3vpn-0.2.0-linux-x86_64.tar.gz",
                    "browser_download_url": "https://github.com/owner/m3vpn/releases/download/v0.2.0/m3vpn-0.2.0-linux-x86_64.tar.gz",
                    "digest": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "size": 456
                },
                {
                    "name": "source.tar.gz",
                    "browser_download_url": "https://github.com/owner/m3vpn/releases/download/v0.2.0/source.tar.gz",
                    "digest": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                    "size": 1
                }
            ]
        });
        let info = parse_release(&release).expect("release");
        assert!(info.available);
        assert_eq!(info.latest_version.as_deref(), Some("0.2.0"));
        assert_eq!(info.artifacts.len(), 2);
        assert_eq!(info.artifacts[0].kind, "arch");
        assert_eq!(info.artifacts[1].kind, "portable");
        assert!(info.release_url.is_some());
    }
}
