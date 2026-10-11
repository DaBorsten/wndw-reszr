#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod i18n;
mod settings;
mod theme;
mod win;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::emath::GuiRounding as _;
use windows::Win32::Foundation::HWND;

use i18n::Lang;
use settings::Settings;
use win::{Rect, WindowInfo};

const APP_NAME: &str = "Wndw Reszr";
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const SIDEBAR_MIN_WIDTH: f32 = 200.0;
const SIDEBAR_MAX_WIDTH: f32 = 400.0;
const EDITOR_MIN_WIDTH: f32 = 480.0;

fn main() -> eframe::Result {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("eingebettetes Symbol");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(icon)
            .with_inner_size([820.0, 700.0])
            .with_min_inner_size([SIDEBAR_MIN_WIDTH + EDITOR_MIN_WIDTH, 420.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            let settings = Settings::load();
            theme::apply(&cc.egui_ctx);
            cc.egui_ctx.set_theme(settings.theme);
            Ok(Box::new(App::new(cc.egui_ctx.clone(), settings)))
        }),
    );
    // Ohne Konsole bliebe ein Fehler beim Start unsichtbar, etwa ein fehlender
    // OpenGL-Treiber.
    if let Err(err) = &result {
        let t = Lang::system().texts();
        win::error_box(APP_NAME, &format!("{}

{err}", t.start_failed));
    }
    result
}

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Px,
    Percent,
}

/// Ein Eingabewert, entweder in Pixeln oder in Prozent des Monitor-Arbeitsbereichs.
#[derive(Clone, Copy)]
struct Field {
    value: f64,
    unit: Unit,
}

impl Field {
    fn px(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Px,
        }
    }

    /// `origin` ist der Anfang des Arbeitsbereichs auf dieser Achse (für Größen 0),
    /// `extent` dessen Länge.
    fn to_px(self, origin: i32, extent: i32) -> i32 {
        match self.unit {
            Unit::Px => self.value.round() as i32,
            Unit::Percent => origin + (extent as f64 * self.value / 100.0).round() as i32,
        }
    }

    fn set_px(&mut self, px: i32, origin: i32, extent: i32) {
        self.value = match self.unit {
            Unit::Px => px as f64,
            Unit::Percent => (px - origin) as f64 / extent.max(1) as f64 * 100.0,
        };
    }

    fn set_unit(&mut self, unit: Unit, origin: i32, extent: i32) {
        let px = self.to_px(origin, extent);
        self.unit = unit;
        self.set_px(px, origin, extent);
    }
}

/// Punkt des Fensters, auf den sich X und Y beziehen.
#[derive(Clone, Copy, PartialEq)]
enum Anchor {
    TopLeft,
    Center,
}

impl Anchor {
    /// Abstand des Bezugspunkts von der linken bzw. oberen Fensterkante.
    fn offset(self, size: i32) -> i32 {
        match self {
            Anchor::TopLeft => 0,
            Anchor::Center => size / 2,
        }
    }

    /// Größte Fenstergröße, bei der das Fenster im Arbeitsbereich bleibt, wenn der
    /// Bezugspunkt auf `pos` stehen bleibt.
    fn max_size(self, pos: i32, origin: i32, extent: i32) -> i32 {
        let end = origin + extent;
        let max = match self {
            Anchor::TopLeft => end - pos,
            // Bei ungerader Größe liegt der überzählige Pixel rechts bzw. unten.
            Anchor::Center => (2 * (pos - origin) + 1).min(2 * (end - pos)),
        };
        max.min(extent).max(1)
    }
}

/// Was passiert, wenn das Fenster beim Vergrößern den Rand des Arbeitsbereichs erreicht.
#[derive(Clone, Copy, PartialEq)]
enum EdgeMode {
    /// Breite und Höhe wachsen nicht weiter, die Position bleibt.
    Stop,
    /// Die Position rückt nach, bis das Fenster den Arbeitsbereich ausfüllt.
    Push,
}

/// Ergebnis des letzten Anwendens. Der Text entsteht erst bei der Anzeige, damit er
/// einem Sprachwechsel folgt.
enum Status {
    Applied(Rect),
    AccessDenied,
    Error(String),
}

