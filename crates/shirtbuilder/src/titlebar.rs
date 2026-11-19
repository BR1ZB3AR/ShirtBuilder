//! The window's own title bar on the desktop (the window has no OS decorations), like SolidCraft:
//! empty space in the top bar drags the window and a double-click there maximizes or restores it,
//! caption buttons at the far right minimize, maximize/restore and close, and thin invisible zones
//! along the window's edges resize it. Dragging and resizing use the OS's own move/resize loops,
//! so window snapping keeps working. The web build has none of this (the browser owns the window).
//!
//! Adapted from SolidCraft's `crates/ui-egui/src/titlebar.rs` (MIT OR Apache-2.0).

use crate::style::P;
use egui::{pos2, vec2, Color32, CursorIcon, Id, LayerId, Order, PointerButton, Pos2, Rect, ResizeDirection, Sense, Stroke, Ui, ViewportCommand};

/// One caption button: as wide as the native Windows ones.
pub const BUTTON_WIDTH: f32 = 46.0;
/// The three caption buttons together.
pub const WIDTH: f32 = 3.0 * BUTTON_WIDTH;
/// Resize border at 100 % scale, in physical pixels.
pub const RESIZE_BORDER_AT_96_DPI: f32 = 8.0;

/// whether this build draws its own title bar (desktop only)
pub const ENABLED: bool = cfg!(not(target_arch = "wasm32"));

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Caption {
    Minimize,
    Maximize,
    Close,
}

/// Left to right.
pub const CAPTIONS: [Caption; 3] = [Caption::Minimize, Caption::Maximize, Caption::Close];

pub fn caption_id(c: Caption) -> Id {
    Id::unique(("titlebar-caption", c as u8))
}

fn maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

pub fn toggle_maximize(ctx: &egui::Context) {
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized(ctx)));
}

/// The bar's background: a press-and-drag on empty space moves the window, a double-click
/// toggles maximize. Call it before laying out the bar so its buttons, registered later, take
/// their clicks.
pub fn drag_area(ui: &Ui, rect: Rect) {
    let ctx = ui.ctx();
    let resp = ui.interact(rect, Id::unique("titlebar-drag"), Sense::click_and_drag());
    // egui hands a drag that starts on a click-only widget to the drag-sensing background below
    // it, so remember whether the press itself landed on bare background.
    if ctx.input(|i| i.pointer.any_pressed()) {
        let bare = ctx.viewport(|v| v.hits.click.is_some_and(|w| w.id == resp.id));
        ctx.data_mut(|d| d.insert_temp(resp.id, bare));
    }
    if resp.double_clicked() {
        toggle_maximize(ctx);
    } else if resp.drag_started_by(PointerButton::Primary) && ctx.data(|d| d.get_temp(resp.id).unwrap_or(false)) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// the minimize / maximize / restore / close glyphs, 10 px, Windows-style
fn paint_glyph(ui: &Ui, c: Caption, max: bool, at: Pos2, stroke: Stroke) {
    let p = ui.painter();
    let h = 5.0;
    match c {
        Caption::Minimize => {
            p.line_segment([at + vec2(-h, 0.5), at + vec2(h, 0.5)], stroke);
        }
        Caption::Maximize if max => {
            // restore: a square with another peeking out behind it
            let front = Rect::from_center_size(at + vec2(-1.0, 1.0), vec2(8.0, 8.0));
            p.rect_stroke(front, 1.0, stroke, egui::StrokeKind::Middle);
            let (a, b, c2) = (front.left_top() + vec2(2.0, -2.0), front.right_top() + vec2(2.0, -2.0), front.right_bottom() + vec2(2.0, -2.0));
            p.line(vec![a, b, c2], stroke);
        }
        Caption::Maximize => {
            p.rect_stroke(Rect::from_center_size(at, vec2(2.0 * h, 2.0 * h)), 1.0, stroke, egui::StrokeKind::Middle);
        }
        Caption::Close => {
            p.line_segment([at + vec2(-h, -h), at + vec2(h, h)], stroke);
            p.line_segment([at + vec2(-h, h), at + vec2(h, -h)], stroke);
        }
    }
}

/// Minimize, Maximize/Restore and Close at the right end of `bar` (the top bar's full rect),
/// full bar height, flush with the window's edge so a maximized window's corner hits Close.
pub fn caption_buttons(ui: &mut Ui, bar: Rect) {
    let ctx = ui.ctx().clone();
    let max = maximized(&ctx);
    let painter = ui.painter().with_clip_rect(bar);
    for (i, c) in CAPTIONS.into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(bar.right() - WIDTH + i as f32 * BUTTON_WIDTH, bar.top()), vec2(BUTTON_WIDTH, bar.height()));
        let resp = ui.interact(r, caption_id(c), Sense::click());
        let tip = match c {
            Caption::Minimize => "Minimize",
            Caption::Maximize if max => "Restore",
            Caption::Maximize => "Maximize",
            Caption::Close => "Close",
        };
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
        let down = resp.is_pointer_button_down_on();
        let hot = resp.hovered() || resp.has_focus();
        let close = c == Caption::Close;
        let fill = match (close, down, hot) {
            (true, true, _) => Color32::from_rgb(0x94, 0x1f, 0x14),
            (true, _, true) => Color32::from_rgb(0xc4, 0x2b, 0x1c),
            (false, true, _) => P.panel3,
            (false, _, true) => P.panel2,
            _ => Color32::TRANSPARENT,
        };
        painter.rect_filled(r, 0.0, fill);
        let ink = if close && (down || hot) { Color32::WHITE } else { P.text };
        paint_glyph(ui, c, max, r.center(), Stroke::new(1.0, ink));
        if resp.on_hover_text(tip).clicked() {
            match c {
                Caption::Minimize => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                Caption::Maximize => toggle_maximize(&ctx),
                Caption::Close => ctx.send_viewport_cmd(ViewportCommand::Close),
            }
        }
    }
}

