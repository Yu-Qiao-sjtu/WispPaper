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
    /// Changed shortcuts by id; an empty key means the shortcut is removed.
    pub shortcuts: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { data_dir: String::new(), profiles: vec![], active_profile: 0, auto_update: true, appearance: String::new(), density: String::new(), shortcuts: BTreeMap::new() }
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
        std::fs::remove_dir_all(dir).ok();
    }
}