struct App {
    ctx: egui::Context,
    windows: Vec<WindowInfo>,
    /// Symbole je Fensterhandle; `None`, wenn das Fenster keines liefert.
    icons: HashMap<isize, Option<egui::TextureHandle>>,
    last_refresh: Instant,
    filter: String,
    selected: Option<HWND>,
    /// Zuletzt gesehene Lage des gewählten Fensters und sein Arbeitsbereich.
    observed: Option<(Rect, Rect)>,
    /// Setzt die Lage des gewählten Fensters.
    mover: Option<win::Mover>,
    x: Field,
    y: Field,
    w: Field,
    h: Field,
    anchor: Anchor,
    edge_mode: EdgeMode,
    live: bool,
    status: Option<Status>,
    settings: Settings,
    settings_open: bool,
}

impl App {
    fn new(ctx: egui::Context, settings: Settings) -> Self {
        Self {
            ctx,
            windows: win::list_windows(),
            icons: HashMap::new(),
            last_refresh: Instant::now(),
            filter: String::new(),
            selected: None,
            observed: None,
            mover: None,
            x: Field::px(0.0),
            y: Field::px(0.0),
            w: Field::px(800.0),
            h: Field::px(600.0),
            anchor: Anchor::Center,
            edge_mode: EdgeMode::Stop,
            live: true,
            status: None,
            settings,
            settings_open: false,
        }
    }

    fn refresh(&mut self) {
        self.windows = win::list_windows();
        self.last_refresh = Instant::now();
        // Windows vergibt Handles neu, ein altes Symbol darf nicht am neuen Fenster kleben.
        let windows = &self.windows;
        self.icons
            .retain(|&key, _| windows.iter().any(|window| window.hwnd.0 as isize == key));
        if self.selected.is_some_and(|hwnd| !win::exists(hwnd)) {
            self.selected = None;
            self.status = None;
            self.mover = None;
            win::unwatch();
        }
    }

    fn select(&mut self, hwnd: HWND) {
        self.selected = Some(hwnd);
        self.status = None;
        self.load_current(hwnd);
        self.observed = observe(hwnd);
        // Neu zeichnen, sobald sich das Fenster ändert: Erst dabei greift `follow`.
        let ctx = self.ctx.clone();
        win::watch(hwnd, move || ctx.request_repaint());
        // Je Auswahl ein eigener: Hängt das alte Fenster, bleibt nur dessen Thread stehen.
        let ctx = self.ctx.clone();
        self.mover = Some(win::Mover::new(hwnd, move || ctx.request_repaint()));
    }

    /// Übernimmt die aktuelle Fensterposition in die Eingabefelder (Einheiten bleiben).
    fn load_current(&mut self, hwnd: HWND) {
        if let (Some(rect), Some(area)) = (win::rect(hwnd), win::work_area(hwnd)) {
            self.w.set_px(rect.w, 0, area.w);
            self.h.set_px(rect.h, 0, area.h);
            self.set_position(rect, area);
        }
    }

    /// Schreibt die Position von `rect` in die X/Y-Felder, bezogen auf den Bezugspunkt.
    fn set_position(&mut self, rect: Rect, area: Rect) {
        self.x
            .set_px(rect.x + self.anchor.offset(rect.w), area.x, area.w);
        self.y
            .set_px(rect.y + self.anchor.offset(rect.h), area.y, area.h);
    }

    fn target_rect(&self, area: Rect) -> Rect {
        let w = self.w.to_px(0, area.w).max(1);
        let h = self.h.to_px(0, area.h).max(1);
        Rect {
            x: self.x.to_px(area.x, area.w) - self.anchor.offset(w),
            y: self.y.to_px(area.y, area.h) - self.anchor.offset(h),
            w,
            h,
        }
    }

    /// Verkleinert und verschiebt das Ziel so, dass es vollständig im Arbeitsbereich liegt.
    fn clamp_to_area(&mut self, area: Rect) {
        let target = self.target_rect(area);
        let w = target.w.min(area.w);
        let h = target.h.min(area.h);
        let x = target.x.clamp(area.x, area.x + area.w - w);
        let y = target.y.clamp(area.y, area.y + area.h - h);
        self.set_target(Rect { x, y, w, h }, area);
    }

    /// Macht `rect` zum Ziel. Felder, die schon passen, bleiben unangetastet: Das
    /// Zurückschreiben würde Prozentwerte durch die Pixelrundung verändern.
    fn set_target(&mut self, rect: Rect, area: Rect) {
        let target = self.target_rect(area);
        if rect.w != target.w {
            self.w.set_px(rect.w, 0, area.w);
        }
        if rect.h != target.h {
            self.h.set_px(rect.h, 0, area.h);
        }
        if rect.x != target.x || rect.w != target.w {
            self.x
                .set_px(rect.x + self.anchor.offset(rect.w), area.x, area.w);
        }
        if rect.y != target.y || rect.h != target.h {
            self.y
                .set_px(rect.y + self.anchor.offset(rect.h), area.y, area.h);
        }
    }

