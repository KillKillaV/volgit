//! Draws the avatar in the terminal. Two modes:
//! - Blocks: half blocks (▀), each cell paints two pixels. Works in any
//!   true-color terminal, but looks pixelated.
//! - Kitty: sends the real image (PNG) through the kitty graphics protocol,
//!   which Ghostty and WezTerm also understand.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::imageops::FilterType;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Real image through the kitty graphics protocol.
    Kitty,
    /// Colored half blocks (any terminal).
    Blocks,
}

/// Rounded corner radius, relative to the image side.
const CORNER: f32 = 0.22;

/// How much wider than tall the avatar is in block mode (percent).
const WIDEN_PCT: usize = 8;

pub struct Avatar {
    /// One line per row of cells, ANSI codes included.
    pub lines: Vec<String>,
    /// Visible width in columns (the same for every line).
    pub width: usize,
    /// Representative color of the image, used as the accent.
    pub accent: Rgb,
}

impl Avatar {
    pub fn from_bytes(bytes: &[u8], cols: usize, mode: Mode) -> Option<Self> {
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        match mode {
            Mode::Blocks => Some(Self::blocks(&img, cols)),
            Mode::Kitty => Self::kitty(&img, cols),
        }
    }

    /// With `cols` = 28 it takes 14 rows (those of a square) but 30 columns:
    /// slightly wider than tall. To avoid distorting the face, the image is
    /// center-cropped to the real proportions of the area instead of stretched.
    fn blocks(img: &RgbaImage, cols: usize) -> Self {
        let rows = (cols / 2).max(1);
        let width = cols + cols * WIDEN_PCT / 100;
        let (w, h) = (width as u32, rows as u32 * 2); // 2 pixels per cell vertically

        // Physical proportions of the area (width/height) given the cell shape.
        let target = width as f32 * crate::term::cell_aspect() / rows as f32;
        let img = crop_to_aspect(img, target);
        // Lanczos3 keeps the most detail when downscaling this much.
        let mut img = image::imageops::resize(&img, w, h, FilterType::Lanczos3);
        round_corners(&mut img, w.min(h) as f32 * CORNER);

        let mut lines = Vec::with_capacity(rows);
        for y in (0..h).step_by(2) {
            let mut line = String::new();
            for x in 0..w {
                let top = img.get_pixel(x, y);
                let bottom = (y + 1 < h).then(|| img.get_pixel(x, y + 1));
                line += &cell(top, bottom);
            }
            line += "\x1b[0m";
            lines.push(line);
        }
        Self {
            lines,
            width,
            accent: accent(&img),
        }
    }

    /// The image takes `cols` columns and as many rows as needed to look square.
    /// It is placed on the first line; the other lines are spaces that reserve
    /// its area so the text next to it doesn't overlap.
    fn kitty(img: &RgbaImage, cols: usize) -> Option<Self> {
        let rows = ((cols as f32 * crate::term::cell_aspect()).round() as usize).max(1);

        let side = img.width().min(img.height()).min(512);
        let mut img = image::imageops::resize(img, side, side, FilterType::Lanczos3);
        round_corners(&mut img, side as f32 * CORNER);
        let accent = accent(&img);

        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .ok()?;

        let blank = " ".repeat(cols);
        let mut lines = vec![blank.clone(); rows];
        lines[0] = format!("{}{blank}", kitty_escape(&png, cols, rows));
        Some(Self {
            lines,
            width: cols,
            accent,
        })
    }
}

/// Center-crops the image to the `aspect` ratio (width / height), trimming the
/// sides or the top and bottom as needed.
fn crop_to_aspect(img: &RgbaImage, aspect: f32) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let (cw, ch) = if w as f32 / h as f32 > aspect {
        (((h as f32 * aspect).round() as u32).clamp(1, w), h)
    } else {
        (w, ((w as f32 / aspect).round() as u32).clamp(1, h))
    };
    image::imageops::crop_imm(img, (w - cw) / 2, (h - ch) / 2, cw, ch).to_image()
}

