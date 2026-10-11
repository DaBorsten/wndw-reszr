//! Farben, Schrift und Widget-Stil der Oberfläche.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};

/// Farben der Oberfläche für ein Design.
pub struct Palette {
    pub bg: Color32,
    pub sidebar: Color32,
    pub card: Color32,
    pub input: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub control_active: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub success: Color32,
    pub error: Color32,
}

const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x16, 0x18, 0x1D),
    sidebar: Color32::from_rgb(0x10, 0x12, 0x16),
    card: Color32::from_rgb(0x1D, 0x20, 0x27),
    input: Color32::from_rgb(0x0E, 0x10, 0x14),
    control: Color32::from_rgb(0x28, 0x2C, 0x35),
    control_hover: Color32::from_rgb(0x32, 0x37, 0x42),
    control_active: Color32::from_rgb(0x3A, 0x40, 0x4D),
    border: Color32::from_rgb(0x2B, 0x2F, 0x39),
    border_strong: Color32::from_rgb(0x41, 0x47, 0x54),
    text: Color32::from_rgb(0xE6, 0xE8, 0xEC),
    muted: Color32::from_rgb(0x8B, 0x93, 0xA1),
    accent: Color32::from_rgb(0x3D, 0x7E, 0xF5),
    accent_soft: Color32::from_rgb(0x1E, 0x2C, 0x48),
    success: Color32::from_rgb(0x4A, 0xD0, 0x80),
    error: Color32::from_rgb(0xFF, 0x6B, 0x6B),
};

const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xF3, 0xF4, 0xF6),
    sidebar: Color32::from_rgb(0xE8, 0xEA, 0xEE),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    input: Color32::from_rgb(0xED, 0xEF, 0xF3),
    control: Color32::from_rgb(0xDE, 0xE1, 0xE7),
    control_hover: Color32::from_rgb(0xD3, 0xD7, 0xDE),
    control_active: Color32::from_rgb(0xC6, 0xCB, 0xD4),
    border: Color32::from_rgb(0xD9, 0xDC, 0xE2),
    border_strong: Color32::from_rgb(0xB4, 0xBA, 0xC5),
    text: Color32::from_rgb(0x1B, 0x1E, 0x24),
    muted: Color32::from_rgb(0x5F, 0x67, 0x75),
    accent: Color32::from_rgb(0x2F, 0x6F, 0xEB),
    accent_soft: Color32::from_rgb(0xD6, 0xE4, 0xFD),
    success: Color32::from_rgb(0x1F, 0x9D, 0x55),
    error: Color32::from_rgb(0xD1, 0x34, 0x38),
};

/// Farben des Designs, in dem `ui` gerade gezeichnet wird.
pub fn colors(ui: &egui::Ui) -> &'static Palette {
    if ui.visuals().dark_mode { &DARK } else { &LIGHT }
}

const SEMIBOLD: &str = "semibold";

/// Schriftfamilie für Überschriften und Abschnittstitel.
pub fn semibold() -> FontFamily {
    FontFamily::Name(SEMIBOLD.into())
}

/// Richtet Schrift und Stil für beide Designs ein. Welches gilt, legt
/// `Context::set_theme` fest.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.style_mut_of(egui::Theme::Dark, |style| apply_style(style, &DARK));
    ctx.style_mut_of(egui::Theme::Light, |style| apply_style(style, &LIGHT));
}

fn apply_style(style: &mut egui::Style, colors: &Palette) {
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (TextStyle::Heading, FontId::new(20.0, semibold())),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(8.0, 8.0);
    spacing.button_padding = egui::vec2(12.0, 5.0);
    spacing.interact_size.y = 28.0;

    let visuals = &mut style.visuals;
    visuals.panel_fill = colors.bg;
    visuals.window_fill = colors.card;
    visuals.extreme_bg_color = colors.input;
    visuals.weak_text_color = Some(colors.muted);
    visuals.error_fg_color = colors.error;
    visuals.selection.bg_fill = colors.accent;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, colors.border);
    widgets.noninteractive.fg_stroke.color = colors.text;
    for (state, fill, border) in [
        (&mut widgets.inactive, colors.control, colors.border),
        (&mut widgets.hovered, colors.control_hover, colors.border_strong),
        (&mut widgets.active, colors.control_active, colors.accent),
        (&mut widgets.open, colors.control_active, colors.border_strong),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, border);
        state.fg_stroke.color = colors.text;
        state.expansion = 0.0;
    }
    for state in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        state.corner_radius = CornerRadius::same(6);
    }
}

/// Segoe UI als Hauptschrift; fehlt sie, bleiben die eingebauten egui-Schriften.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let windows = std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into());
    let dir = std::path::Path::new(&windows).join("Fonts");
    let mut load = |name: &str, file: &str| {
        let bytes = std::fs::read(dir.join(file)).ok()?;
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
        Some(name.to_owned())
    };
    let regular = load("segoe-ui", "segoeui.ttf");
    let bold = load("segoe-ui-semibold", "seguisb.ttf");

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    if let Some(regular) = regular {
        proportional.insert(0, regular);
    }
    let mut heading = proportional.clone();
    if let Some(bold) = bold {
        heading.insert(0, bold);
    }
    fonts.families.insert(semibold(), heading);
    fonts
}