    /// Übernimmt im Live-Modus Änderungen, die nicht von dieser App stammen, etwa wenn
    /// das Fenster verschoben oder die Skalierung in Windows umgestellt wurde.
    fn follow(&mut self, hwnd: HWND, area: Rect) {
        let observed = win::rect(hwnd).map(|rect| (rect, area));
        if observed == self.observed {
            return;
        }
        self.observed = observed;
        // Solange eine eigene Änderung aussteht, ist jede Bewegung des Fensters deren Folge.
        let busy = self.mover.as_ref().is_some_and(win::Mover::busy);
        if self.live
            && !busy
            && let Some((rect, _)) = observed
        {
            self.set_target(rect, area);
            self.status = None;
        }
    }

    /// Wechselt den Bezugspunkt und rechnet X/Y um, sodass das Ziel gleich bleibt.
    fn set_anchor(&mut self, anchor: Anchor, area: Rect) {
        let target = self.target_rect(area);
        self.anchor = anchor;
        self.set_position(target, area);
    }

    /// Das Ergebnis holt `finish_apply` ab, sobald das Fenster reagiert hat.
    fn apply(&mut self, area: Rect) {
        self.clamp_to_area(area);
        let target = self.target_rect(area);
        if let Some(mover) = &mut self.mover {
            mover.request(target);
        }
    }

    fn finish_apply(&mut self, hwnd: HWND) {
        let Some((target, result)) = self.mover.as_mut().and_then(win::Mover::poll) else {
            return;
        };
        self.status = Some(match result {
            Ok(()) => Status::Applied(target),
            Err(win::SetError::AccessDenied) => Status::AccessDenied,
            Err(win::SetError::Other(message)) => Status::Error(message),
        });
        // Die eigene Änderung soll nicht als fremde gelten.
        self.observed = observe(hwnd);
    }

    /// Setzt alle Felder auf Prozentwerte des Arbeitsbereichs und wendet sie an.
    fn apply_preset(&mut self, area: Rect, [x, y, w, h]: [f64; 4]) {
        let percent = |value| Field {
            value,
            unit: Unit::Percent,
        };
        self.w = percent(w);
        self.h = percent(h);
        self.x.unit = Unit::Percent;
        self.y.unit = Unit::Percent;
        let target = Rect {
            x: percent(x).to_px(area.x, area.w),
            y: percent(y).to_px(area.y, area.h),
            ..self.target_rect(area)
        };
        self.set_position(target, area);
        self.apply(area);
    }

    fn center(&mut self, area: Rect) {
        let target = self.target_rect(area);
        let centered = Rect {
            x: area.x + (area.w - target.w) / 2,
            y: area.y + (area.h - target.h) / 2,
            ..target
        };
        self.set_position(centered, area);
        self.apply(area);
    }

