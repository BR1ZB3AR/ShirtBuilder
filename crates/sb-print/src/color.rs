//! Colour helpers (the web app's `hexToRgb`, `shade`, `mix`, `luminance`).

use resvg::tiny_skia::Color;

pub fn hex_to_rgb(hex: &str) -> [f64; 3] {
    let mut h: String = hex.trim().trim_start_matches('#').to_string();
    if h.len() == 3 {
        h = h.chars().flat_map(|c| [c, c]).collect();
    }
    let n = u32::from_str_radix(h.get(0..6).unwrap_or(""), 16).unwrap_or(0);
    [((n >> 16) & 255) as f64, ((n >> 8) & 255) as f64, (n & 255) as f64]
}

pub fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let c = |v: f64| (v.clamp(0.0, 255.0)).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
}

pub fn shade(hex: &str, k: f64) -> String {
    let [r, g, b] = hex_to_rgb(hex);
    rgb_to_hex(r * k, g * k, b * k)
}

pub fn mix(a: &str, b: &str, t: f64) -> String {
    let (x, y) = (hex_to_rgb(a), hex_to_rgb(b));
    rgb_to_hex(x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t)
}

pub fn luminance(hex: &str) -> f64 {
    let [r, g, b] = hex_to_rgb(hex);
    (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
}

/// a tiny-skia colour from `#rrggbb` and an alpha
pub fn color(hex: &str, alpha: f64) -> Color {
    let [r, g, b] = hex_to_rgb(hex);
    Color::from_rgba8(r as u8, g as u8, b as u8, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

pub fn rgba(r: u8, g: u8, b: u8, a: f64) -> Color {
    Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}
