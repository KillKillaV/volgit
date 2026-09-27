//! Terminal queries: size and image support.

/// Window size as reported by the OS (ioctl TIOCGWINSZ). None if stdout is
/// not a terminal.
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

/// Width in columns. Falls back to $COLUMNS, then to 120.
pub fn width() -> usize {
    winsize()
        .map(|ws| ws.ws_col as usize)
        .or_else(|| std::env::var("COLUMNS").ok()?.parse().ok())
        .unwrap_or(120)
}

/// Width/height ratio of a cell in pixels (≈0.5 for most fonts). Needed so
/// that a square image is drawn square.
pub fn cell_aspect() -> f32 {
    winsize()
        .filter(|ws| ws.ws_xpixel > 0 && ws.ws_ypixel > 0)
        .map(|ws| {
            (ws.ws_xpixel as f32 / ws.ws_col as f32) / (ws.ws_ypixel as f32 / ws.ws_row as f32)
        })
        .unwrap_or(0.5)
}

/// Does the terminal support the kitty graphics protocol?
/// kitty, Ghostty and WezTerm do. Inside tmux/screen it never reaches the real terminal.
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