    fn window_list(&mut self, ui: &mut egui::Ui) {
        let t = self.settings.language.texts();
        ui.horizontal(|ui| {
            ui.heading(t.windows);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(t.refresh).clicked() {
                    self.refresh();
                }
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text(t.search_hint)
                .margin(egui::Margin::symmetric(8, 6))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(4.0);

        for window in &self.windows {
            self.icons.entry(window.hwnd.0 as isize).or_insert_with(|| {
                let rgba = win::icon(window.hwnd)?;
                let image = egui::ColorImage::from_rgba_premultiplied([win::ICON_SIZE; 2], &rgba);
                Some(
                    ui.ctx()
                        .load_texture("window-icon", image, egui::TextureOptions::LINEAR),
                )
            });
        }

        let filter = self.filter.to_lowercase();
        let mut clicked = None;
        // Unter der Liste bleibt Platz für den Knopf zu den Einstellungen.
        let footer = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(ui.available_height() - footer)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let mut any = false;
                for window in &self.windows {
                    if !window.title.to_lowercase().contains(&filter) {
                        continue;
                    }
                    any = true;
                    let selected = self.selected == Some(window.hwnd);
                    let icon = self
                        .icons
                        .get(&(window.hwnd.0 as isize))
                        .and_then(Option::as_ref);
                    let row = window_row(ui, &window.title, icon, selected, window.elevated);
                    if window.elevated {
                        row.tooltip(t.elevated_hint);
                    } else if row.tooltip(&window.title).clicked() {
                        clicked = Some(window.hwnd);
                    }
                }
                if !any {
                    ui.add_space(8.0);
                    ui.weak(t.no_windows);
                }
            });
        if let Some(hwnd) = clicked {
            self.select(hwnd);
        }
        let settings = egui::Button::new(format!("⚙  {}", t.settings))
            .min_size(egui::vec2(ui.available_width(), 0.0));
        if ui.add(settings).clicked() {
            self.settings_open = true;
        }
    }

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let t = self.settings.language.texts();
        let frame = egui::Frame::popup(&ctx.global_style())
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::same(16));
        let mut changed = false;
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("settings"))
            .frame(frame)
            .show(ctx, |ui| {
                let colors = theme::colors(ui);
                let heading = ui.heading(t.settings).rect;
                ui.add_space(4.0);
                egui::Grid::new("settings")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(t.theme).color(colors.muted));
                        let themes = [
                            (egui::ThemePreference::System, t.theme_system),
                            (egui::ThemePreference::Light, t.theme_light),
                            (egui::ThemePreference::Dark, t.theme_dark),
                        ];
                        if let Some(theme) = segmented(ui, self.settings.theme, &themes) {
                            self.settings.theme = theme;
                            ui.ctx().set_theme(theme);
                            changed = true;
                        }
                        ui.end_row();

                        ui.label(egui::RichText::new(t.language).color(colors.muted));
                        let languages = [(Lang::De, "Deutsch"), (Lang::En, "English")];
                        if let Some(language) = segmented(ui, self.settings.language, &languages)
                        {
                            self.settings.language = language;
                            changed = true;
                        }
                        ui.end_row();
                    });
                // Placed after the content so it can sit at the dialog's right edge
                // without stretching the modal.
                let size = egui::Vec2::splat(heading.height());
                let rect = egui::Rect::from_min_size(
                    egui::pos2(ui.min_rect().right() - size.x, heading.top()),
                    size,
                );
                let response = ui
                    .interact(rect, ui.id().with("close"), egui::Sense::click())
                    .on_hover_text(t.close);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t.close)
                });
                let visuals = ui.style().interact(&response);
                if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
                }
                let cross = rect.shrink(size.x * 0.3);
                let stroke = egui::Stroke::new(1.5, visuals.fg_stroke.color);
                ui.painter()
                    .line_segment([cross.left_top(), cross.right_bottom()], stroke);
                ui.painter()
                    .line_segment([cross.right_top(), cross.left_bottom()], stroke);
                close = response.clicked();
            });
        if changed {
            self.settings.save();
        }
        if close || modal.should_close() {
            self.settings_open = false;
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let t = self.settings.language.texts();
        let Some(hwnd) = self.selected else {
            ui.centered_and_justified(|ui| {
                ui.weak(t.select_window);
            });
            return;
        };
        let Some(area) = win::work_area(hwnd) else {
            ui.weak(t.window_gone);
            return;
        };
        self.finish_apply(hwnd);
        self.follow(hwnd, area);

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| self.editor_contents(ui, hwnd, area));
    }

    fn editor_contents(&mut self, ui: &mut egui::Ui, hwnd: HWND, area: Rect) {
        let t = self.settings.language.texts();
        let colors = theme::colors(ui);
        let title = self
            .windows
            .iter()
            .find(|window| window.hwnd == hwnd)
            .map_or("", |window| window.title.as_str());
        ui.add(egui::Label::new(egui::RichText::new(title).heading()).truncate());
        ui.label(
            egui::RichText::new(format!(
                "{}: {} × {} px {} ({}, {})",
                t.work_area, area.w, area.h, t.at, area.x, area.y
            ))
            .small()
            .weak(),
        );
        ui.add_space(8.0);

        card(ui, |ui| {
            preview(ui, area, self.target_rect(area), win::rect(hwnd));
        });

        // Die Felder lassen nur Werte zu, bei denen das Fenster im Arbeitsbereich bleibt.
        // X und Y enden dort, wo das Fenster bei unveränderter Größe den Rand erreicht.
        let target = self.target_rect(area);
        let (w, h) = (target.w.min(area.w), target.h.min(area.h));
        let x_min = area.x + self.anchor.offset(w);
        let y_min = area.y + self.anchor.offset(h);
        let x_range = x_min..=x_min + area.w - w;
        let y_range = y_min..=y_min + area.h - h;
        let (w_max, h_max) = match self.edge_mode {
            EdgeMode::Stop => (
                self.anchor
                    .max_size(self.x.to_px(area.x, area.w), area.x, area.w),
                self.anchor
                    .max_size(self.y.to_px(area.y, area.h), area.y, area.h),
            ),
            EdgeMode::Push => (area.w.max(1), area.h.max(1)),
        };

        let mut changed = false;
        card(ui, |ui| {
            section(ui, t.position_size);
            egui::Grid::new("fields")
                .num_columns(3)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    changed |= field_row(ui, "X", &mut self.x, area.x, area.w, x_range);
                    changed |= field_row(ui, "Y", &mut self.y, area.y, area.h, y_range);
                    changed |= field_row(ui, t.width, &mut self.w, 0, area.w, 1..=w_max);
                    changed |= field_row(ui, t.height, &mut self.h, 0, area.h, 1..=h_max);
                });

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(t.anchor_label).color(colors.muted));
                let anchors = [
                    (Anchor::TopLeft, t.anchor_top_left),
                    (Anchor::Center, t.anchor_center),
                ];
                if let Some(anchor) = segmented(ui, self.anchor, &anchors) {
                    self.set_anchor(anchor, area);
                }
            });

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(t.edge_label).color(colors.muted));
                let modes = [
                    (EdgeMode::Stop, t.edge_stop),
                    (EdgeMode::Push, t.edge_push),
                ];
                if let Some(mode) = segmented(ui, self.edge_mode, &modes) {
                    self.edge_mode = mode;
                }
            })
            .response
            .tooltip(t.edge_hint);

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let apply =
                    egui::Button::new(egui::RichText::new(t.apply).color(egui::Color32::WHITE))
                        .fill(colors.accent)
                        .stroke(egui::Stroke::NONE);
                // Im Live-Modus folgt das Fenster den Feldern, Anwenden und Laden wären
                // dort wirkungslos.
                let apply = ui
                    .add_enabled(!self.live, apply)
                    .disabled_tooltip(t.live_hint);
                if apply.clicked() {
                    self.apply(area);
                }
                let load = ui
                    .add_enabled(!self.live, egui::Button::new(t.load_current))
                    .disabled_tooltip(t.live_hint);
                if load.clicked() {
                    self.load_current(hwnd);
                    self.status = None;
                }
                ui.add_space(4.0);
                // Die Standardrundung macht aus dem kleinen Kästchen fast einen Kreis.
                let widgets = &mut ui.visuals_mut().widgets;
                widgets.inactive.bg_fill = colors.input;
                widgets.inactive.bg_stroke = egui::Stroke::new(1.0, colors.border_strong);
                for state in [
                    &mut widgets.inactive,
                    &mut widgets.hovered,
                    &mut widgets.active,
                ] {
                    state.corner_radius = egui::CornerRadius::same(4);
                }
                // Beim Einschalten sofort anwenden: Die Felder können schon vom Fenster
                // abweichen, wenn vorher ohne Live-Modus geändert wurde.
                changed |= ui.checkbox(&mut self.live, t.live).changed();
            });
            if changed {
                // Bei „Position nachrücken“ kann die neue Größe über den Rand ragen.
                self.clamp_to_area(area);
                if self.live {
                    self.apply(area);
                }
            }

            if let Some(status) = &self.status {
                let (dot, color, text) = match status {
                    Status::Applied(rect) => (
                        colors.success,
                        colors.muted,
                        format!(
                            "{}: {} × {} px {} ({}, {})",
                            t.applied, rect.w, rect.h, t.at, rect.x, rect.y
                        ),
                    ),
                    Status::AccessDenied => {
                        (colors.error, colors.error, t.access_denied.to_owned())
                    }
                    Status::Error(message) => {
                        (colors.error, colors.error, format!("{}: {message}", t.error))
                    }
                };
                ui.horizontal_wrapped(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, dot);
                    ui.label(egui::RichText::new(text).small().color(color));
                });
            }
        });

        card(ui, |ui| {
            section(ui, t.presets);
            ui.horizontal_wrapped(|ui| {
                const PRESETS: [[f64; 4]; 5] = [
                    [0.0, 0.0, 50.0, 100.0],
                    [50.0, 0.0, 50.0, 100.0],
                    [0.0, 0.0, 100.0, 50.0],
                    [0.0, 50.0, 100.0, 50.0],
                    [0.0, 0.0, 100.0, 100.0],
                ];
                for ((label, hint), preset) in t.preset_tiles.into_iter().zip(PRESETS) {
                    if preset_tile(ui, label, preset).tooltip(hint).clicked() {
                        self.apply_preset(area, preset);
                    }
                }
                let (label, hint) = t.preset_center;
                if preset_tile(ui, label, [25.0, 25.0, 50.0, 50.0])
                    .tooltip(hint)
                    .clicked()
                {
                    self.center(area);
                }
            });
        });
    }
}

