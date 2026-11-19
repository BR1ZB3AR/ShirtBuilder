//! Garment pattern definitions — a line-by-line port of the web app's `pattern.ts`, so both
//! editions produce identical pieces (verified by `tests/parity.rs` against the web app).

use crate::geom::{clamp, cubic, lerp, mirror_x, poly_len, v2, V2};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PieceId {
    RibF,
    RibB,
    Collar,
    SleeveR,
    SleeveL,
    Front,
    Back,
}

impl PieceId {
    pub const ALL: [PieceId; 7] = [PieceId::RibF, PieceId::RibB, PieceId::Collar, PieceId::SleeveR, PieceId::SleeveL, PieceId::Front, PieceId::Back];
    pub fn as_str(self) -> &'static str {
        match self {
            PieceId::RibF => "ribF",
            PieceId::RibB => "ribB",
            PieceId::Collar => "collar",
            PieceId::SleeveR => "sleeveR",
            PieceId::SleeveL => "sleeveL",
            PieceId::Front => "front",
            PieceId::Back => "back",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    Tee,
    Vneck,
    Polo,
    Shirt,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SleeveLen {
    Short,
    Long,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fit {
    M,
    F,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Size {
    XS,
    S,
    M,
    L,
    XL,
    XXL,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollarKind {
    Rib,
    Polo,
    Shirt,
}

pub const STYLES: [(Style, &str, &str); 4] = [
    (Style::Tee, "Round Neck T-Shirt", "Crew neck with rib band"),
    (Style::Vneck, "V-Neck T-Shirt", "V neck with rib band"),
    (Style::Polo, "Polo Collar Shirt", "Knit collar, 3-button placket"),
    (Style::Shirt, "Collar Shirt", "Point collar, full button front"),
];
pub const SLEEVES: [(SleeveLen, &str); 2] = [(SleeveLen::Short, "Short sleeve"), (SleeveLen::Long, "Long sleeve")];
pub const FITS: [(Fit, &str); 2] = [(Fit::M, "Male"), (Fit::F, "Female")];
pub const SIZES: [Size; 6] = [Size::XS, Size::S, Size::M, Size::L, Size::XL, Size::XXL];

impl Style {
    fn key(self) -> &'static str {
        match self {
            Style::Tee => "tee",
            Style::Vneck => "vneck",
            Style::Polo => "polo",
            Style::Shirt => "shirt",
        }
    }
    pub fn name(self) -> &'static str {
        STYLES.iter().find(|s| s.0 == self).map(|s| s.1).unwrap_or("")
    }
}
impl SleeveLen {
    fn key(self) -> &'static str {
        match self {
            SleeveLen::Short => "short",
            SleeveLen::Long => "long",
        }
    }
}
impl Fit {
    fn key(self) -> &'static str {
        match self {
            Fit::M => "m",
            Fit::F => "f",
        }
    }
}
impl Size {
    pub fn key(self) -> &'static str {
        match self {
            Size::XS => "XS",
            Size::S => "S",
            Size::M => "M",
            Size::L => "L",
            Size::XL => "XL",
            Size::XXL => "XXL",
        }
    }
    fn index(self) -> i32 {
        SIZES.iter().position(|s| *s == self).unwrap() as i32
    }
}

/// e.g. `tee-short-m-M` (style-sleeve-fit-size), the same ids the web app stores in project files
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GarmentId {
    pub style: Style,
    pub sleeve: SleeveLen,
    pub fit: Fit,
    pub size: Size,
}

pub const DEFAULT_GARMENT: GarmentId = GarmentId { style: Style::Tee, sleeve: SleeveLen::Short, fit: Fit::M, size: Size::M };

impl fmt::Display for GarmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}-{}-{}", self.style.key(), self.sleeve.key(), self.fit.key(), self.size.key())
    }
}

impl GarmentId {
    pub fn all() -> Vec<GarmentId> {
        let mut out = Vec::with_capacity(96);
        for (style, _, _) in STYLES {
            for (sleeve, _) in SLEEVES {
                for (fit, _) in FITS {
                    for size in SIZES {
                        out.push(GarmentId { style, sleeve, fit, size });
                    }
                }
            }
        }
        out
    }

