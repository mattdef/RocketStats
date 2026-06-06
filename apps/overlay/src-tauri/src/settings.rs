use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DEFAULT_LOG_PATH: &str = "~/.local/share/RocketStats/Launch.log";
const DEFAULT_APP_LOG_DIR: &str = "~/.local/share/RocketStats/logs";
const SETTINGS_PATH: &str = "~/.config/rocketstats/settings.json";
const DEFAULT_OPACITY: f64 = 0.9;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub log_path: String,
    pub app_log_dir: String,
    pub opacity: f64,
    pub always_on_top: bool,
    pub click_through: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            log_path: DEFAULT_LOG_PATH.to_owned(),
            app_log_dir: DEFAULT_APP_LOG_DIR.to_owned(),
            opacity: DEFAULT_OPACITY,
            always_on_top: true,
            click_through: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        Self::load_from_path(settings_path())
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to_path(settings_path())
    }

    pub fn resolved_log_path(&self) -> PathBuf {
        expand_home(&self.log_path)
    }

    pub fn resolved_app_log_dir(&self) -> PathBuf {
        expand_home(&self.app_log_dir)
    }

    fn load_from_path(path: PathBuf) -> Self {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(_) => return Self::default(),
        };

        let mut settings = match serde_json::from_str::<Self>(&contents) {
            Ok(settings) => settings,
            Err(_) => return Self::default(),
        };

        settings.opacity = normalize_opacity(settings.opacity);
        settings
    }

    fn save_to_path(&self, path: PathBuf) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut settings = self.clone();
        settings.opacity = normalize_opacity(settings.opacity);

        let contents = serde_json::to_string_pretty(&settings).map_err(std::io::Error::other)?;
        fs::write(path, contents)
    }
}

fn settings_path() -> PathBuf {
    expand_home(SETTINGS_PATH)
}

fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(stripped) => home_dir().join(stripped),
        None if path == "~" => home_dir(),
        None => PathBuf::from(path),
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn normalize_opacity(opacity: f64) -> f64 {
    if opacity.is_finite() {
        opacity.clamp(0.1, 1.0)
    } else {
        DEFAULT_OPACITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_values_match_expected_backend_defaults() {
        assert_eq!(
            Settings::default(),
            Settings {
                log_path: DEFAULT_LOG_PATH.to_owned(),
                app_log_dir: DEFAULT_APP_LOG_DIR.to_owned(),
                opacity: DEFAULT_OPACITY,
                always_on_top: true,
                click_through: true,
            }
        );
    }

    #[test]
    fn load_returns_default_when_file_is_missing() {
        let path = tempdir().unwrap().path().join("missing.json");

        assert_eq!(Settings::load_from_path(path), Settings::default());
    }

    #[test]
    fn load_clamps_opacity_from_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "log_path": "~/custom.log",
                "app_log_dir": "~/custom-app-logs",
                "opacity": 4.2,
                "always_on_top": false,
                "click_through": false
            }"#,
        )
        .unwrap();

        let settings = Settings::load_from_path(path);

        assert_eq!(settings.log_path, "~/custom.log");
        assert_eq!(settings.app_log_dir, "~/custom-app-logs");
        assert_eq!(settings.opacity, 1.0);
        assert!(!settings.always_on_top);
        assert!(!settings.click_through);
    }

    #[test]
    fn save_creates_parent_directories_and_writes_clamped_values() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rocketstats/settings.json");
        let settings = Settings {
            log_path: "~/saved.log".to_owned(),
            app_log_dir: "~/saved-app-logs".to_owned(),
            opacity: 0.0,
            always_on_top: false,
            click_through: false,
        };

        settings.save_to_path(path.clone()).unwrap();

        let saved = Settings::load_from_path(path);
        assert_eq!(saved.log_path, "~/saved.log");
        assert_eq!(saved.app_log_dir, "~/saved-app-logs");
        assert_eq!(saved.opacity, 0.1);
        assert!(!saved.always_on_top);
        assert!(!saved.click_through);
    }
}