fn observe(hwnd: HWND) -> Option<(Rect, Rect)> {
    Some((win::rect(hwnd)?, win::work_area(hwnd)?))
}

/// Tooltips über dem Element statt wie bei egui üblich darunter, wo sie in der
/// Fensterliste die folgenden Einträge verdecken.
trait TooltipExt: Sized {
    fn tooltip(self, text: &str) -> Self;
    fn disabled_tooltip(self, text: &str) -> Self;
}

impl TooltipExt for egui::Response {
    fn tooltip(self, text: &str) -> Self {
        show_tooltip(egui::Tooltip::for_enabled(&self), text);
        self
    }

    fn disabled_tooltip(self, text: &str) -> Self {
        show_tooltip(egui::Tooltip::for_disabled(&self), text);
        self
    }
}

fn show_tooltip(mut tooltip: egui::Tooltip<'_>, text: &str) {
    tooltip.popup = tooltip.popup.align(egui::RectAlign::TOP_START);
    tooltip.show(|ui| {
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.label(text);
    });
}

/// Umrandete Fläche über die volle Breite, die zusammengehörige Bedienelemente gruppiert.
fn card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let colors = theme::colors(ui);
    egui::Frame::new()
        .fill(colors.card)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

fn section(ui: &mut egui::Ui, text: &str) {
    let colors = theme::colors(ui);
    ui.label(
        egui::RichText::new(text)
            .font(egui::FontId::new(13.0, theme::semibold()))
            .color(colors.muted),
    );
}