    fn parse_exact(s: &str) -> Option<GarmentId> {
        let p: Vec<&str> = s.split('-').collect();
        if p.len() != 4 {
            return None;
        }
        Some(GarmentId {
            style: STYLES.iter().map(|x| x.0).find(|x| x.key() == p[0])?,
            sleeve: SLEEVES.iter().map(|x| x.0).find(|x| x.key() == p[1])?,
            fit: FITS.iter().map(|x| x.0).find(|x| x.key() == p[2])?,
            size: SIZES.iter().copied().find(|x| x.key() == p[3])?,
        })
    }

    /// accepts current ids, older ids saved before sizes existed, and the original
    /// 'crew' / 'vneck' / 'longsleeve' names; anything else becomes the default garment
    pub fn normalize(raw: &str) -> GarmentId {
        if let Some(id) = Self::parse_exact(raw) {
            return id;
        }
        let legacy = match raw {
            "crew" => Some("tee-short-m-M"),
            "vneck" => Some("vneck-short-m-M"),
            "longsleeve" => Some("tee-long-m-M"),
            _ => None,
        };
        if let Some(id) = legacy.and_then(Self::parse_exact) {
            return id;
        }
        Self::parse_exact(&format!("{raw}-M")).unwrap_or(DEFAULT_GARMENT)
    }

    pub fn with_size(self, size: Size) -> GarmentId {
        GarmentId { size, ..self }
    }
}

impl Serialize for GarmentId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for GarmentId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Ok(GarmentId::normalize(&raw))
    }
}

#[derive(Clone, Debug)]
pub struct GarmentSpec {
    pub id: GarmentId,
    pub title: String,
    pub subtitle: String,
    pub body_w: f64,
    pub body_h: f64,
    pub neck_w: f64,
    pub front_neck: f64,
    pub back_neck: f64,
    pub shoulder_half: f64,
    pub shoulder_drop: f64,
    pub arm_depth: f64,
    pub sleeve_w: f64,
    pub sleeve_h: f64,
    pub sleeve_hem_w: f64,
    pub cap_h: f64,
    pub sleeve_angle: f64,
    pub vneck: bool,
    pub rib_h: f64,
    pub collar: CollarKind,
    pub collar_stand: f64,
    pub collar_fall: f64,
    pub collar_drop: f64,
    pub collar_spread: f64,
    pub collar_gap: f64,
    pub placket_w: f64,
    pub placket_l: f64,
    pub buttons: u32,
    pub hem_rise: f64,
    pub waist_in: f64,
    pub waist_y: f64,
    pub hem_out: f64,
    pub bust: f64,
    pub cuff_h: f64,
    pub sleeve_band: f64,
    pub pocket: bool,
    pub rib_ratio: f64,
    pub cap_ease: f64,
}

impl GarmentSpec {
    pub fn style(&self) -> Style {
        self.id.style
    }
    pub fn sleeve(&self) -> SleeveLen {
        self.id.sleeve
    }
    pub fn fit(&self) -> Fit {
        self.id.fit
    }
    pub fn size(&self) -> Size {
        self.id.size
    }

