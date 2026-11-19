//! The web app's 30 themes (`themes.ts`): standard light / dark, high contrast, Liquid Glass
//! (translucent panels over glowing colour) and Cyberpunk (a neon grid under a dark UI), each turned
//! into the UI palette the same way the web app derives its CSS variables.

use crate::style::{Fx, Palette};
use egui::Color32;
use sb_print::color::{hex_to_rgb, mix, shade};

pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
    pub hc: bool,
    pub bg: &'static str,
    pub panel: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub accent: &'static str,
    pub canvas: Option<&'static str>,
    pub fx: Fx,
    pub accent2: Option<&'static str>,
    pub blobs: [&'static str; 3],
}

#[allow(clippy::too_many_arguments)]
const fn t(id: &'static str, name: &'static str, dark: bool, bg: &'static str, panel: &'static str, text: &'static str, muted: &'static str, accent: &'static str) -> Theme {
    Theme { id, name, dark, hc: false, bg, panel, text, muted, accent, canvas: None, fx: Fx::None, accent2: None, blobs: ["", "", ""] }
}
const fn hc(mut th: Theme) -> Theme {
    th.hc = true;
    th
}
const fn glass(mut th: Theme, accent2: &'static str, blobs: [&'static str; 3]) -> Theme {
    th.fx = Fx::Glass;
    th.accent2 = Some(accent2);
    th.blobs = blobs;
    th
}
const fn cyber(mut th: Theme, accent2: &'static str) -> Theme {
    th.fx = Fx::Cyber;
    th.accent2 = Some(accent2);
    th
}