/// Eintrag der Fensterliste über die volle Breite, mit Akzentbalken für die Auswahl.
/// Ein gesperrter Eintrag ist ausgegraut und nicht anklickbar.
fn window_row(
    ui: &mut egui::Ui,
    title: &str,
    icon: Option<&egui::TextureHandle>,
    selected: bool,
    locked: bool,
) -> egui::Response {
    let colors = theme::colors(ui);
    let size = egui::vec2(ui.available_width(), 30.0);
    let sense = if locked {
        egui::Sense::hover()
    } else {
        egui::Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, !locked, selected, title)
    });

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, egui::CornerRadius::same(6), colors.accent_soft);
            let bar = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 1.5, rect.center().y),
                egui::vec2(3.0, 14.0),
            );
            painter.rect_filled(bar, egui::CornerRadius::same(2), colors.accent);
        } else if response.hovered() && !locked {
            painter.rect_filled(rect, egui::CornerRadius::same(6), colors.control);
        }

        let color = if locked {
            colors.muted.gamma_multiply(0.55)
        } else if selected || response.hovered() {
            colors.text
        } else {
            colors.muted.lerp_to_gamma(colors.text, 0.55)
        };
        let galley = egui::WidgetText::from(title).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            rect.width() - 46.0,
            egui::TextStyle::Body,
        );
        // Der Platz bleibt auch ohne Symbol frei, damit die Titel bündig stehen.
        if let Some(icon) = icon {
            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 20.0, rect.center().y),
                egui::vec2(16.0, 16.0),
            )
            .round_to_pixels(ui.pixels_per_point());
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            let tint = if locked {
                egui::Color32::WHITE.gamma_multiply(0.35)
            } else {
                egui::Color32::WHITE
            };
            painter.image(icon.id(), icon_rect, uv, tint);
        }
        let pos = egui::pos2(rect.left() + 36.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(pos, galley, color);
    }
    response
}

/// Umschalter mit genau einer aktiven Option. Gibt die neu gewählte Option zurück.
fn segmented<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: T,
    options: &[(T, &str)],
) -> Option<T> {
    let colors = theme::colors(ui);
    let mut picked = None;
    egui::Frame::new()
        .fill(colors.input)
        .corner_radius(egui::CornerRadius::same(7))
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            // Vor `horizontal` setzen: Die Zeile übernimmt ihre Höhe aus `interact_size`,
            // sonst bleibt über und unter den Knöpfen mehr Rand als seitlich.
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().button_padding = egui::vec2(10.0, 3.0);
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                // In allen Zuständen gleich: egui zieht die Rahmenbreite vom Innenabstand
                // ab, ein nur teilweise entfernter Rahmen verschiebt beim Hovern den Text.
                state.bg_stroke = egui::Stroke::NONE;
                state.corner_radius = egui::CornerRadius::same(5);
            }
            ui.horizontal(|ui| {
                for &(value, text) in options {
                    let button = egui::Button::selectable(current == value, text);
                    if ui.add(button).clicked() && current != value {
                        picked = Some(value);
                    }
                }
            });
        });
    picked
}