    pub fn new(id: GarmentId) -> GarmentSpec {
        let f = id.fit == Fit::F;
        let long = id.sleeve == SleeveLen::Long;
        let base = |m: f64, w: f64| if f { w } else { m };
        let mut s = GarmentSpec {
            id,
            title: id.style.name().to_string(),
            subtitle: format!(
                "{} · {} sleeve · {}",
                if f { "Women's" } else { "Men's" },
                if long { "Long" } else { "Short" },
                id.size.key()
            ),
            body_w: base(53.0, 47.0),
            body_h: base(73.0, 64.0),
            neck_w: base(8.5, 8.0),
            front_neck: base(9.5, 10.5),
            back_neck: base(2.5, 2.2),
            shoulder_half: base(21.5, 18.5),
            shoulder_drop: 4.0,
            arm_depth: base(22.0, 19.5),
            sleeve_w: base(42.0, 35.5),
            sleeve_h: base(23.5, 16.0),
            sleeve_hem_w: base(38.0, 32.0),
            cap_h: base(11.0, 10.5),
            sleeve_angle: base(40.0, 38.0),
            vneck: false,
            rib_h: 2.2,
            collar: CollarKind::Rib,
            collar_stand: 0.0,
            collar_fall: 0.0,
            collar_drop: 0.0,
            collar_spread: 0.0,
            collar_gap: 0.0,
            placket_w: 0.0,
            placket_l: 0.0,
            buttons: 0,
            hem_rise: 0.0,
            waist_in: base(0.0, 3.0),
            waist_y: base(0.55, 0.56),
            hem_out: base(0.0, 1.8),
            bust: base(0.0, 2.4),
            cuff_h: 0.0,
            sleeve_band: 0.0,
            pocket: false,
            rib_ratio: 0.88,
            cap_ease: 0.3,
        };
        match id.style {
            Style::Tee => {}
            Style::Vneck => {
                s.vneck = true;
                s.front_neck = if f { 15.0 } else { 16.0 };
            }
            Style::Polo => {
                s.collar = CollarKind::Polo;
                s.neck_w = if f { 7.8 } else { 8.6 };
                s.front_neck = if f { 7.5 } else { 8.0 };
                s.back_neck = 2.2;
                s.collar_stand = 2.6;
                s.collar_fall = 4.6;
                s.collar_drop = 0.6;
                s.collar_spread = 1.0;
                s.collar_gap = if f { 1.4 } else { 1.6 };
                s.placket_w = if f { 2.8 } else { 3.2 };
                s.placket_l = if f { 12.5 } else { 15.0 };
                s.buttons = 3;
                s.body_h += 2.0;
                s.sleeve_band = 2.5;
            }
            Style::Shirt => {
                s.collar = CollarKind::Shirt;
                s.neck_w = if f { 7.3 } else { 8.0 };
                s.front_neck = if f { 7.0 } else { 7.5 };
                s.back_neck = 2.0;
                s.collar_stand = 3.2;
                s.collar_fall = 5.0;
                s.collar_drop = 3.0;
                s.collar_spread = 2.8;
                s.collar_gap = 0.3;
                s.placket_w = if f { 3.0 } else { 3.4 };
                s.placket_l = 999.0;
                s.buttons = if f { 6 } else { 7 };
                s.hem_rise = if f { 6.0 } else { 7.0 };
                s.body_h += if f { 3.0 } else { 4.0 };
                s.sleeve_h = if f { 18.0 } else { 24.0 };
                s.sleeve_hem_w = if f { 34.0 } else { 40.0 };
                s.pocket = !f;
                s.cap_ease = 1.5; // woven: the cap is eased into the armhole
            }
        }
        if long {
            s.sleeve_h = if f { 57.0 } else { 62.0 };
            s.sleeve_hem_w = if id.style == Style::Shirt {
                if f { 21.0 } else { 25.0 }
            } else if f {
                19.5
            } else {
                22.0
            };
            s.cap_h = if f { 11.5 } else { 12.5 };
            s.sleeve_angle = 64.0;
            s.cuff_h = if id.style == Style::Shirt { if f { 5.0 } else { 6.0 } } else { 0.0 };
            s.sleeve_band = match id.style {
                Style::Polo => 5.0,
                Style::Shirt => 0.0,
                _ => 6.0, // knit rib cuff on long-sleeve tees
            };
        }
        // grade from the size-M base (women's grades a little smaller)
        let k = (id.size.index() - Size::M.index()) as f64;
        let gk = k * if f { 0.85 } else { 1.0 };
        s.body_w += 2.5 * gk;
        s.body_h += 2.0 * gk;
        s.shoulder_half += 0.8 * gk;
        s.neck_w += 0.25 * gk;
        s.front_neck += 0.3 * gk;
        s.back_neck += 0.1 * gk;
        s.arm_depth += 0.8 * gk;
        s.sleeve_w += 1.5 * gk;
        s.sleeve_hem_w += if long { 0.6 } else { 1.2 } * gk;
        s.sleeve_h += if long { 1.0 } else { 0.6 } * gk;
        s
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    Body,
    Sleeve,
    Rib,
    Collar,
}

impl PieceKind {
    /// narrow trim pieces (neck rib, collar): labelled above, no grain line
    pub fn is_trim(self) -> bool {
        matches!(self, PieceKind::Rib | PieceKind::Collar)
    }
}

#[derive(Clone, Debug)]
pub struct Piece {
    pub id: PieceId,
    pub name: String,
    pub kind: PieceKind,
    pub w: f64,
    pub h: f64,
    pub outline: Vec<V2>,
    /// layout position on the pattern sheet
    pub x: f64,
    pub y: f64,
    pub cut: u32,
}

#[derive(Clone, Debug)]
pub struct Garment {
    pub spec: GarmentSpec,
    pub pieces: Vec<Piece>,
    /// sheet size
    pub w: f64,
    pub h: f64,
    /// front-piece local, left neck point → right neck point
    pub front_neck: Vec<V2>,
    pub back_neck: Vec<V2>,
    /// body-piece local, right-hand armhole, shoulder point → underarm
    pub armhole: Vec<V2>,
    /// button centres in front-piece coords
    pub buttons: Vec<V2>,
}

fn cap_shape(sx: f64, sw: f64, h: f64) -> f64 {
    let t = clamp(sx / sw, 0.0, 1.0);
    let b = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * t).cos();
    h * (1.0 - b.powf(0.7))
}

