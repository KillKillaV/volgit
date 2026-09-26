//! Renderiza una imagen en la terminal con medios bloques (▀): cada celda
//! pinta dos píxeles, el de arriba como color de texto y el de abajo como fondo.

use image::imageops::FilterType;
use image::{Rgba, RgbaImage};

pub type Rgb = (u8, u8, u8);

pub struct Avatar {
    /// Una línea por fila de celdas, ya con códigos ANSI.
    pub lines: Vec<String>,
    /// Ancho visible en columnas (igual para todas las líneas).
    pub width: usize,
    /// Color representativo de la imagen, para usar como acento.
    pub accent: Rgb,
}

impl Avatar {
    pub fn from_bytes(bytes: &[u8], cols: usize) -> Option<Self> {
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let size = cols as u32;
        // Lanczos3 conserva mejor los detalles al reducir tanto la imagen.
        let mut img = image::imageops::resize(&img, size, size, FilterType::Lanczos3);
        round_corners(&mut img, size as f32 * 0.22);

        let mut lines = Vec::with_capacity(cols / 2 + 1);
        for y in (0..size).step_by(2) {
            let mut line = String::new();
            for x in 0..size {
                let top = img.get_pixel(x, y);
                let bottom = (y + 1 < size).then(|| img.get_pixel(x, y + 1));
                line += &cell(top, bottom);
            }
            line += "\x1b[0m";
            lines.push(line);
        }
        Some(Self { lines, width: cols, accent: accent(&img) })
    }
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

/// Hace transparentes las esquinas para dar un cuadrado redondeado.
fn round_corners(img: &mut RgbaImage, r: f32) {
    let (w, h) = (img.width() as f32, img.height() as f32);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let cx = px.clamp(r, w - r);
        let cy = py.clamp(r, h - r);
        if (px - cx).powi(2) + (py - cy).powi(2) > r * r {
            p[3] = 0;
        }
    }
}

/// Media de los píxeles pesada por saturación, normalizada a un tono legible
/// sobre fondo oscuro. Si la imagen es casi gris, devuelve un azul neutro.
fn accent(img: &RgbaImage) -> Rgb {
    let (mut sum, mut weight) = ([0f32; 3], 0f32);
    for p in img.pixels().filter(|p| visible(p)) {
        let (_, s, l) = to_hsl(p[0], p[1], p[2]);
        // Ignora casi negros y casi blancos: no aportan color.
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
