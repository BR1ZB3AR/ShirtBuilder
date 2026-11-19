//! Pattern sheet exports (the web app's `actions.ts`): a PNG print sheet, a true-scale SVG print &
//! cut sheet in named layers (Inkscape / Illustrator / cutter software pick them up), and an SVG
//! of the cut lines only. 1 SVG user unit = 1 cm.

use crate::draw::{draw_piece_content, poly_path, Mode};
use crate::images::ImageCache;
use crate::text;
use base64::Engine as _;
use resvg::tiny_skia::{Color, FillRule, Paint, Pixmap, Rect, Stroke, StrokeDash, Transform};
use sb_core::cutting::{cut_outline, notch_segments, notches, Notch, SEAM};
use sb_core::doc::{Align, Doc, Side};
use sb_core::geom::V2;
use sb_core::pattern::{fmt_len, garment, Garment, Piece, Unit};
use sb_core::seams::seams;

struct SheetPiece<'a> {
    p: &'a Piece,
    /// piece coords, outside orientation
    cut: Vec<V2>,
    notches: Vec<Notch>,
    sa: bool,
    /// sheet coords
    label_y: f64,
}

struct Layout<'a> {
    pieces: Vec<SheetPiece<'a>>,
    x0: f64,
    y0: f64,
    w: f64,
    h: f64,
    scale_y: f64,
}

fn sheet_layout(g: &Garment, sa: bool) -> Layout<'_> {
    let pieces: Vec<SheetPiece> = g
        .pieces
        .iter()
        .map(|p| {
            let cut = if sa { cut_outline(p, g) } else { p.outline.clone() };
            let ys = cut.iter().map(|q| q.y);
            let label_y = if p.kind.is_trim() { p.y + ys.fold(f64::INFINITY, f64::min) - 0.9 } else { p.y + ys.fold(f64::NEG_INFINITY, f64::max) + 1.7 };
            SheetPiece { p, cut, notches: notches(p, g), sa, label_y }
        })
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for sp in &pieces {
        for q in &sp.cut {
            x0 = x0.min(q.x + sp.p.x);
            x1 = x1.max(q.x + sp.p.x);
            y0 = y0.min(q.y + sp.p.y);
            y1 = y1.max(q.y + sp.p.y);
        }
        y0 = y0.min(sp.label_y - 1.4);
        y1 = y1.max(sp.label_y + 1.6);
    }
    let pad = 2.0;
    let scale_y = y1 + 1.5;
    Layout { pieces, x0: x0 - pad, y0: y0 - pad, w: x1 - x0 + 2.0 * pad, h: scale_y + 1.5 - y0 + 2.0 * pad, scale_y }
}

fn mirror_pts(pts: &[V2], w: f64) -> Vec<V2> {
    pts.iter().map(|q| V2 { x: w - q.x, y: q.y }).collect()
}
fn mirror_notch(n: &Notch, w: f64) -> Notch {
    Notch { p: V2 { x: w - n.p.x, y: n.p.y }, n: V2 { x: -n.n.x, y: n.n.y }, t: V2 { x: -n.t.x, y: n.t.y }, count: n.count }
}

fn piece_label(p: &Piece, unit: Unit, sa: bool) -> String {
    format!("{} — {} × {} {} finished · cut {}{}", p.name, fmt_len(p.w, unit, None), fmt_len(p.h, unit, None), unit.label(), p.cut, if sa { " · incl. allowances" } else { "" })
}

const SA_NOTE: &str = "Solid = cut line (seam allowance 1 cm, hems 2.5 cm body / 2 cm sleeves) · dashed = stitch line · ▸ notches";
const NET_NOTE: &str = "Net stitch lines — no seam allowance added";

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

