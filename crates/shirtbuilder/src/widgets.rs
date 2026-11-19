//! Widgets drawn to match the web app (segmented controls, switches, slider rows, sections,
//! icon buttons, swatches).

use crate::style::{icon, icons, semibold, P};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use lucide_icons::Icon;


/// size a segmented control will take (for centring it)
pub fn seg_size<T>(ui: &Ui, options: &[(T, &str)], small: bool) -> Vec2 {
    let font = FontId::proportional(if small { 12.0 } else { 13.0 });
    let pad = if small { vec2(10.0, 3.0) } else { vec2(12.0, 4.0) };
    let w: f32 = options.iter().map(|(_, l)| ui.fonts_mut(|f| f.layout_no_wrap(l.to_string(), font.clone(), P.text).size().x) + pad.x * 2.0).sum();
    let h = ui.fonts_mut(|f| f.row_height(&font)) + pad.y * 2.0;
    vec2(w + 2.0 * (options.len() as f32 + 1.0), h + 4.0)
}

/// segmented control. `small` = the accent-filled variant used in pane headers and the sidebar
pub fn seg<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)], small: bool) -> bool {
    let font = FontId::proportional(if small { 12.0 } else { 13.0 });
    let pad = if small { vec2(10.0, 3.0) } else { vec2(12.0, 4.0) };
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, l)| ui.fonts_mut(|f| f.layout_no_wrap(l.to_string(), font.clone(), P.text).size().x) + pad.x * 2.0)
        .collect();
    let h = ui.fonts_mut(|f| f.row_height(&font)) + pad.y * 2.0;
    let total = vec2(widths.iter().sum::<f32>() + 2.0 * (options.len() as f32 + 1.0), h + 4.0);
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());
    let painter = ui.painter();
    painter.rect(rect, CornerRadius::same(8), P.panel2, Stroke::new(1.0, P.border), StrokeKind::Inside);
    let mut x = rect.left() + 2.0;
    let mut changed = false;
    for ((v, label), w) in options.iter().zip(&widths) {
        let r = egui::Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(*w, h));
        let resp = ui.interact(r, ui.auto_id_with(("seg", label)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let on = *value == *v;
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, *label));
        if on {
            let fill = if small { P.accent_strong } else { P.seg_on };
            ui.painter().rect_filled(r, CornerRadius::same(6), fill);
        }
        let color = if on { if small { Color32::WHITE } else { P.text } } else if resp.hovered() { P.text } else { P.muted };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *label, font.clone(), color);
        if resp.clicked() && !on {
            *value = *v;
            changed = true;
        }
        x += w + 2.0;
    }
    changed
}

/// iOS-style switch
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(34.0, 20.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *on, ""));
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = if *on { P.accent } else { P.panel3 };
    ui.painter().rect_filled(rect, CornerRadius::same(10), bg);
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, t);
    ui.painter().circle_filled(pos2(x, rect.center().y), 8.0, Color32::WHITE);
    resp
}

/// "label ……… [switch]" row
pub fn toggle_row(ui: &mut Ui, label: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let lbl = ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            changed = switch(ui, on).labelled_by(lbl.id).changed();
        });
    });
    changed
}

/// "label  [slider]  value" row, like the web sidebar
pub fn slider_row(ui: &mut Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>, step: f64, fmt: impl Fn(f64) -> String, enabled: bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(enabled, |ui| {
            // label cell and slider shrink to fit narrow places (the layer panel is inset)
            let avail = ui.available_width();
            let cell_w = (avail - 104.0 - 60.0).clamp(64.0, 118.0);
            ui.spacing_mut().slider_width = (avail - cell_w - 60.0).clamp(60.0, 104.0);
            let (cell, lbl) = ui.allocate_exact_size(vec2(cell_w, 20.0), Sense::hover());
            let color = if enabled { P.text } else { P.muted };
            ui.painter().text(cell.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(13.0), color);
            lbl.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, label));
            changed = ui.add(egui::Slider::new(v, range).show_value(false).step_by(step)).labelled_by(lbl.id).changed();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(fmt(*v)).color(P.muted));
            });
        });
    });
    changed
}

/// a sidebar section: title (+ optional subtitle and action icon), contents, divider
pub fn section(ui: &mut Ui, title: &str, sub: Option<&str>, action: Option<(Icon, &str)>, add: impl FnOnce(&mut Ui)) -> bool {
    let mut acted = false;
    egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 16, top: 14, bottom: 16 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).family(semibold()).size(13.5));
            if let Some((ic, tip)) = action {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    acted = icon_button(ui, ic, tip, true).clicked();
                });
            }
        });
        if let Some(s) = sub {
            ui.label(RichText::new(s).size(11.5).color(P.muted));
        }
        ui.add_space(6.0);
        add(ui);
    });
    let r = ui.min_rect();
    ui.painter().hline(r.x_range(), ui.cursor().top(), Stroke::new(1.0, P.border));
    acted
}

