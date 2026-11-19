//! Text artwork: the web app's fonts (bundled), shaped with rustybuzz (kerning, ligatures),
//! laid out like the web app's canvas text — at BASE px, line height × BASE, letter spacing
//! × BASE after every character, each line on the em-box middle — then fitted to the layer box.
//! Weights a font doesn't have are synthesised like a browser does (outline emboldened, or slanted
//! for italic).

use resvg::tiny_skia::{Path, PathBuilder};
use rustybuzz::{ttf_parser, Face, UnicodeBuffer, Variation};
use sb_core::doc::{Align, Layer};

/// px size text is laid out at before scaling into its box (as in the web app)
pub const BASE: f64 = 100.0;

const INTER_VAR: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/Inter-Variable.ttf");
const SPACE_GROTESK: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/SpaceGrotesk-Variable.ttf");
const PLAYFAIR: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/PlayfairDisplay-Variable.ttf");
const PLAYFAIR_ITALIC: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/PlayfairDisplay-Italic-Variable.ttf");
const PERMANENT_MARKER: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/PermanentMarker-Regular.ttf");
const ANTON: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/Anton-Regular.ttf");
const BEBAS: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/BebasNeue-Regular.ttf");
const BANGERS: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/Bangers-Regular.ttf");
const PACIFICO: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/Pacifico-Regular.ttf");
pub const INTER_REGULAR: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/Inter-Regular.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../../shirtbuilder/assets/fonts/Inter-SemiBold.ttf");

/// the text fonts offered in the editor (same list and order as the web app)
pub const FONTS: [&str; 8] = ["Permanent Marker", "Anton", "Bebas Neue", "Bangers", "Inter", "Space Grotesk", "Playfair Display", "Pacifico"];

/// raw font file for a family (UI previews use these too)
pub fn font_bytes(family: &str, italic: bool) -> &'static [u8] {
    match family {
        "Permanent Marker" => PERMANENT_MARKER,
        "Anton" => ANTON,
        "Bebas Neue" => BEBAS,
        "Bangers" => BANGERS,
        "Space Grotesk" => SPACE_GROTESK,
        "Playfair Display" if italic => PLAYFAIR_ITALIC,
        "Playfair Display" => PLAYFAIR,
        "Pacifico" => PACIFICO,
        _ => INTER_VAR,
    }
}

/// a face at a weight, with what has to be synthesised
pub struct FontPick {
    pub face: Face<'static>,
    pub fake_bold: bool,
    pub fake_italic: bool,
}

pub fn pick(family: &str, weight: f64, italic: bool) -> FontPick {
    let bytes = font_bytes(family, italic);
    let mut face = Face::from_slice(bytes, 0).unwrap_or_else(|| Face::from_slice(INTER_VAR, 0).expect("bundled font"));
    let wght = ttf_parser::Tag::from_bytes(b"wght");
    let variable = face.variation_axes().into_iter().any(|a| a.tag == wght);
    if variable {
        face.set_variations(&[Variation { tag: wght, value: weight as f32 }]);
    }
    let real_italic = family == "Playfair Display";
    FontPick { face, fake_bold: !variable && weight >= 600.0, fake_italic: italic && !real_italic }
}

struct Glyph {
    /// 0 = primary face, 1 = Inter fallback
    face: usize,
    id: u16,
    x: f64,
}

pub struct Line {
    pub width: f64,
    glyphs: Vec<Glyph>,
}

/// a laid-out text block at BASE px
pub struct TextBlock {
    pub w: f64,
    pub h: f64,
    pub lines: Vec<Line>,
    pub line_h: f64,
    /// px from the em-box middle down to the baseline
    pub mid_to_base: f64,
    pick: FontPick,
    fallback: Face<'static>,
    scale: f64,
    fallback_scale: f64,
}

/// shape one line; glyphs missing from the font come from Inter (like a browser's fallback)
fn shape_line(text: &str, f: &FontPick, fallback: &Face<'static>, scale: f64, fscale: f64, spacing: f64) -> Line {
    let mut buf = UnicodeBuffer::new();
    buf.push_str(text);
    let out = rustybuzz::shape(&f.face, &[], buf);
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut glyphs = Vec::new();
    let mut pen = 0.0;
    let infos = out.glyph_infos();
    let pos = out.glyph_positions();
    for (k, (info, p)) in infos.iter().zip(pos).enumerate() {
        let next_cluster = infos.get(k + 1).map(|n| n.cluster);
        let new_cluster_next = next_cluster != Some(info.cluster);
        if info.glyph_id == 0 {
            // .notdef: draw the characters of this cluster with the fallback font
            let start = info.cluster as usize;
            let end = next_cluster.map(|c| c as usize).filter(|&c| c > start).unwrap_or(text.len());
            for &(_, ch) in chars.iter().filter(|(i, _)| *i >= start && *i < end) {
                if let Some(gid) = fallback.glyph_index(ch) {
                    glyphs.push(Glyph { face: 1, id: gid.0, x: pen });
                    pen += fallback.glyph_hor_advance(gid).unwrap_or(0) as f64 * fscale + spacing;
                } else if !ch.is_whitespace() {
                    pen += BASE * 0.5 + spacing;
                }
            }
            continue;
        }
        glyphs.push(Glyph { face: 0, id: info.glyph_id as u16, x: pen + p.x_offset as f64 * scale });
        pen += p.x_advance as f64 * scale;
        if new_cluster_next {
            pen += spacing;
        }
    }
    Line { width: pen, glyphs }
}

