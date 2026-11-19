//! Look & feel matching the web app: Inter text, Lucide icons, the "Forest Dark" colours,
//! rounded flat widgets.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

/// special looks: frosted glass panels over glowing colour, or a neon grid
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fx {
    None,
    Glass,
    Cyber,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // some fields are for future use / debugging
pub struct Palette {
    pub dark: bool,
    pub hc: bool,
    pub fx: Fx,
    pub bg: Color32,
    pub panel: Color32,
    pub panel2: Color32,
    pub panel3: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_strong: Color32,
    pub accent_weak: Color32,
    /// text on an accent-filled button
    pub accent_ink: Color32,
    pub accent2: Color32,
    pub seg_on: Color32,
    pub canvas: Color32,
    pub grid: Color32,
    pub ruler: Color32,
    pub ruler_tick: Color32,
    pub piece_line: Color32,
    pub chip: Color32,
    /// menus and dialogs (nearly solid even on glass themes)
    pub menu: Color32,
    /// glowing / glass panel edge
    pub edge: Color32,
    pub blobs: [Color32; 3],
}

/// the current theme's palette, read anywhere as `P.accent`, `P.panel`, …
pub struct CurrentPalette;
pub static P: CurrentPalette = CurrentPalette;
static CURRENT: AtomicPtr<Palette> = AtomicPtr::new(std::ptr::null_mut());

impl std::ops::Deref for CurrentPalette {
    type Target = Palette;
    fn deref(&self) -> &Palette {
        let p = CURRENT.load(Ordering::Acquire);
        if p.is_null() {
            default_palette()
        } else {
            // SAFETY: palettes are leaked on install and never freed
            unsafe { &*p }
        }
    }
}

fn default_palette() -> &'static Palette {
    static D: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
    D.get_or_init(|| crate::themes::palette(&crate::themes::THEMES[0], None))
}

/// make `pal` the current palette (a few hundred bytes leaked per theme change)
pub fn set_palette(pal: Palette) {
    CURRENT.store(Box::into_raw(Box::new(pal)), Ordering::Release);
}

pub const INTER: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}
pub fn icons() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// a Lucide icon as text
pub fn icon(i: lucide_icons::Icon) -> String {
    char::from(i).to_string()
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("inter".into(), Arc::new(FontData::from_static(INTER)));
    fonts.font_data.insert("inter-semibold".into(), Arc::new(FontData::from_static(INTER_SEMIBOLD)));
    fonts.font_data.insert("lucide".into(), Arc::new(FontData::from_static(lucide_icons::LUCIDE_FONT_BYTES)));
    // Lucide stays out of the text families: its ligatures would turn words like "3D" into icons.
    // Icons are always drawn with the dedicated `icons()` family.
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    fonts.families.insert(semibold(), vec!["inter-semibold".into(), "inter".into()]);
    fonts.families.insert(icons(), vec!["lucide".into()]);
    // the text-artwork fonts, so the font menu can show each name in its own typeface
    for f in sb_print::text::FONTS {
        fonts.font_data.insert(format!("art-{f}"), Arc::new(FontData::from_static(sb_print::text::font_bytes(f, false))));
        fonts.families.insert(FontFamily::Name(f.into()), vec![format!("art-{f}"), "inter".into()]);
    }
    ctx.set_fonts(fonts);
}

/// make `p` the current palette and restyle egui's own widgets with it
pub fn apply(ctx: &egui::Context, p: &Palette) {
    set_palette(*p);
    let p = *p;
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.0)),
            (TextStyle::Body, FontId::proportional(13.0)),
            (TextStyle::Button, FontId::proportional(13.0)),
            (TextStyle::Heading, FontId::new(15.0, semibold())),
            (TextStyle::Monospace, FontId::monospace(12.0)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.slider_width = 104.0;
        style.spacing.combo_width = 140.0;

        let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
        v.panel_fill = p.panel;
        v.window_fill = p.panel;
        v.window_stroke = Stroke::new(1.0, p.border);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        v.extreme_bg_color = p.bg;
        v.faint_bg_color = p.panel2;
        v.override_text_color = Some(p.text);
        v.selection.bg_fill = p.accent_strong;
        v.selection.stroke = Stroke::new(1.0, p.text);
        v.hyperlink_color = p.accent;
        v.slider_trailing_fill = true;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = CornerRadius::same(7);
        }
        v.widgets.inactive.weak_bg_fill = p.panel2;
        v.widgets.inactive.bg_fill = p.panel3;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
        v.widgets.hovered.weak_bg_fill = p.panel3;
        v.widgets.hovered.bg_fill = p.accent;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.active.weak_bg_fill = p.seg_on;
        v.widgets.active.bg_fill = p.accent;
        v.widgets.open.weak_bg_fill = p.panel3;
        v.window_fill = p.menu;
        v.widgets.noninteractive.weak_bg_fill = p.panel2;
        v.selection.stroke = Stroke::new(1.0, p.accent_ink);
        v.text_edit_bg_color = Some(p.panel2);
        style.visuals = v;
    });
}
