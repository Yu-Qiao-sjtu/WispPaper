//! settings.json: where the libraries live, the model profiles, and the
//! choices made on the settings pages.

use crate::ai::Profile;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The folder holding every library; empty means the default.
    pub data_dir: String,
    pub profiles: Vec<Profile>,
    /// Index into `profiles` of the one questions go to.
    pub active_profile: usize,
    /// Look for a newer release after launch and twice a day.
    pub auto_update: bool,
    /// "light" or "dark"; empty follows the system.
    pub appearance: String,
    /// "compact" tightens the paper table; empty is the standard rows.
    pub density: String,
    /// The proxy every request goes through, as a URL (`http://127.0.0.1:7890`,
    /// `socks5://…`); empty leaves it to the environment (`HTTPS_PROXY` and the like).
    pub proxy: String,
    /// Changed shortcuts by id; an empty key means the shortcut is removed.
    pub shortcuts: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { data_dir: String::new(), profiles: vec![], active_profile: 0, auto_update: true, appearance: String::new(), density: String::new(), proxy: String::new(), shortcuts: BTreeMap::new() }
    }
}

/// `WISP_PAPER_HOME` keeps the data and the settings of tests and
/// side-by-side builds in one folder of their own.
fn home_override() -> Option<PathBuf> {
    std::env::var("WISP_PAPER_HOME").ok().filter(|home| !home.trim().is_empty()).map(PathBuf::from)
}

/// Where the libraries live unless the settings name another folder.
pub fn default_root() -> PathBuf {
    home_override().unwrap_or_else(|| dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from(".")).join("WispPaper"))
}

pub fn settings_file() -> PathBuf {
    match home_override() {
        Some(home) => home.join("settings.json"),
        None => dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("WispPaper").join("settings.json"),
    }
}

/// The schemes a proxy address may have.
pub const PROXY_SCHEMES: [&str; 6] = ["http", "https", "socks4", "socks4a", "socks5", "socks5h"];

/// What is wrong with a proxy address, if anything. Empty is fine: it means
/// "not set". `direct` (no proxy at all) is only meaningful for one model
/// service, so it is accepted only where `allow_direct` says so.
pub fn proxy_problem(value: &str, allow_direct: bool) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || (allow_direct && value.eq_ignore_ascii_case("direct")) { return None }
    let Some((scheme, rest)) = value.split_once("://") else { return Some("代理地址要写完整，例如 http://127.0.0.1:7890 或 socks5://127.0.0.1:1080。".into()) };
    if !PROXY_SCHEMES.contains(&scheme.to_lowercase().as_str()) { return Some(format!("不支持 {scheme}:// 代理。可用：{}。", PROXY_SCHEMES.join("、"))) }
    // What stands after any `user:password@`, up to the port or a path.
    let host = rest.rsplit('@').next().unwrap_or("").split(['/', ':']).next().unwrap_or("");
    if host.is_empty() || value.contains(char::is_whitespace) { return Some("代理地址里缺少主机名，或者含有空格。".into()) }
    None
}

impl Settings {
    /// A missing file is the defaults; a broken one is an error, so it is
    /// never silently overwritten with defaults.
    pub fn load(path: &Path) -> Result<Settings> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).with_context(|| format!("设置文件格式不正确：{}", path.display())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(err) => Err(err).with_context(|| format!("无法读取设置文件 {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)? }
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&temporary, path).with_context(|| format!("无法写入设置文件 {}", path.display()))?;
        Ok(())
    }

    pub fn profile(&self) -> Option<&Profile> {
        self.profiles.get(self.active_profile).or(self.profiles.first())
    }

    /// The profile questions go to; before one is saved, the default one,
    /// which works as soon as its API key is in the environment.
    pub fn profile_or_default(&self) -> Profile {
        self.profile().cloned().unwrap_or_default()
    }

    /// The folder holding the libraries: the configured one, else `default`.
    pub fn data_root(&self, default: &Path) -> PathBuf {
        if self.data_dir.trim().is_empty() { default.to_path_buf() } else { PathBuf::from(self.data_dir.trim()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_save() {
        let dir = std::env::temp_dir().join(format!("wisp-settings-{}", crate::model::new_id("T")));
        let path = dir.join("settings.json");
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
        let mut s = Settings::default();
        s.profiles.push(Profile::default());
        s.active_profile = 0;
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
        std::fs::write(&path, "{broken").unwrap();
        assert!(Settings::load(&path).is_err());
        // A file from before a setting existed takes that setting's default.
        std::fs::write(&path, r#"{"dataDir":"/x","shortcuts":{"search":""}}"#).unwrap();
        let old = Settings::load(&path).unwrap();
        assert!(old.auto_update && old.appearance.is_empty());
        assert_eq!((old.data_dir.as_str(), old.shortcuts.get("search").map(String::as_str)), ("/x", Some("")));
        assert_eq!(s.data_root(Path::new("/d")), PathBuf::from("/d"));
        assert!(old.proxy.is_empty());
        // A profile saved before proxies existed has none of its own.
        std::fs::write(&path, r#"{"proxy":"socks5://127.0.0.1:1080","profiles":[{"name":"x","protocol":"openai","baseUrl":"http://h/v1","apiKey":"","model":"m"}]}"#).unwrap();
        let with = Settings::load(&path).unwrap();
        assert_eq!((with.proxy.as_str(), with.profiles[0].proxy.as_str()), ("socks5://127.0.0.1:1080", ""));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn proxy_addresses() {
        for good in ["", "  ", "http://127.0.0.1:7890", "HTTPS://proxy.example", "socks5://user:pass@10.0.0.1:1080", "socks5h://localhost:1080/"] {
            assert_eq!(proxy_problem(good, false), None, "{good}");
        }
        for bad in ["127.0.0.1:7890", "ftp://x", "http://", "http://:8080", "http://a b:1", "direct"] {
            assert!(proxy_problem(bad, false).is_some(), "{bad}");
        }
        assert_eq!(proxy_problem(" Direct ", true), None);
    }
}