fn ease(t: f64) -> f64 {
    0.5 - 0.5 * (std::f64::consts::PI * clamp(t, 0.0, 1.0)).cos()
}

impl Garment {
    pub fn piece(&self, id: PieceId) -> Option<&Piece> {
        self.pieces.iter().find(|p| p.id == id)
    }
    pub fn has_piece(&self, id: PieceId) -> bool {
        self.piece(id).is_some()
    }
    /// pieces that always exist
    pub fn front(&self) -> &Piece {
        self.piece(PieceId::Front).expect("front")
    }
    pub fn back(&self) -> &Piece {
        self.piece(PieceId::Back).expect("back")
    }

    /// sleeve cap height at sleeve x
    pub fn cap_y(&self, sx: f64) -> f64 {
        cap_shape(sx, self.spec.sleeve_w, self.spec.cap_h)
    }

    pub fn sleeve_half_w(&self, sy: f64) -> f64 {
        let s = &self.spec;
        let ch = s.cap_h;
        if sy <= ch {
            s.sleeve_w / 2.0
        } else {
            lerp(s.sleeve_w / 2.0, s.sleeve_hem_w / 2.0, (sy - ch) / (s.sleeve_h - ch))
        }
    }

    /// side-seam inset from the full panel width at body row py
    pub fn inset(&self, py: f64) -> f64 {
        let s = &self.spec;
        let ay = s.shoulder_drop + s.arm_depth;
        let h = s.body_h;
        let wy = (ay + 4.0).max(s.waist_y * h);
        if s.waist_in <= 0.0 || py <= ay {
            return 0.0;
        }
        if py <= wy {
            return s.waist_in * ease((py - ay) / (wy - ay));
        }
        s.waist_in - s.hem_out * ease((py - wy) / (h - wy))
    }

    /// hem line y at body x
    pub fn hem_at(&self, px: f64) -> f64 {
        let s = &self.spec;
        let h = s.body_h;
        if s.hem_rise <= 0.0 {
            return h;
        }
        let xs = self.inset(h - s.hem_rise);
        let t = clamp((px - xs) / (s.body_w - 2.0 * xs), 0.0, 1.0);
        h - s.hem_rise * (2.0 * t - 1.0).abs().powf(2.4)
    }