/// The resize border in physical pixels at `scale` (1.0 = 96 DPI), never less than a pixel.
pub fn resize_border_px(scale: f32) -> f32 {
    let s = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    (RESIZE_BORDER_AT_96_DPI * s).round().max(1.0)
}

/// The edge and corner zones of a window with rect `w`, edges `edge` wide: (zone, direction,
/// cursor). Corners come last so they win where they overlap the edges.
pub fn zones(w: Rect, edge: f32) -> [(Rect, ResizeDirection, CursorIcon); 8] {
    use ResizeDirection::*;
    let (e, c) = (edge, 2.0 * edge);
    [
        (Rect::from_min_max(w.left_top(), pos2(w.right(), w.top() + e)), North, CursorIcon::ResizeVertical),
        (Rect::from_min_max(pos2(w.left(), w.bottom() - e), w.right_bottom()), South, CursorIcon::ResizeVertical),
        (Rect::from_min_max(w.left_top(), pos2(w.left() + e, w.bottom())), West, CursorIcon::ResizeHorizontal),
        (Rect::from_min_max(pos2(w.right() - e, w.top()), w.right_bottom()), East, CursorIcon::ResizeHorizontal),
        (Rect::from_min_size(w.left_top(), vec2(c, c)), NorthWest, CursorIcon::ResizeNwSe),
        (Rect::from_min_size(w.right_top() - vec2(c, 0.0), vec2(c, c)), NorthEast, CursorIcon::ResizeNeSw),
        (Rect::from_min_size(w.left_bottom() - vec2(0.0, c), vec2(c, c)), SouthWest, CursorIcon::ResizeNeSw),
        (Rect::from_min_size(w.right_bottom() - vec2(c, c), vec2(c, c)), SouthEast, CursorIcon::ResizeNwSe),
    ]
}

/// Invisible resize zones along the window's edges, above everything else; a press in one
/// starts an OS resize in that direction. None while the window is maximized or fullscreen.
pub fn resize_zones(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    if maximized(&ctx) || ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let w = ctx.content_rect();
    let ppp = ctx.pixels_per_point();
    let edge = resize_border_px(ctx.input(|i| i.viewport().native_pixels_per_point).unwrap_or(ppp)) / ppp;
    let layer = LayerId::new(Order::Foreground, Id::unique("titlebar-resize"));
    let zui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(w));
    for (i, (r, dir, cursor)) in zones(w, edge).into_iter().enumerate() {
        let resp = zui.interact(r, Id::unique(("titlebar-resize", i)), Sense::drag()).on_hover_cursor(cursor);
        if resp.drag_started_by(PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}

/// a thin outline round the undecorated window so it reads as a window on any wallpaper
pub fn window_outline(ctx: &egui::Context) {
    if maximized(ctx) || ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let r = ctx.content_rect();
    ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("window-outline"))).rect_stroke(r, 0.0, Stroke::new(1.0, P.border), egui::StrokeKind::Inside);
}