/// PNG print sheet of one side, with artwork, cut lines, notches and labels (`dpi` 100 by default)
pub fn pattern_png(doc: &Doc, side: Side, unit: Unit, sa: bool, dpi: f64, imgs: &ImageCache) -> Result<Vec<u8>, String> {
    let g = garment(doc.garment);
    let l = sheet_layout(&g, sa);
    let ppc = dpi / 2.54;
    let (w, h) = ((l.w * ppc).round() as u32, (l.h * ppc).round() as u32);
    let mut pm = Pixmap::new(w.max(1), h.max(1)).ok_or("sheet too large")?;
    pm.fill(Color::WHITE);
    let base = Transform::from_row(ppc as f32, 0.0, 0.0, ppc as f32, (-l.x0 * ppc) as f32, (-l.y0 * ppc) as f32);
    let inside = side == Side::Inside;
    let all = seams(&g, side);
    let black = paint(Color::BLACK);
    for sp in &l.pieces {
        let p = sp.p;
        let mut ts = base.pre_translate(p.x as f32, p.y as f32);
        if inside {
            ts = ts.pre_translate(p.w as f32, 0.0).pre_scale(-1.0, 1.0);
        }
        draw_piece_content(&mut pm, ts, p, &g, doc, &all, side, Mode::Export, if sp.sa { Some(&sp.cut) } else { None }, imgs);
        let ts = base.pre_translate(p.x as f32, p.y as f32);
        let cut = if inside { mirror_pts(&sp.cut, p.w) } else { sp.cut.clone() };
        let net = if inside { mirror_pts(&p.outline, p.w) } else { p.outline.clone() };
        if let Some(path) = poly_path(&cut, true) {
            pm.stroke_path(&path, &black, &Stroke { width: 0.05, ..Default::default() }, ts, None);
        }
        if sp.sa {
            if let Some(path) = poly_path(&net, true) {
                let s = Stroke { width: 0.03, dash: StrokeDash::new(vec![0.4, 0.25], 0.0), ..Default::default() };
                pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 140)), &s, ts, None);
            }
        }
        for n0 in &sp.notches {
            let n = if inside { mirror_notch(n0, p.w) } else { *n0 };
            for (a, b) in notch_segments(&n, if sp.sa { SEAM } else { 0.0 }) {
                if let Some(path) = poly_path(&[a, b], false) {
                    pm.stroke_path(&path, &black, &Stroke { width: 0.06, ..Default::default() }, ts, None);
                }
            }
        }
        if let Some(t) = text::label_path(&piece_label(p, unit, sp.sa), "Inter", 600.0, 1.1, p.x + p.w / 2.0, sp.label_y, Align::Center) {
            pm.fill_path(&t, &paint(Color::from_rgba8(0x22, 0x22, 0x22, 255)), FillRule::Winding, base, None);
        }
    }
    if let Some(r) = Rect::from_xywh(0.0, l.scale_y as f32, 10.0, 0.25) {
        pm.fill_rect(r, &black, base, None);
    }
    let note = format!("10 cm   ·   {} — {}   ·   {}", g.spec.title, g.spec.subtitle, if sa { SA_NOTE } else { NET_NOTE });
    if let Some(t) = text::label_path(&note, "Inter", 400.0, 1.0, 10.5, l.scale_y + 0.45, Align::Left) {
        pm.fill_path(&t, &black, FillRule::Winding, base, None);
    }
    png(pm)
}