    pub fn build(id: GarmentId) -> Garment {
        let mut s = GarmentSpec::new(id);
        let w = s.body_w;
        let h = s.body_h;
        let cx = w / 2.0;
        let ay = s.shoulder_drop + s.arm_depth;
        let pi = std::f64::consts::PI;

        let neck = |depth: f64, v: bool, neck_w: f64| -> Vec<V2> {
            let n = 40;
            (0..=n)
                .map(|i| {
                    let t = i as f64 / n as f64;
                    if v {
                        let k = 1.0 - (2.0 * t - 1.0).abs();
                        v2(cx - neck_w + 2.0 * neck_w * t, depth * k.powf(1.3))
                    } else {
                        let a = pi * t;
                        v2(cx - neck_w * a.cos(), depth * a.sin().powf(0.85))
                    }
                })
                .collect()
        };

        let sp = v2(cx + s.shoulder_half, s.shoulder_drop);
        let armhole = cubic(sp, v2(sp.x - 1.3, sp.y + s.arm_depth * 0.5), v2(sp.x + (w - sp.x) * 0.15, ay), v2(w, ay), 28);

        // a temporary garment gives access to inset()/hem_at() while building
        let mut g = Garment {
            spec: s.clone(),
            pieces: vec![],
            w: 0.0,
            h: 0.0,
            front_neck: vec![],
            back_neck: vec![],
            armhole: armhole.clone(),
            buttons: vec![],
        };
        let side_end = h - s.hem_rise;
        let xs = g.inset(side_end);
        let side: Vec<V2> = (1..=24)
            .map(|i| {
                let py = lerp(ay, side_end, i as f64 / 24.0);
                v2(w - g.inset(py), py)
            })
            .collect();
        let hem: Vec<V2> = (1..40)
            .map(|i| {
                let x = lerp(w - xs, xs, i as f64 / 40.0);
                v2(x, g.hem_at(x))
            })
            .collect();

        let body = |nk: &[V2]| -> Vec<V2> {
            let mut out = nk.to_vec();
            out.extend_from_slice(&armhole);
            out.extend_from_slice(&side);
            out.extend_from_slice(&hem);
            out.extend(mirror_x(&side, w).into_iter().rev());
            out.extend(mirror_x(&armhole, w).into_iter().rev());
            out
        };

        let front_neck = neck(s.front_neck, s.vneck, s.neck_w);
        let back_neck = neck(s.back_neck, false, s.neck_w);

        let sw = s.sleeve_w;
        let sh = s.sleeve_h;
        let taper = (sw - s.sleeve_hem_w) / 2.0;
        let cap_len = |hh: f64| -> f64 {
            let mut l = 0.0;
            let mut px = 0.0;
            let mut py = cap_shape(0.0, sw, hh);
            for i in 1..=160 {
                let x = (sw * i as f64) / 160.0;
                let y = cap_shape(x, sw, hh);
                l += (x - px).hypot(y - py);
                px = x;
                py = y;
            }
            l
        };
        // the sleeve cap is sewn into the whole armhole (front + back): solve its height so the seam
        // lengths match, plus a little ease for woven fabric
        let cap_target = 2.0 * poly_len(&armhole) + s.cap_ease;
        let mut lo = 2.0;
        let mut hi = (sh - 4.0).min(26.0);
        for _ in 0..40 {
            let m = (lo + hi) / 2.0;
            if cap_len(m) > cap_target {
                hi = m;
            } else {
                lo = m;
            }
        }
        let ch = (lo + hi) / 2.0;
        s.cap_h = ch;
        let mut sleeve_outline: Vec<V2> = (0..=56)
            .map(|i| {
                let x = (sw * i as f64) / 56.0;
                v2(x, cap_shape(x, sw, ch))
            })
            .collect();
        sleeve_outline.push(v2(sw - taper, sh));
        sleeve_outline.push(v2(taper, sh));

        let rect = |rw: f64, rh: f64| vec![v2(0.0, 0.0), v2(rw, 0.0), v2(rw, rh), v2(0.0, rh)];

        let gap = 9.0;
        let sheet_w = 2.0 * w + gap;
        let back_x = w + gap;
        let mut trims: Vec<Piece> = vec![];
        let top_h;
        if s.collar == CollarKind::Rib {
            // the rib is cut shorter than the neckline (stretched on so it lies flat) and double width
            // (folded along the middle: top half sits inside, bottom half is the visible outside)
            let rib_fl = poly_len(&front_neck) * s.rib_ratio;
            let rib_bl = poly_len(&back_neck) * s.rib_ratio;
            let rh = 2.0 * s.rib_h;
            top_h = rh;
            trims.push(Piece { id: PieceId::RibF, name: "Neck rib · front".into(), kind: PieceKind::Rib, w: rib_fl, h: rh, outline: rect(rib_fl, rh), x: (w - rib_fl) / 2.0, y: 0.0, cut: 1 });
            trims.push(Piece { id: PieceId::RibB, name: "Neck rib · back".into(), kind: PieceKind::Rib, w: rib_bl, h: rh, outline: rect(rib_bl, rh), x: back_x + (w - rib_bl) / 2.0, y: 0.0, cut: 1 });
        } else {
            // one-piece collar: neck seam along y = 0, stand up to the fold, fall below it
            let l = poly_len(&front_neck) + poly_len(&back_neck) - 2.0 * s.collar_gap;
            let st = s.collar_stand;
            let hc = st + s.collar_fall;
            let dr = s.collar_drop;
            let spr = s.collar_spread;
            let mut outline = vec![v2(0.0, 0.0), v2(l, 0.0), v2(l, st)];
            for i in 0..=40 {
                let x = lerp(l - spr, spr, i as f64 / 40.0);
                let t = (x - spr) / (l - 2.0 * spr);
                outline.push(v2(x, hc + dr * (2.0 * t - 1.0).abs().powi(3)));
            }
            outline.push(v2(0.0, st));
            top_h = hc + dr;
            trims.push(Piece {
                id: PieceId::Collar,
                name: if s.collar == CollarKind::Polo { "Knit collar" } else { "Collar" }.into(),
                kind: PieceKind::Collar,
                w: l,
                h: hc + dr,
                outline,
                x: (sheet_w - l) / 2.0,
                y: 0.0,
                cut: if s.collar == CollarKind::Polo { 1 } else { 2 },
            });
        }
        let sleeve_y = top_h + 8.0;
        let body_y = sleeve_y + sh + BODY_ROW_GAP;

        let mut pieces = trims;
        pieces.push(Piece { id: PieceId::SleeveR, name: "Right sleeve".into(), kind: PieceKind::Sleeve, w: sw, h: sh, outline: sleeve_outline.clone(), x: (w - sw) / 2.0, y: sleeve_y, cut: 1 });
        pieces.push(Piece { id: PieceId::SleeveL, name: "Left sleeve".into(), kind: PieceKind::Sleeve, w: sw, h: sh, outline: sleeve_outline, x: back_x + (w - sw) / 2.0, y: sleeve_y, cut: 1 });
        pieces.push(Piece {
            id: PieceId::Front,
            name: if s.placket_l > 0.0 { "Front · placket" } else { "Front" }.into(),
            kind: PieceKind::Body,
            w,
            h,
            outline: body(&front_neck),
            x: 0.0,
            y: body_y,
            cut: 1,
        });
        pieces.push(Piece { id: PieceId::Back, name: "Back".into(), kind: PieceKind::Body, w, h, outline: body(&back_neck), x: back_x, y: body_y, cut: 1 });

        g.spec = s;
        let s = &g.spec;
        let mut buttons = vec![];
        if s.buttons > 0 {
            let y0 = s.front_neck + if s.collar == CollarKind::Shirt { 1.6 } else { 2.0 };
            let y1 = if s.placket_l >= h { g.hem_at(cx) - 13.0 } else { s.front_neck + s.placket_l - 2.2 };
            for i in 0..s.buttons {
                let t = if s.buttons > 1 { i as f64 / (s.buttons - 1) as f64 } else { 0.0 };
                buttons.push(v2(cx, lerp(y0, y1, t)));
            }
        }
        g.pieces = pieces;
        g.w = sheet_w;
        g.h = body_y + h;
        g.front_neck = front_neck;
        g.back_neck = back_neck;
        g.buttons = buttons;
        g
    }
}

