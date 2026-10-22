//! 새 버전 확인: GitHub 의 최신 릴리스(main 에 머지될 때마다 자동으로 올라간다)와 지금 버전을 비교한다.
//! 한 시간에 한 번만 묻고 결과는 설정 폴더의 update-check.json 에 둔다. DBTOOL_NO_UPDATE_CHECK=1 이면 묻지 않는다.
//! HTTP 는 curl 로 한다 (macOS·Linux 기본 도구, 의존성 추가 없이)
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const REPO: &str = "Hkhyyy/dbtool";
const CACHE_FILE: &str = "update-check.json";
const CHECK_EVERY_SECS: u64 = 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// "0.1.2" (태그의 v 를 뗀 것)
    pub version: String,
    /// 릴리스 페이지
    pub url: String,
    /// 릴리스 노트 (마크다운)
    pub notes: String,
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn asset(&self, prefix: &str, suffix: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name.starts_with(prefix) && a.name.ends_with(suffix))
    }
}

#[derive(Serialize, Deserialize)]
struct Cache {
    checked_at: u64,
    latest: Option<Release>,
}

pub fn disabled() -> bool {
    std::env::var("DBTOOL_NO_UPDATE_CHECK").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// "0.1.10" > "0.1.9" 처럼 숫자로 비교한다. 못 읽으면 새 버전이 아닌 것으로
pub fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u64>> { s.trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect() };
    match (parse(current), parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// GitHub API 로 최신 릴리스를 읽는다
pub async fn latest_release() -> AppResult<Release> {
    let out = tokio::process::Command::new("curl")
        .args(["-fsSL", "-m", "10", "-H", "Accept: application/vnd.github+json", &format!("https://api.github.com/repos/{REPO}/releases/latest")])
        .output()
        .await
        .map_err(|e| AppError::Msg(format!("curl: {e}")))?;
    if !out.status.success() {
        return Err(AppError::Msg(format!("Update check failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    parse_release(&out.stdout)
}

fn parse_release(json: &[u8]) -> AppResult<Release> {
    let v: serde_json::Value = serde_json::from_slice(json)?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let assets = v
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().map(|x| Asset { name: x["name"].as_str().unwrap_or("").into(), url: x["browser_download_url"].as_str().unwrap_or("").into() }).collect())
        .unwrap_or_default();
    let version = s("tag_name").trim_start_matches('v').to_string();
    if version.is_empty() {
        return Err(AppError::Msg("Update check failed: no release found".into()));
    }
    Ok(Release { version, url: s("html_url"), notes: s("body"), assets })
}

/// 새 버전이 있으면 Some. force 가 아니면 한 시간에 한 번만 네트워크로 묻고 그 사이엔 저장해 둔 결과를 쓴다
pub async fn check(dir: &Path, current: &str, force: bool) -> AppResult<Option<Release>> {
    let path = dir.join(CACHE_FILE);
    let cached: Option<Cache> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
    let latest = match cached {
        Some(c) if !force && now().saturating_sub(c.checked_at) < CHECK_EVERY_SECS => c.latest,
        _ => {
            let r = latest_release().await?;
            let _ = std::fs::write(&path, serde_json::to_string(&Cache { checked_at: now(), latest: Some(r.clone()) })?);
            Some(r)
        }
    };
    Ok(latest.filter(|r| is_newer(current, &r.version)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.9", "0.1.10"));
        assert!(is_newer("0.1.10", "0.2.0"));
        assert!(is_newer("0.1.0", "v1.0.0"));
        assert!(!is_newer("0.1.1", "0.1.1"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "garbage"));
    }

    #[test]
    fn parses_github_release() {
        let json = br#"{"tag_name":"v0.1.2","html_url":"https://github.com/Hkhyyy/dbtool/releases/tag/v0.1.2","body":"notes",
            "assets":[{"name":"dbtool_0.1.2_aarch64.dmg","browser_download_url":"https://x/dmg"},
                      {"name":"dbtool-cli_0.1.2_aarch64-apple-darwin.tar.gz","browser_download_url":"https://x/tgz"}]}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.version, "0.1.2");
        assert_eq!(r.asset("dbtool_", ".dmg").unwrap().url, "https://x/dmg");
        assert_eq!(r.asset("dbtool-cli_", "aarch64-apple-darwin.tar.gz").unwrap().url, "https://x/tgz");
    }

    #[tokio::test]
    async fn cached_result_is_used_within_an_hour() {
        let dir = std::env::temp_dir().join(format!("dbtool-upd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let rel = Release { version: "9.9.9".into(), url: "u".into(), notes: String::new(), assets: vec![] };
        std::fs::write(dir.join(CACHE_FILE), serde_json::to_string(&Cache { checked_at: now(), latest: Some(rel.clone()) }).unwrap()).unwrap();
        // 네트워크 없이 저장된 결과로 답한다
        assert_eq!(check(&dir, "0.1.0", false).await.unwrap(), Some(rel));
        assert_eq!(check(&dir, "9.9.9", false).await.unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