/// encode a premultiplied pixmap as a PNG file
pub fn png(pm: Pixmap) -> Result<Vec<u8>, String> {
    pm.encode_png().map_err(|e| format!("PNG: {e}"))
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
/// JS `+n.toFixed(3)` as text
fn f3(n: f64) -> String {
    let r = (n * 1000.0).round() / 1000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}
fn pts_attr(pts: &[V2], ox: f64, oy: f64) -> String {
    pts.iter().map(|q| format!("{},{}", f3(q.x + ox), f3(q.y + oy))).collect::<Vec<_>>().join(" ")
}

/// vector cut line, stitch line, notches, grain line and label for one piece (sheet coords)
fn svg_piece_lines(sp: &SheetPiece, inside: bool, unit: Unit) -> (String, Vec<String>) {
    let p = sp.p;
    let cut = if inside { mirror_pts(&sp.cut, p.w) } else { sp.cut.clone() };
    let net = if inside { mirror_pts(&p.outline, p.w) } else { p.outline.clone() };
    let cut_el = format!(r##"<polygon id="cut-{}" points="{}" fill="none" stroke="#000" stroke-width="0.05"/>"##, p.id.as_str(), pts_attr(&cut, p.x, p.y));
    let mut marks = Vec::new();
    if sp.sa {
        marks.push(format!(r##"<polygon points="{}" fill="none" stroke="#666" stroke-width="0.03" stroke-dasharray="0.4 0.25"/>"##, pts_attr(&net, p.x, p.y)));
    }
    for n0 in &sp.notches {
        let n = if inside { mirror_notch(n0, p.w) } else { *n0 };
        for (a, b) in notch_segments(&n, if sp.sa { SEAM } else { 0.0 }) {
            marks.push(format!(r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#000" stroke-width="0.06"/>"##, f3(a.x + p.x), f3(a.y + p.y), f3(b.x + p.x), f3(b.y + p.y)));
        }
    }
    if !p.kind.is_trim() {
        marks.push(format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#888" stroke-width="0.05" stroke-dasharray="0.6 0.4"/>"##,
            f3(p.x + p.w / 2.0),
            f3(p.y + 3.0),
            f3(p.x + p.w / 2.0),
            f3(p.y + p.h - 4.0)
        ));
    }
    if p.kind == sb_core::pattern::PieceKind::Rib {
        let fy = p.y + p.h / 2.0;
        marks.push(format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#888" stroke-width="0.04" stroke-dasharray="0.3 0.2"/><text x="{}" y="{}" font-size="0.7" font-family="Inter, Arial, sans-serif">FOLD</text>"##,
            f3(p.x),
            f3(fy),
            f3(p.x + p.w),
            f3(fy),
            f3(p.x + 0.4),
            f3(fy - 0.25)
        ));
    }
    marks.push(format!(r##"<text x="{}" y="{}" font-size="1.1" text-anchor="middle" font-family="Inter, Arial, sans-serif">{}</text>"##, f3(p.x + p.w / 2.0), f3(sp.label_y), esc(&piece_label(p, unit, sp.sa))));
    (cut_el, marks)
}

fn svg_doc(l: &Layout, title: &str, layers: &[String], note: &str) -> String {
    format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="{w}cm" height="{h}cm" viewBox="{x0} {y0} {w} {h}">
<title>{title}</title>
{layers}
<g id="scale" inkscape:groupmode="layer" inkscape:label="Scale"><rect x="0" y="{sy}" width="10" height="0.25"/><text x="10.5" y="{ty}" font-size="1" font-family="Inter, Arial, sans-serif">10 cm · {note}</text></g>
</svg>"##,
        w = f3(l.w),
        h = f3(l.h),
        x0 = f3(l.x0),
        y0 = f3(l.y0),
        title = esc(title),
        layers = layers.join("\n"),
        sy = f3(l.scale_y),
        ty = f3(l.scale_y + 0.45),
        note = esc(note)
    )
}
fn layer(id: &str, label: &str, body: &[String]) -> String {
    format!("<g id=\"{id}\" inkscape:groupmode=\"layer\" inkscape:label=\"{label}\">\n{}\n</g>", body.join("\n"))
}

/// SVG of the cut lines, notches, grain lines and labels only
pub fn cutlines_svg(doc: &Doc, unit: Unit, sa: bool) -> String {
    let g = garment(doc.garment);
    let l = sheet_layout(&g, sa);
    let (mut cuts, mut marks) = (Vec::new(), Vec::new());
    for sp in &l.pieces {
        let (c, m) = svg_piece_lines(sp, false, unit);
        cuts.push(c);
        marks.extend(m);
    }
    let title = format!("ShirtBuilder — {} ({}) — cut lines", g.spec.title, g.spec.subtitle);
    svg_doc(&l, &title, &[layer("cut-lines", "Cut lines", &cuts), layer("marks", "Marks", &marks)], if sa { SA_NOTE } else { NET_NOTE })
}

/// True-scale print & cut sheet: each piece's fabric, details and artwork rasterised at `dpi`
/// (bleeding out to the cut line, clipped to it), plus vector cut lines and marks.
pub fn print_svg(doc: &Doc, side: Side, unit: Unit, sa: bool, dpi: f64, imgs: &ImageCache) -> Result<String, String> {
    let g = garment(doc.garment);
    let l = sheet_layout(&g, sa);
    let ppc = dpi / 2.54;
    let inside = side == Side::Inside;
    let all = seams(&g, side);
    let (mut defs, mut prints, mut cuts, mut marks) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for sp in &l.pieces {
        let p = sp.p;
        let cut_l = if inside { mirror_pts(&sp.cut, p.w) } else { sp.cut.clone() };
        let bx = cut_l.iter().map(|q| q.x).fold(f64::INFINITY, f64::min);
        let by = cut_l.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
        let bw = cut_l.iter().map(|q| q.x).fold(f64::NEG_INFINITY, f64::max) - bx;
        let bh = cut_l.iter().map(|q| q.y).fold(f64::NEG_INFINITY, f64::max) - by;
        let (cw, ch) = ((bw * ppc).ceil().max(1.0) as u32, (bh * ppc).ceil().max(1.0) as u32);
        let mut pm = Pixmap::new(cw, ch).ok_or("piece image too large")?;
        let mut ts = Transform::from_row(ppc as f32, 0.0, 0.0, ppc as f32, (-bx * ppc) as f32, (-by * ppc) as f32);
        if inside {
            ts = ts.pre_translate(p.w as f32, 0.0).pre_scale(-1.0, 1.0);
        }
        draw_piece_content(&mut pm, ts, p, &g, doc, &all, side, Mode::Export, if sp.sa { Some(&sp.cut) } else { None }, imgs);
        let href = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png(pm)?));
        let id = p.id.as_str();
        defs.push(format!(r#"<clipPath id="clip-{id}"><polygon points="{}"/></clipPath>"#, pts_attr(&cut_l, p.x, p.y)));
        prints.push(format!(
            r#"<image id="print-{id}" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" href="{href}" xlink:href="{href}" clip-path="url(#clip-{id})"/>"#,
            f3(p.x + bx),
            f3(p.y + by),
            f3(cw as f64 / ppc),
            f3(ch as f64 / ppc)
        ));
        let (c, m) = svg_piece_lines(sp, inside, unit);
        cuts.push(c);
        marks.extend(m);
    }
    let side_name = if inside { "inside" } else { "outside" };
    let title = format!("ShirtBuilder — {} ({}) — {side_name}", g.spec.title, g.spec.subtitle);
    Ok(svg_doc(
        &l,
        &title,
        &[format!("<defs>{}</defs>", defs.join("")), layer("print", "Print", &prints), layer("cut-lines", "Cut lines", &cuts), layer("marks", "Marks", &marks)],
        if sa { SA_NOTE } else { NET_NOTE },
    ))
}