/// cached garment geometry (building one takes well under a millisecond, but views ask often)
pub fn garment(id: GarmentId) -> Arc<Garment> {
    static CACHE: OnceLock<Mutex<HashMap<GarmentId, Arc<Garment>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut map = cache.lock().unwrap();
    map.entry(id).or_insert_with(|| Arc::new(Garment::build(id))).clone()
}

pub const ATLAS_PAD: f64 = 2.0;

/// gap (cm) between the sleeve row and the front / back row of the pattern sheet: room for the
/// sleeves' name and measurement labels clear of the body pieces' shoulders (the web app uses 11)
pub const BODY_ROW_GAP: f64 = 16.0;

/// texture atlas rectangle: the pattern sheet plus padding
pub fn atlas_rect(g: &Garment) -> (f64, f64, f64, f64) {
    (-ATLAS_PAD, -ATLAS_PAD, g.w + 2.0 * ATLAS_PAD, g.h + 2.0 * ATLAS_PAD)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Mm,
    #[default]
    Cm,
    In,
}

impl Unit {
    pub fn cm(self) -> f64 {
        match self {
            Unit::Mm => 0.1,
            Unit::Cm => 1.0,
            Unit::In => 2.54,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::In => "in",
        }
    }
}

/// JavaScript `Math.round` (rounds .5 towards +∞)
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// format a length in centimetres for display in `unit`, like the web app
pub fn fmt_len(cm: f64, unit: Unit, digits: Option<i32>) -> String {
    let v = cm / unit.cm();
    let d = digits.unwrap_or(match unit {
        Unit::Mm => 0,
        Unit::In => 1,
        Unit::Cm => {
            let r = js_round(v * 10.0) / 10.0;
            if r.fract() == 0.0 {
                0
            } else {
                1
            }
        }
    });
    let k = 10f64.powi(d);
    let r = js_round(v * k) / k;
    // JS number → string: integers without a decimal point, otherwise shortest form
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}