pub const THEMES: [Theme; 30] = [
    // dark
    t("forest-dark", "Forest Dark", true, "#16181a", "#1d2022", "#dce0dc", "#858c87", "#6f9f7a"),
    t("one-dark", "One Dark", true, "#21252b", "#282c34", "#d7dae0", "#7f848e", "#61afef"),
    t("tokyo-night", "Tokyo Night", true, "#16161e", "#1a1b26", "#c0caf5", "#737aa2", "#7aa2f7"),
    t("gruvbox-dark", "Gruvbox Dark", true, "#1d2021", "#282828", "#ebdbb2", "#a89984", "#d79921"),
    t("dracula", "Dracula", true, "#21222c", "#282a36", "#f8f8f2", "#8b8fa8", "#bd93f9"),
    t("nord", "Nord", true, "#272c36", "#2e3440", "#e5e9f0", "#8e98aa", "#88c0d0"),
    Theme { canvas: Some("#0e2b52"), ..t("blueprint", "Blueprint", true, "#0b2545", "#10305a", "#e3eefc", "#86a5cc", "#4fb3ff") },
    t("graphite", "Graphite", true, "#141414", "#1c1c1c", "#e6e6e6", "#8c8c8c", "#e0a84f"),
    t("espresso", "Espresso", true, "#1a1512", "#231c18", "#eee2d6", "#9c8b7d", "#d98a4e"),
    t("neon", "Neon Night", true, "#150f1f", "#1e1530", "#ece3fb", "#9285ad", "#c06cff"),
    hc(t("green-phosphor", "Green Phosphor", true, "#050c06", "#0a140b", "#7dff8f", "#3f9a4a", "#39ff5a")),
    hc(t("one-bit-dark", "1-bit Dark", true, "#000000", "#000000", "#ffffff", "#c8c8c8", "#ffffff")),
    hc(t("hc-amber", "High Contrast Amber", true, "#000000", "#0a0a0a", "#ffffff", "#c4c4c4", "#ffd400")),
    // light
    t("daylight", "Daylight", false, "#eceeec", "#ffffff", "#1e2421", "#6b736e", "#4f8a5d"),
    t("platinum", "Platinum 9", false, "#dddddd", "#eeeeee", "#111111", "#5d5d5d", "#6c5fc7"),
    t("cupertino", "Cupertino", false, "#ececef", "#fbfbfd", "#1d1d1f", "#6e6e73", "#0a84ff"),
    t("solarized-light", "Solarized Light", false, "#eee8d5", "#fdf6e3", "#3c4a50", "#7d8b8c", "#268bd2"),
    t("gruvbox-light", "Gruvbox Light", false, "#ebdbb2", "#fbf1c7", "#3c3836", "#7c6f64", "#af3a03"),
    t("ocean", "Ocean", false, "#e8eef5", "#ffffff", "#172231", "#5f6f84", "#2f7fd1"),
    t("rose", "Rosé", false, "#f5ecee", "#fffafb", "#2d1d22", "#866a72", "#d0567a"),
    hc(t("one-bit-light", "1-bit Light", false, "#ffffff", "#ffffff", "#000000", "#3a3a3a", "#000000")),
    // liquid glass: translucent frosted panels floating over soft glowing colour
    glass(t("liquid-glass", "Liquid Glass", true, "#0d1230", "#1a2150", "#eef2ff", "#a3acd6", "#8fd0ff"), "#ffb680", ["#2b4bd1", "#7a2fb8", "#0f6fa8"]),
    glass(t("liquid-glass-light", "Liquid Glass Light", false, "#eef1fb", "#ffffff", "#141a33", "#5d6688", "#2156e8"), "#a33b16", ["#b9c9ff", "#f6c6e6", "#bfeaff"]),
    glass(t("liquid-glass-green", "Green Liquid Glass", true, "#06180f", "#0e2a1b", "#ecfff3", "#9cc8ae", "#86efac"), "#e7ff8c", ["#127a45", "#0b4f6b", "#3c7a12"]),
    glass(t("liquid-glass-pink", "Liquid Glass Pink", true, "#1d0817", "#33112b", "#fff0f8", "#d6a3c3", "#ffb3d9"), "#ffb35c", ["#b0246e", "#5b1a8a", "#d0456e"]),
    glass(t("liquid-glass-amber", "Liquid Glass Amber", true, "#1a1205", "#2e2108", "#fff7e6", "#d6c08f", "#ffd76a"), "#ffc38a", ["#b06a12", "#7a4a06", "#c99a1c"]),
    // cyberpunk: near-black with a neon grid, glowing edges and a neon accent pair
    cyber(t("cyber-magenta", "Cyberpunk Magenta", true, "#0b0414", "#140a22", "#f6e9ff", "#a88fc4", "#ff2bd6"), "#00e5ff"),
    cyber(t("cyber-red-cyan", "Cyberpunk Red Cyan", true, "#0d0407", "#1a080d", "#ffeef1", "#c4949c", "#00e5ff"), "#ff2e4d"),
    cyber(t("cyber-green-cyan", "Cyberpunk Green Cyan", true, "#030d07", "#08190f", "#eafff1", "#8fc4a2", "#00e5ff"), "#39ff6a"),
    cyber(t("cyber-orange-cyan", "Cyberpunk Orange Cyan", true, "#0e0803", "#1c1006", "#fff4e8", "#c8a68a", "#00e5ff"), "#ff8a1f"),
];

/// accent colours offered under the theme gallery
pub const ACCENTS: [&str; 8] = ["#6f9f7a", "#61afef", "#7aa2f7", "#bd93f9", "#d0567a", "#e5673f", "#d79921", "#3fb8af"];

pub fn theme(id: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0])
}

pub fn c(hex: &str) -> Color32 {
    let [r, g, b] = hex_to_rgb(hex);
    Color32::from_rgb(r as u8, g as u8, b as u8)
}
pub fn ca(hex: &str, a: f64) -> Color32 {
    let [r, g, b] = hex_to_rgb(hex);
    Color32::from_rgba_unmultiplied(r as u8, g as u8, b as u8, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}
fn white(a: f64) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, (a * 255.0).round() as u8)
}