/// Verkleinerte Ansicht des Arbeitsbereichs mit dem Zielrechteck und, falls abweichend,
/// der aktuellen Fensterlage als Umriss.
fn preview(ui: &mut egui::Ui, area: Rect, target: Rect, current: Option<Rect>) {
    let colors = theme::colors(ui);
    let aspect = area.w.max(1) as f32 / area.h.max(1) as f32;
    let width = ui.available_width();
    let height = (width / aspect).min(130.0);
    let (outer, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let screen = egui::Rect::from_center_size(outer.center(), egui::vec2(height * aspect, height));
    let scale = height / area.h.max(1) as f32;
    let map = |r: Rect| {
        egui::Rect::from_min_size(
            screen.min + egui::vec2((r.x - area.x) as f32, (r.y - area.y) as f32) * scale,
            egui::vec2(r.w as f32, r.h as f32) * scale,
        )
    };

    let radius = egui::CornerRadius::same(3);
    let inside = egui::StrokeKind::Inside;
    let painter = ui.painter().with_clip_rect(screen);
    painter.rect(
        screen,
        egui::CornerRadius::same(6),
        colors.input,
        egui::Stroke::new(1.0, colors.border),
        inside,
    );
    if let Some(current) = current.filter(|current| *current != target) {
        let stroke = egui::Stroke::new(1.0, colors.border_strong);
        painter.rect_stroke(map(current), radius, stroke, inside);
    }
    let rect = map(target);
    painter.rect(
        rect,
        radius,
        colors.accent.gamma_multiply(0.25),
        egui::Stroke::new(1.5, colors.accent),
        inside,
    );
    if rect.width() > 80.0 && rect.height() > 22.0 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{} × {}", target.w, target.h),
            egui::FontId::proportional(12.0),
            colors.text,
        );
    }
}

/// Kachel der Schnellauswahl: Mini-Monitor mit der Ziellage (`glyph` in Prozent als
/// x, y, Breite, Höhe) und Beschriftung darunter.
fn preset_tile(ui: &mut egui::Ui, label: &str, glyph: [f64; 4]) -> egui::Response {
    let colors = theme::colors(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(72.0, 64.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let painter = ui.painter();
        painter.rect(
            rect,
            egui::CornerRadius::same(8),
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );

        // Alles auf ganze Gerätepixel legen: Bei gebrochener Skalierung rundet egui sonst
        // jede Kante einzeln, und der Rand um die Füllung wird je Seite unterschiedlich breit.
        let ppp = ui.pixels_per_point();
        let px = |points: f32| (points * ppp).round().max(1.0) / ppp;
        let monitor = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.top() + 23.0),
            egui::vec2(32.0, 20.0),
        )
        .round_to_pixels(ppp);
        painter.rect(
            monitor,
            egui::CornerRadius::same(3),
            colors.input,
            egui::Stroke::new(px(1.0), colors.border_strong),
            egui::StrokeKind::Outside,
        );
        let [x, y, w, h] = glyph.map(|percent| percent as f32 / 100.0);
        let inner = monitor.shrink(px(2.0));
        let fill = egui::Rect::from_min_size(
            inner.min + egui::vec2(x, y) * inner.size(),
            egui::vec2(w, h) * inner.size(),
        )
        .round_to_pixels(ppp);
        painter.rect_filled(fill, egui::CornerRadius::same(1), colors.accent);

        painter.text(
            egui::pos2(rect.center().x, rect.bottom() - 16.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            visuals.fg_stroke.color,
        );
    }
    response
}