/// square icon button (top bar, layer rows)
pub fn icon_button(ui: &mut Ui, ic: Icon, tooltip: &str, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), if enabled { Sense::click() } else { Sense::hover() });
    let resp = resp.on_hover_text(tooltip);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tooltip));
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(7), P.panel2);
    }
    let color = if !enabled { P.muted.gamma_multiply(0.45) } else if resp.hovered() { P.text } else { P.muted };
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon(ic), FontId::new(16.0, icons()), color);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BtnKind {
    Normal,
    Primary,
}

/// text button with an optional leading icon (top bar, sidebar)
pub fn button(ui: &mut Ui, ic: Option<Icon>, text: &str, kind: BtnKind, enabled: bool) -> Response {
    let font = FontId::proportional(13.0);
    let tw = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), P.text).size().x);
    let iw = if ic.is_some() { 21.0 } else { 0.0 };
    let size = vec2(tw + iw + 24.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, if text.is_empty() { "More" } else { text }));
    let hovered = enabled && resp.hovered();
    let (fill, stroke, fg) = match kind {
        BtnKind::Primary => (if hovered { P.accent } else { P.accent_strong }, Stroke::NONE, Color32::WHITE),
        BtnKind::Normal => (if hovered { P.panel3 } else { P.panel2 }, Stroke::new(1.0, P.border), P.text),
    };
    let fg = if enabled { fg } else { fg.gamma_multiply(0.4) };
    ui.painter().rect(rect, CornerRadius::same(7), fill, stroke, StrokeKind::Inside);
    let mut x = rect.left() + 12.0;
    if let Some(ic) = ic {
        ui.painter().text(pos2(x, rect.center().y), Align2::LEFT_CENTER, icon(ic), FontId::new(15.0, icons()), fg);
        x += iw;
    }
    ui.painter().text(pos2(x, rect.center().y), Align2::LEFT_CENTER, text, font, fg);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// width of a [`button`] with this icon / text, for layouts that must know it in advance
pub fn button_width(ui: &Ui, ic: bool, text: &str) -> f32 {
    let tw = if text.is_empty() { 0.0 } else { ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), FontId::proportional(13.0), P.text).size().x) };
    tw + if ic { 21.0 } else { 0.0 } + 24.0
}

pub const ICON_BUTTON_FRAMED_W: f32 = 36.0;

/// a [`button`] showing only its icon (compact top bar); `name` is its tooltip and accessible name
pub fn icon_button_framed(ui: &mut Ui, ic: Icon, name: &str, kind: BtnKind, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ICON_BUTTON_FRAMED_W, 30.0), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, name));
    let hovered = enabled && resp.hovered();
    let (fill, stroke, fg) = match kind {
        BtnKind::Primary => (if hovered { P.accent } else { P.accent_strong }, Stroke::NONE, Color32::WHITE),
        BtnKind::Normal => (if hovered { P.panel3 } else { P.panel2 }, Stroke::new(1.0, P.border), P.text),
    };
    let fg = if enabled { fg } else { fg.gamma_multiply(0.4) };
    ui.painter().rect(rect, CornerRadius::same(7), fill, stroke, StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon(ic), FontId::new(15.0, icons()), fg);
    let resp = resp.on_hover_text(name);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// big full-width button (Artwork: Image / Text)
pub fn big_button(ui: &mut Ui, ic: Icon, text: &str, width: f32, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 36.0), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, text));
    let hovered = enabled && resp.hovered();
    ui.painter().rect(rect, CornerRadius::same(8), if hovered { P.panel3 } else { P.panel2 }, Stroke::new(1.0, P.border), StrokeKind::Inside);
    let fg = if enabled { P.text } else { P.muted };
    let font = FontId::proportional(13.0);
    let tw = ui.fonts_mut(|f| f.layout_no_wrap(text.to_string(), font.clone(), fg).size().x);
    let x0 = rect.center().x - (tw + 22.0) / 2.0;
    ui.painter().text(pos2(x0, rect.center().y), Align2::LEFT_CENTER, icon(ic), FontId::new(15.0, icons()), fg);
    ui.painter().text(pos2(x0 + 22.0, rect.center().y), Align2::LEFT_CENTER, text, font, fg);
    resp
}

/// round colour swatch; `on` draws the selection ring
pub fn swatch(ui: &mut Ui, color: Color32, on: bool, tooltip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, tooltip));
    let c = rect.center();
    if on {
        ui.painter().circle_stroke(c, 13.5, Stroke::new(2.0, P.accent));
    }
    ui.painter().circle(c, 11.0, color, Stroke::new(1.0, Color32::from_white_alpha(40)));
    resp.on_hover_text(tooltip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// muted helper text
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(P.muted));
}

pub fn hex_color(c: &str) -> Color32 {
    let h = c.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("80"), 16).unwrap_or(128);
    Color32::from_rgb(p(0), p(2), p(4))
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}
