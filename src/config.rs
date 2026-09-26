//! Archivo de configuración: `~/.config/volgit/config.toml`
//! (o `$XDG_CONFIG_HOME/volgit/config.toml`).
//!
//! Guarda las opciones por defecto para no tener que escribirlas siempre.
//! Lo que se pase por línea de comandos manda sobre lo que diga el archivo.

use crate::ImageArg;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::path::PathBuf;

/// Plantilla que escribe `--init-config`. Todo va comentado: sin tocar nada,
/// volgit se comporta igual que sin archivo.
pub const TEMPLATE: &str = r#"# volgit configuration. Command-line flags take precedence over these values.
# Uncomment the lines you want to change.

# Sections shown on user profiles by default
# repos = true
# activity = true
# panel = true

# How many repos / contributors to show: a number or "all"
# top = 5

# Avatar: "auto" (kitty graphics if supported), "kitty" or "blocks"
# image = "auto"
# avatar_size = 28
# avatar = true
# color = true

# Minutes to keep GitHub responses cached (0 disables the cache)
# cache_minutes = 10
"#;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub repos: bool,
    pub activity: bool,
    pub panel: bool,
    pub top: Option<Top>,
    pub image: Option<ImageArg>,
    pub avatar_size: Option<u16>,
    pub avatar: Option<bool>,
    pub color: Option<bool>,
    pub cache_minutes: Option<u64>,
}

/// `top = 5` o `top = "all"`: en TOML son tipos distintos, así que se aceptan los dos.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Top {
    Number(usize),
    Word(String),
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("volgit").join("config.toml"))
    }

    /// Lee el archivo si existe. Si no existe, configuración vacía; si está mal, error.
    pub fn load() -> Result<Self> {
        let Some(path) = Self::path() else { return Ok(Self::default()) };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("could not read {}", path.display())),
        };
        Self::parse(&text).with_context(|| format!("invalid config file {}", path.display()))
    }

    fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        if let Some(n) = config.avatar_size.filter(|n| !(8..=80).contains(n)) {
            bail!("avatar_size must be between 8 and 80, got {n}");
        }
        Ok(config)
    }

    /// `top` ya convertido: número, o usize::MAX para "all".
    pub fn top(&self) -> Result<Option<usize>> {
        match &self.top {
            None => Ok(None),
            Some(Top::Number(n)) => Ok(Some(*n)),
            Some(Top::Word(w)) => crate::parse_top(w).map(Some).map_err(anyhow::Error::msg),
        }
    }

    /// Crea el archivo con la plantilla. No sobrescribe uno existente.
    pub fn init() -> Result<(PathBuf, bool)> {
        let path = Self::path().context("could not find the config directory ($HOME is not set)")?;
        if path.exists() {
            return Ok((path, false));
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
        }
        std::fs::write(&path, TEMPLATE).with_context(|| format!("could not write {}", path.display()))?;
        Ok((path, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plantilla_vale_y_no_cambia_nada() {
        let c = Config::parse(TEMPLATE).unwrap();
        assert!(!c.repos && !c.activity && !c.panel);
        assert!(c.top.is_none() && c.image.is_none() && c.cache_minutes.is_none());
    }

    #[test]
    fn lee_valores() {
        let c = Config::parse("repos = true\ntop = \"all\"\nimage = \"blocks\"\navatar_size = 30\ncache_minutes = 0").unwrap();
        assert!(c.repos);
        assert_eq!(c.top().unwrap(), Some(usize::MAX));
        assert!(matches!(c.image, Some(ImageArg::Blocks)));
        assert_eq!(c.avatar_size, Some(30));
        assert_eq!(c.cache_minutes, Some(0));
        assert_eq!(Config::parse("top = 3").unwrap().top().unwrap(), Some(3));
    }

    #[test]
    fn rechaza_errores() {
        assert!(Config::parse("repo = true").is_err()); // clave mal escrita
        assert!(Config::parse("avatar_size = 200").is_err());
        assert!(Config::parse("image = \"sixel\"").is_err());
        assert!(Config::parse("top = \"muchos\"").unwrap().top().is_err());
    }
}