pub fn layout(l: &Layer) -> TextBlock {
    let family = l.font.as_deref().unwrap_or("Inter");
    let pick = pick(family, l.weight.unwrap_or(400.0), l.italic.unwrap_or(false));
    let fallback = Face::from_slice(INTER_VAR, 0).expect("bundled font");
    let scale = BASE / pick.face.units_per_em() as f64;
    let fscale = BASE / fallback.units_per_em() as f64;
    let spacing = l.letter_spacing.unwrap_or(0.0) * BASE;
    let text = l.text.as_deref().unwrap_or("");
    let lines: Vec<Line> = text
        .split('\n')
        .map(|s| {
            let mut line = shape_line(if s.is_empty() { " " } else { s }, &pick, &fallback, scale, fscale, spacing);
            line.width = line.width.max(1.0);
            line
        })
        .collect();
    let line_h = BASE * l.line_height.unwrap_or(1.1);
    let w = lines.iter().map(|l| l.width).fold(f64::MIN, f64::max);
    let h = (line_h * lines.len() as f64).max(1.0);
    let asc = pick.face.ascender() as f64 * scale;
    let desc = -(pick.face.descender() as f64) * scale;
    TextBlock { w, h, lines, line_h, mid_to_base: (asc - desc) / 2.0, pick, fallback, scale, fallback_scale: fscale }
}

/// aspect (h / w) of a text layer's natural box
pub fn text_aspect(l: &Layer) -> f64 {
    let b = layout(l);
    b.h / b.w
}

struct Outline<'a> {
    pb: &'a mut PathBuilder,
    x: f64,
    y: f64,
    s: f64,
    skew: f64,
}
impl Outline<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (x as f64, y as f64);
        ((self.x + (x + self.skew * y) * self.s) as f32, (self.y - y * self.s) as f32)
    }
}
impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.pb.move_to(a, b);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (a, b) = self.p(x, y);
        self.pb.line_to(a, b);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, d) = self.p(x, y);
        self.pb.quad_to(a, b, c, d);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.p(x1, y1);
        let (c, d) = self.p(x2, y2);
        let (e, f) = self.p(x, y);
        self.pb.cubic_to(a, b, c, d, e, f);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

impl TextBlock {
    pub fn fake_bold(&self) -> bool {
        self.pick.fake_bold
    }
    /// outline of the whole block in BASE px, origin at the block centre (as the web app draws it)
    pub fn path(&self, align: Align) -> Option<Path> {
        let mut pb = PathBuilder::new();
        let skew = if self.pick.fake_italic { 0.25 } else { 0.0 };
        for (i, line) in self.lines.iter().enumerate() {
            let y_mid = -self.h / 2.0 + self.line_h * (i as f64 + 0.5);
            let baseline = y_mid + self.mid_to_base;
            let x0 = match align {
                Align::Left => -self.w / 2.0,
                Align::Right => self.w / 2.0 - line.width,
                Align::Center => -line.width / 2.0,
            };
            for g in &line.glyphs {
                let (face, s) = if g.face == 0 { (&self.pick.face, self.scale) } else { (&self.fallback, self.fallback_scale) };
                let mut o = Outline { pb: &mut pb, x: x0 + g.x, y: baseline, s, skew };
                face.outline_glyph(ttf_parser::GlyphId(g.id), &mut o);
            }
        }
        pb.finish()
    }
}

/// a one-line label (pattern marks, export sheets): outline at `size` px with the baseline at y,
/// anchored left / centre / right of x
pub fn label_path(text: &str, family: &str, weight: f64, size: f64, x: f64, y: f64, align: Align) -> Option<Path> {
    let pick = pick(family, weight, false);
    let fallback = Face::from_slice(INTER_VAR, 0).expect("bundled font");
    let scale = size / pick.face.units_per_em() as f64;
    let fscale = size / fallback.units_per_em() as f64;
    let line = shape_line(text, &pick, &fallback, scale, fscale, 0.0);
    let x0 = match align {
        Align::Left => x,
        Align::Center => x - line.width / 2.0,
        Align::Right => x - line.width,
    };
    let mut pb = PathBuilder::new();
    for g in &line.glyphs {
        let (face, s) = if g.face == 0 { (&pick.face, scale) } else { (&fallback, fscale) };
        let mut o = Outline { pb: &mut pb, x: x0 + g.x, y, s, skew: 0.0 };
        face.outline_glyph(ttf_parser::GlyphId(g.id), &mut o);
    }
    pb.finish()
}
