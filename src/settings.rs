//! Einstellungen der App und ihre Ablage unter `%APPDATA%\windows-resizer`.

use std::path::PathBuf;

use eframe::egui::ThemePreference;

use crate::i18n::Lang;

pub struct Settings {
    pub theme: ThemePreference,
    pub language: Lang,
}

impl Settings {
    /// Gespeicherte Einstellungen; was fehlt oder unlesbar ist, folgt dem System.
    pub fn load() -> Self {
        let mut settings = Self {
            theme: ThemePreference::System,
            language: Lang::system(),
        };
        let text = path().and_then(|path| std::fs::read_to_string(path).ok());
        for line in text.as_deref().unwrap_or_default().lines() {
            match line.split_once('=') {
                Some(("theme", value)) => {
                    settings.theme = match value {
                        "light" => ThemePreference::Light,
                        "dark" => ThemePreference::Dark,
                        _ => ThemePreference::System,
                    }
                }
                Some(("language", value)) => {
                    if let Some(language) = Lang::from_code(value) {
                        settings.language = language;
                    }
                }
                _ => {}
            }
        }
        settings
    }

    /// Schlägt das Speichern fehl, gelten die Einstellungen nur bis zum Beenden.
    pub fn save(&self) {
        let Some(path) = path() else { return };
        let theme = match self.theme {
            ThemePreference::System => "system",
            ThemePreference::Light => "light",
            ThemePreference::Dark => "dark",
        };
        let text = format!("theme={theme}\nlanguage={}\n", self.language.code());
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }
}

fn path() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?);
    Some(dir.join("windows-resizer").join("settings.txt"))
}
