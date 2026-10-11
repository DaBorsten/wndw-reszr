//! Texte der Oberfläche auf Deutsch und Englisch.

use crate::win;

#[derive(Clone, Copy, PartialEq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// Sprache der Windows-Oberfläche, Englisch für alles außer Deutsch.
    pub fn system() -> Self {
        const LANG_GERMAN: u16 = 0x07;
        if win::ui_language() == LANG_GERMAN {
            Lang::De
        } else {
            Lang::En
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "de" => Some(Lang::De),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    pub fn texts(self) -> &'static Texts {
        match self {
            Lang::De => &DE,
            Lang::En => &EN,
        }
    }
}

pub struct Texts {
    pub windows: &'static str,
    pub refresh: &'static str,
    pub search_hint: &'static str,
    pub elevated_hint: &'static str,
    pub no_windows: &'static str,
    pub select_window: &'static str,
    pub window_gone: &'static str,
    pub work_area: &'static str,
    /// Verbindet eine Größe mit ihrer Position: „800 × 600 px bei (0, 0)“.
    pub at: &'static str,
    pub position_size: &'static str,
    pub width: &'static str,
    pub height: &'static str,
    pub anchor_label: &'static str,
    pub anchor_top_left: &'static str,
    pub anchor_center: &'static str,
    pub edge_label: &'static str,
    pub edge_stop: &'static str,
    pub edge_push: &'static str,
    pub edge_hint: &'static str,
    pub apply: &'static str,
    pub load_current: &'static str,
    pub live: &'static str,
    pub live_hint: &'static str,
    pub applied: &'static str,
    pub access_denied: &'static str,
    pub error: &'static str,
    pub presets: &'static str,
    /// Beschriftung und Tooltip je Kachel: linke, rechte, obere, untere Hälfte, vollflächig.
    pub preset_tiles: [(&'static str, &'static str); 5],
    pub preset_center: (&'static str, &'static str),
    pub settings: &'static str,
    pub theme: &'static str,
    pub theme_system: &'static str,
    pub theme_light: &'static str,
    pub theme_dark: &'static str,
    pub language: &'static str,
    pub close: &'static str,
    pub start_failed: &'static str,
}

const DE: Texts = Texts {
    windows: "Fenster",
    refresh: "Aktualisieren",
    search_hint: "Fenster suchen…",
    elevated_hint: "Läuft mit Administratorrechten. Starte Wndw Reszr als \
                    Administrator, um dieses Fenster zu ändern.",
    no_windows: "Keine Fenster gefunden.",
    select_window: "Wähle links ein Fenster aus.",
    window_gone: "Das Fenster ist nicht mehr verfügbar.",
    work_area: "Arbeitsbereich des Monitors",
    at: "bei",
    position_size: "Position und Größe",
    width: "Breite",
    height: "Höhe",
    anchor_label: "X/Y beziehen sich auf",
    anchor_top_left: "Ecke oben links",
    anchor_center: "Fenstermitte",
    edge_label: "Am Monitorrand",
    edge_stop: "Größe begrenzen",
    edge_push: "Position nachrücken",
    edge_hint: "Größe begrenzen: Breite und Höhe enden am Rand, die Position bleibt.\n\
                Position nachrücken: Das Fenster weicht aus und kann weiter wachsen.",
    apply: "Anwenden",
    load_current: "Aktuelle Werte laden",
    live: "Live anwenden",
    live_hint: "Nur ohne „Live anwenden“ verfügbar",
    applied: "Gesetzt",
    access_denied: "Zugriff verweigert: Das Fenster läuft mit Administratorrechten. \
                    Starte diese App ebenfalls als Administrator.",
    error: "Fehler",
    presets: "Schnellauswahl",
    preset_tiles: [
        ("Links", "Linke Hälfte"),
        ("Rechts", "Rechte Hälfte"),
        ("Oben", "Obere Hälfte"),
        ("Unten", "Untere Hälfte"),
        ("Voll", "Vollflächig"),
    ],
    preset_center: ("Mitte", "Zentrieren, Größe bleibt"),
    settings: "Einstellungen",
    theme: "Design",
    theme_system: "System",
    theme_light: "Hell",
    theme_dark: "Dunkel",
    language: "Sprache",
    close: "Schließen",
    start_failed: "Wndw Reszr konnte nicht starten. Die App braucht OpenGL 2.0 oder \n                   neuer, ein aktueller Grafiktreiber kann helfen.",
};

const EN: Texts = Texts {
    windows: "Windows",
    refresh: "Refresh",
    search_hint: "Search windows…",
    elevated_hint: "Runs with administrator rights. Start Wndw Reszr as \
                    administrator to change this window.",
    no_windows: "No windows found.",
    select_window: "Select a window on the left.",
    window_gone: "The window is no longer available.",
    work_area: "Monitor work area",
    at: "at",
    position_size: "Position and size",
    width: "Width",
    height: "Height",
    anchor_label: "X/Y refer to",
    anchor_top_left: "Top-left corner",
    anchor_center: "Window center",
    edge_label: "At the monitor edge",
    edge_stop: "Limit size",
    edge_push: "Shift position",
    edge_hint: "Limit size: width and height end at the edge, the position stays.\n\
                Shift position: the window moves aside and can keep growing.",
    apply: "Apply",
    load_current: "Load current values",
    live: "Apply live",
    live_hint: "Only available while “Apply live” is off",
    applied: "Set",
    access_denied: "Access denied: the window runs with administrator rights. \
                    Start this app as administrator as well.",
    error: "Error",
    presets: "Presets",
    preset_tiles: [
        ("Left", "Left half"),
        ("Right", "Right half"),
        ("Top", "Top half"),
        ("Bottom", "Bottom half"),
        ("Full", "Full area"),
    ],
    preset_center: ("Center", "Center, keep size"),
    settings: "Settings",
    theme: "Theme",
    theme_system: "System",
    theme_light: "Light",
    theme_dark: "Dark",
    language: "Language",
    close: "Close",
    start_failed: "Wndw Reszr could not start. The app needs OpenGL 2.0 or newer, \n                   an up-to-date graphics driver may help.",
};
