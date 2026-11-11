//! settings.json: where the libraries live and the model profiles.

use crate::ai::Profile;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The folder holding every library; empty means the default.
    pub data_dir: String,
    pub profiles: Vec<Profile>,
    /// Index into `profiles` of the one questions go to.
    pub active_profile: usize,
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
        assert_eq!(s.data_root(Path::new("/d")), PathBuf::from("/d"));
        std::fs::remove_dir_all(dir).ok();
    }
}
