//! 检查更新: which release is the newest. The app asks GitHub for
//! `releases/latest`, which answers with a redirect to the newest release's
//! page; the version is read off that address, so nothing is downloaded and
//! nothing about the user is sent. (The JSON API would say the same, but it
//! is rate-limited per address and fails behind a shared one.)

pub const PROJECT_URL: &str = "https://github.com/Yu-Qiao-sjtu/WispPaper";
pub const RELEASES_URL: &str = "https://github.com/Yu-Qiao-sjtu/WispPaper/releases";

/// The address whose redirect names the newest release.
pub fn latest_url() -> String {
    format!("{RELEASES_URL}/latest")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// The release's page, where its packages are downloaded.
    pub url: String,
}

fn parts(version: &str) -> Option<Vec<u64>> {
    version.split('.').map(|p| p.parse().ok()).collect()
}

/// `a` is a later version than `b` (dotted numbers; a missing part is 0).
pub fn newer(a: &str, b: &str) -> bool {
    let (Some(mut a), Some(mut b)) = (parts(a), parts(b)) else { return false };
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

/// The release a `releases/latest` redirect points to. `None` when it points
/// anywhere else: a project without releases redirects to the release list,
/// and only this project's own release pages are ever offered to open.
pub fn release_from_location(location: &str) -> Option<Release> {
    let tag = location.trim().strip_prefix(RELEASES_URL)?.strip_prefix("/tag/")?;
    let version = tag.strip_prefix('v').unwrap_or(tag);
    parts(version)?;
    Some(Release { version: version.to_string(), url: format!("{RELEASES_URL}/tag/{tag}") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(newer("0.2.0", "0.1.9") && newer("0.10.0", "0.9.9") && newer("1.0", "0.9.9") && newer("0.1.0.1", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.0") && !newer("0.1", "0.1.0") && !newer("0.1.0", "0.2.0"));
        // Not a dotted number: never offered.
        assert!(!newer("0.2.0-beta", "0.1.0") && !newer("", "0.1.0"));
    }

    #[test]
    fn redirects() {
        let release = release_from_location("https://github.com/Yu-Qiao-sjtu/WispPaper/releases/tag/v0.2.0").unwrap();
        assert_eq!((release.version.as_str(), release.url.as_str()), ("0.2.0", "https://github.com/Yu-Qiao-sjtu/WispPaper/releases/tag/v0.2.0"));
        assert_eq!(release_from_location("https://github.com/Yu-Qiao-sjtu/WispPaper/releases/tag/1.3").unwrap().version, "1.3");
        // No release yet, another project, a tag that is not a version.
        for location in ["https://github.com/xuzhougeng/WispPaper/releases/tag/v9.9.9", "https://github.com/Yu-Qiao-sjtu/WispPaper/releases", "https://github.com/someone/else/releases/tag/v9.0.0", "https://github.com/Yu-Qiao-sjtu/WispPaper/releases/tag/nightly", "https://github.com/Yu-Qiao-sjtu/WispPaper/releases/tag/v1.0/../../x", ""] {
            assert_eq!(release_from_location(location), None, "{location}");
        }
    }
}