/// Eine Zeile aus Beschriftung, Zahlenfeld und Einheitenumschalter.
/// `range` sind die zulässigen Werte in Pixeln.
/// Gibt zurück, ob sich der Wert in Pixeln geändert haben kann.
fn field_row(
    ui: &mut egui::Ui,
    label: &str,
    field: &mut Field,
    origin: i32,
    extent: i32,
    range: std::ops::RangeInclusive<i32>,
) -> bool {
    let colors = theme::colors(ui);
    ui.label(egui::RichText::new(label).color(colors.muted));

    let start = field.value;
    // Grenzen in der Einheit des Felds.
    let bound = |px: i32| {
        let mut bound = *field;
        bound.set_px(px, origin, extent);
        bound.value
    };
    let (min, max) = (bound(*range.start()), bound(*range.end()));
    let unit = field.unit;
    let speed = match unit {
        Unit::Px => 1.0,
        Unit::Percent => 0.1,
    };
    let drag = match unit {
        Unit::Px => egui::DragValue::new(&mut field.value)
            .speed(speed)
            .max_decimals(0)
            .suffix(" px"),
        Unit::Percent => egui::DragValue::new(&mut field.value)
            .speed(speed)
            .max_decimals(2)
            .suffix(" %"),
    };
    // Nicht auf bestehende Werte anwenden: Das würde ein Fenster, das über den Rand
    // hinausragt, im Live-Modus schon beim Auswählen verschieben.
    let drag = drag
        .range(min..=max)
        .clamp_existing_to_range(false);
    let response = ui
        .scope(|ui| {
            let widgets = &mut ui.visuals_mut().widgets;
            widgets.inactive.weak_bg_fill = colors.input;
            widgets.hovered.weak_bg_fill = colors.input;
            ui.add_sized([120.0, 28.0], drag)
        })
        .inner;
    let mut changed = response.changed();

    // Beim Ziehen bleibt der Mauszeiger unsichtbar an Ort und Stelle stehen, damit der Weg
    // nicht am Bildschirmrand endet. Der Wert folgt deshalb der eigenen Messung statt der
    // von egui, das jedes Zurücksetzen des Zeigers als Bewegung in die Gegenrichtung sieht.
    let pin_id = response.id.with("drag_pin");
    if response.drag_started()
        && let Some(anchor) = win::cursor_pos()
    {
        ui.data_mut(|data| data.insert_temp(pin_id, (anchor, start)));
    }
    if response.drag_stopped() {
        ui.data_mut(|data| data.remove::<((i32, i32), f64)>(pin_id));
    }
    if response.dragged()
        && let Some((anchor, precise)) =
            ui.data(|data| data.get_temp::<((i32, i32), f64)>(pin_id))
        && let Some(pos) = win::cursor_pos()
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        field.value = start;
        if pos != anchor {
            win::set_cursor_pos(anchor);
            // Wie bei egui: nach rechts und nach oben erhöht, Shift zieht zehnmal feiner.
            let points = ((pos.0 - anchor.0) - (pos.1 - anchor.1)) as f64
                / ui.ctx().pixels_per_point() as f64;
            let slow = ui.input(|input| input.modifiers.shift_only());
            let speed = if slow { speed / 10.0 } else { speed };
            let precise = (precise + points * speed).max(min).min(max);
            let step = match unit {
                Unit::Px => 1.0,
                Unit::Percent if slow => 100.0,
                Unit::Percent => 10.0,
            };
            field.value = (precise * step).round() / step;
            ui.data_mut(|data| data.insert_temp(pin_id, (anchor, precise)));
        }
        changed = field.value != start;
    }

    // Esc während des Ziehens stellt den Wert vom Beginn des Ziehens wieder her.
    let start_id = response.id.with("drag_start");
    if response.drag_started() {
        ui.data_mut(|data| data.insert_temp(start_id, start));
    }
    // egui bricht das Ziehen bei Esc selbst ab, noch bevor das Feld an der Reihe ist:
    // In diesem Frame gilt es deshalb schon als losgelassen, nicht mehr als gezogen.
    if response.drag_stopped()
        && ui.input(|input| input.key_pressed(egui::Key::Escape))
        && let Some(start) = ui.data(|data| data.get_temp::<f64>(start_id))
    {
        field.value = start;
        changed = true;
    }

    let units = [(Unit::Px, "px"), (Unit::Percent, "%")];
    if let Some(unit) = segmented(ui, field.unit, &units) {
        field.set_unit(unit, origin, extent);
    }
    ui.end_row();

    changed
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
            self.refresh();
        }
        ui.ctx().request_repaint_after(REFRESH_INTERVAL);
        let colors = theme::colors(ui);

        // Die Seitenleiste lässt dem Editor immer seine Mindestbreite.
        let sidebar_max = (ui.available_width() - EDITOR_MIN_WIDTH)
            .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH);
        egui::Panel::left("windows")
            .default_size(260.0)
            .size_range(SIDEBAR_MIN_WIDTH..=sidebar_max)
            .frame(
                egui::Frame::new()
                    .fill(colors.sidebar)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ui, |ui| self.window_list(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(colors.bg)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| self.editor(ui));
        self.settings_dialog(ui.ctx());
    }
}