/// kitty graphics protocol sequence that displays a PNG:
/// `ESC _G <keys> ; <base64> ESC \`. Keys used:
/// - a=T: transmit and display at once
/// - f=100: the data is a PNG
/// - c, r: size in columns and rows (kitty scales the image to fit)
/// - C=1: don't move the cursor, so the text next to it can be written
/// - q=2: no reply from the terminal (its "OK" would show up in the shell)
/// - m=1/0: more chunks follow / last chunk
///
/// The base64 is split into 4096-byte chunks as the protocol requires; only
/// the first one carries all the keys.
fn kitty_escape(png: &[u8], cols: usize, rows: usize) -> String {
    let data = BASE64.encode(png);
    let chunks: Vec<&[u8]> = data.as_bytes().chunks(4096).collect();
    let mut out = String::with_capacity(data.len() + chunks.len() * 24);
    for (i, chunk) in chunks.iter().enumerate() {
        let more = u8::from(i + 1 < chunks.len());
        if i == 0 {
            out += &format!("\x1b_Ga=T,f=100,c={cols},r={rows},C=1,q=2,m={more};");
        } else {
            out += &format!("\x1b_Gm={more},q=2;");
        }
        // Base64 is always ASCII, so every chunk is valid UTF-8.
        out += std::str::from_utf8(chunk).unwrap_or_default();
        out += "\x1b\\";
    }
    out
}

fn visible(p: &Rgba<u8>) -> bool {
    p[3] > 127
}

fn cell(top: &Rgba<u8>, bottom: Option<&Rgba<u8>>) -> String {
    let bottom = bottom.filter(|p| visible(p));
    match (visible(top), bottom) {
        (true, Some(b)) => format!(
            "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
            top[0], top[1], top[2], b[0], b[1], b[2]
        ),
        (true, None) => format!("\x1b[49m\x1b[38;2;{};{};{}m▀", top[0], top[1], top[2]),
        (false, Some(b)) => format!("\x1b[49m\x1b[38;2;{};{};{}m▄", b[0], b[1], b[2]),
        (false, None) => "\x1b[0m ".to_string(),
    }
}

/// Makes the corners transparent to get a rounded square.
/// The edge is antialiased: pixels cut in half by the arc become
/// semi-transparent instead of all-or-nothing.
fn round_corners(img: &mut RgbaImage, r: f32) {
    let (w, h) = (img.width() as f32, img.height() as f32);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let cx = px.clamp(r, w - r);
        let cy = py.clamp(r, h - r);
        let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
        // dist <= r - 0.5 → opaque; dist >= r + 0.5 → transparent; in between, a gradient.
        let coverage = (r + 0.5 - dist).clamp(0.0, 1.0);
        p[3] = (p[3] as f32 * coverage) as u8;
    }
}

/// Saturation-weighted average of the pixels, normalized to a hue that reads
/// well on a dark background. Nearly gray images get a neutral blue.
fn accent(img: &RgbaImage) -> Rgb {
    let (mut sum, mut weight) = ([0f32; 3], 0f32);
    for p in img.pixels().filter(|p| visible(p)) {
        let (_, s, l) = to_hsl(p[0], p[1], p[2]);
        // Ignore near-black and near-white pixels: they carry no color.
        let w = s * (1.0 - (2.0 * l - 1.0).abs());
        for i in 0..3 {
            sum[i] += p[i] as f32 * w;
        }
        weight += w;
    }
    if weight < 1.0 {
        return (122, 162, 247);
    }
    let avg = sum.map(|c| (c / weight) as u8);
    let (h, s, _) = to_hsl(avg[0], avg[1], avg[2]);
    if s < 0.12 {
        return (122, 162, 247);
    }
    from_hsl(h, s.clamp(0.45, 0.75), 0.68)
}

fn to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 {
        0..60 => (c, x, 0.0),
        60..120 => (x, c, 0.0),
        120..180 => (0.0, c, x),
        180..240 => (0.0, x, c),
        240..300 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f32| ((v + m) * 255.0).round() as u8;
    (f(r), f(g), f(b))
}
