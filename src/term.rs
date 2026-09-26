//! Consultas sobre la terminal: tamaño y soporte de imágenes.

/// Tamaño de la ventana según el sistema (ioctl TIOCGWINSZ). None si la salida
/// no es una terminal.
fn winsize() -> Option<libc::winsize> {
    #[cfg(unix)]
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 {
            return Some(ws);
        }
    }
    None
}

/// Ancho en columnas. Si no se puede preguntar, prueba $COLUMNS y si no asume 120.
pub fn width() -> usize {
    winsize()
        .map(|ws| ws.ws_col as usize)
        .or_else(|| std::env::var("COLUMNS").ok()?.parse().ok())
        .unwrap_or(120)
}

/// Proporción ancho/alto de una celda en píxeles (≈0.5 en casi todas las
/// fuentes). Hace falta para que una imagen cuadrada salga cuadrada.
pub fn cell_aspect() -> f32 {
    winsize()
        .filter(|ws| ws.ws_xpixel > 0 && ws.ws_ypixel > 0)
        .map(|ws| (ws.ws_xpixel as f32 / ws.ws_col as f32) / (ws.ws_ypixel as f32 / ws.ws_row as f32))
        .unwrap_or(0.5)
}

/// ¿La terminal entiende el protocolo gráfico de kitty?
/// kitty, Ghostty y WezTerm sí. Dentro de tmux/screen no llega a la terminal real.
pub fn supports_kitty_graphics() -> bool {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    if !var("TMUX").is_empty() || var("TERM").starts_with("screen") {
        return false;
    }
    !var("KITTY_WINDOW_ID").is_empty()
        || var("TERM") == "xterm-kitty"
        || var("TERM") == "xterm-ghostty"
        || matches!(var("TERM_PROGRAM").as_str(), "ghostty" | "WezTerm")
}