/// relative luminance (WCAG) of a colour
fn luminance_of(hex: &str) -> f64 {
    let ch = |v: f64| {
        let s = v / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = hex_to_rgb(hex);
    0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
}

/// the UI palette of a theme (+ optional accent override) — the web app's `themeVars`
pub fn palette(th: &Theme, accent_override: Option<&str>) -> Palette {
    let dark = th.dark;
    let acc = accent_override.unwrap_or(th.accent);
    let k: f64 = if th.hc { 2.4 } else { 1.0 };
    let mut p = Palette {
        dark,
        hc: th.hc,
        fx: th.fx,
        bg: c(th.bg),
        panel: c(th.panel),
        text: c(th.text),
        muted: c(th.muted),
        accent: c(acc),
        accent_strong: c(&shade(acc, if dark { 0.86 } else { 0.88 })),
        accent_weak: ca(acc, if th.hc { 0.28 } else if dark { 0.16 } else { 0.13 }),
        accent_ink: if luminance_of(acc) > 0.55 { c("#111111") } else { Color32::WHITE },
        accent2: c(th.accent2.unwrap_or(acc)),
        panel2: Color32::TRANSPARENT,
        panel3: Color32::TRANSPARENT,
        border: Color32::TRANSPARENT,
        seg_on: Color32::TRANSPARENT,
        canvas: Color32::TRANSPARENT,
        grid: Color32::TRANSPARENT,
        ruler: Color32::TRANSPARENT,
        ruler_tick: Color32::TRANSPARENT,
        piece_line: Color32::TRANSPARENT,
        chip: Color32::TRANSPARENT,
        menu: c(th.panel),
        edge: Color32::TRANSPARENT,
        blobs: [Color32::TRANSPARENT; 3],
    };
    if dark {
        p.panel2 = c(&mix(th.panel, th.text, 0.05 * k.min(1.6)));
        p.panel3 = c(&mix(th.panel, th.text, 0.09 * k.min(1.6)));
        p.border = c(&mix(th.panel, th.text, (0.1 * k).min(0.5)));
        p.seg_on = c(&mix(th.panel, th.text, if th.hc { 0.22 } else { 0.13 }));
        p.canvas = c(&th.canvas.map(String::from).unwrap_or_else(|| mix(th.bg, th.panel, 0.35)));
        p.grid = ca(th.text, 0.1);
        p.ruler = c(th.panel);
        p.ruler_tick = c(&mix(th.panel, th.text, if th.hc { 0.6 } else { 0.25 }));
        p.piece_line = ca(th.text, if th.hc { 0.75 } else { 0.32 });
        p.chip = ca(th.panel, if th.hc { 0.9 } else { 0.62 });
    } else {
        p.panel2 = c(&mix(th.panel, th.bg, 0.6));
        p.panel3 = c(&mix(th.bg, th.text, if th.hc { 0.08 } else { 0.03 }));
        p.border = c(&mix(th.bg, th.text, (0.08 * k).min(0.6)));
        p.seg_on = if th.hc { c(&mix(th.panel, th.text, 0.12)) } else { c(th.panel) };
        p.canvas = c(&th.canvas.map(String::from).unwrap_or_else(|| mix(th.bg, th.text, 0.03)));
        p.grid = ca(th.text, 0.12);
        p.ruler = c(&mix(th.panel, th.bg, 0.3));
        p.ruler_tick = c(&mix(th.bg, th.text, if th.hc { 0.7 } else { 0.3 }));
        p.piece_line = ca(th.text, if th.hc { 0.8 } else { 0.32 });
        p.chip = ca(th.panel, if th.hc { 0.95 } else { 0.7 });
    }
    match th.fx {
        Fx::Glass => {
            p.blobs = th.blobs.map(|b| ca(b, if dark { 0.85 } else { 0.9 }));
            p.panel = if dark { ca(th.panel, 0.42) } else { white(0.55) };
            p.panel2 = if dark { white(0.07) } else { white(0.55) };
            p.panel3 = if dark { white(0.12) } else { white(0.8) };
            p.border = if dark { white(0.14) } else { white(0.85) };
            p.seg_on = if dark { white(0.18) } else { white(0.95) };
            p.canvas = if dark { Color32::from_rgba_unmultiplied(8, 10, 24, 71) } else { white(0.3) };
            p.ruler = if dark { white(0.05) } else { white(0.5) };
            p.chip = if dark { Color32::from_rgba_unmultiplied(20, 24, 50, 115) } else { white(0.6) };
            p.edge = if dark { white(0.22) } else { white(0.95) };
            p.menu = if dark { ca(&mix(th.bg, th.panel, 0.5), 0.94) } else { Color32::from_rgba_unmultiplied(250, 251, 255, 242) };
        }
        Fx::Cyber => {
            let a2 = th.accent2.unwrap_or(acc);
            p.panel = ca(th.panel, 0.82);
            p.panel2 = ca(&mix(th.panel, acc, 0.08), 0.9);
            p.panel3 = ca(&mix(th.panel, acc, 0.16), 0.9);
            p.border = ca(acc, 0.32);
            p.seg_on = ca(acc, 0.22);
            p.canvas = ca(th.bg, 0.55);
            p.ruler = ca(th.panel, 0.85);
            p.chip = ca(th.panel, 0.75);
            p.grid = ca(a2, 0.22);
            p.piece_line = ca(acc, 0.55);
            p.menu = ca(&mix(th.bg, th.panel, 0.6), 0.97);
            p.edge = ca(acc, 0.55);
        }
        Fx::None => {}
    }
    p
}

/// the app background behind translucent panels: glowing blobs (glass) or a neon grid (cyber)
pub fn paint_app_background(painter: &egui::Painter, r: egui::Rect, th: &Theme, p: &Palette) {
    painter.rect_filled(r, 0.0, c(th.bg));
    match th.fx {
        Fx::Glass => {
            // radial-gradient(60% 55% at 12% 18%) … as soft elliptical glows
            for (i, (cx, cy, rx, ry)) in [(0.12, 0.18, 0.6, 0.55), (0.88, 0.26, 0.55, 0.6), (0.55, 1.05, 0.7, 0.6)].into_iter().enumerate() {
                glow(painter, egui::pos2(r.left() + r.width() * cx, r.top() + r.height() * cy), r.width() * rx * 0.7, r.height() * ry * 0.7, p.blobs[i]);
            }
        }
        Fx::Cyber => {
            let a2 = th.accent2.unwrap_or(th.accent);
            glow(painter, egui::pos2(r.center().x, r.bottom() + r.height() * 0.15), r.width() * 0.9 * 0.6, r.height() * 0.6 * 0.6, ca(a2, 0.28));
            glow(painter, egui::pos2(r.center().x, r.top() - r.height() * 0.2), r.width() * 0.7 * 0.6, r.height() * 0.5 * 0.6, ca(th.accent, 0.18));
            let line = egui::Stroke::new(1.0, ca(a2, 0.16));
            let mut x = r.left();
            while x < r.right() {
                painter.vline(x, r.y_range(), line);
                x += 32.0;
            }
            let mut y = r.top();
            while y < r.bottom() {
                painter.hline(r.x_range(), y, line);
                y += 32.0;
            }
        }
        Fx::None => {}
    }
}

/// an elliptical radial glow fading to transparent (triangle fan with a soft falloff ring)
pub fn glow(painter: &egui::Painter, c0: egui::Pos2, rx: f32, ry: f32, col: Color32) {
    let mut mesh = egui::Mesh::default();
    let n = 48;
    let rings = [(0.0f32, 1.0f32), (0.35, 0.8), (0.7, 0.35), (1.0, 0.0)];
    for &(rr, a) in &rings {
        for i in 0..n {
            let t = i as f32 / n as f32 * std::f32::consts::TAU;
            let p = c0 + egui::vec2(t.cos() * rx * rr, t.sin() * ry * rr);
            mesh.colored_vertex(p, col.gamma_multiply(a));
        }
    }
    for ring in 0..rings.len() - 1 {
        for i in 0..n {
            let a = (ring * n + i) as u32;
            let b = (ring * n + (i + 1) % n) as u32;
            let c2 = ((ring + 1) * n + i) as u32;
            let d = ((ring + 1) * n + (i + 1) % n) as u32;
            mesh.add_triangle(a, b, c2);
            mesh.add_triangle(b, d, c2);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}
