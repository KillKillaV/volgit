//! Caché en disco de las respuestas de GitHub y de las fotos.
//!
//! Cada entrada es un archivo en `~/.cache/volgit/` (o `$XDG_CACHE_HOME/volgit`)
//! cuyo nombre es un hash de la clave (normalmente la URL). Dentro va la clave
//! en la primera línea, para descartar colisiones, y después el contenido tal
//! cual. La caducidad se mira con la fecha de modificación del archivo.
//!
//! Es "best effort": si algo falla al leer o escribir, se ignora y se va a la
//! red como si no hubiera caché.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Las entradas más viejas que esto se borran al arrancar, aunque el TTL sea menor.
const PRUNE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

pub struct Cache {
    dir: PathBuf,
    ttl: Duration,
}

impl Cache {
    /// None si el TTL es 0 (caché desactivada) o no se encuentra la carpeta de caché.
    pub fn new(ttl: Duration) -> Option<Self> {
        if ttl.is_zero() {
            return None;
        }
        let base = std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
        let cache = Self::at(base.join("volgit"), ttl);
        cache.prune();
        Some(cache)
    }

    /// Caché en una carpeta concreta (lo usan los tests).
    fn at(dir: PathBuf, ttl: Duration) -> Self {
        Self { dir, ttl }
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{:016x}", fnv1a(key)))
    }

    /// El contenido guardado para `key`, si existe y no ha caducado.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.path(key);
        let age = fs::metadata(&path).ok()?.modified().ok()?.elapsed().ok()?;
        if age > self.ttl {
            return None;
        }
        let data = fs::read(&path).ok()?;
        let newline = data.iter().position(|&b| b == b'\n')?;
        (&data[..newline] == key.as_bytes()).then(|| data[newline + 1..].to_vec())
    }

    pub fn put(&self, key: &str, body: &[u8]) {
        if fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let mut data = Vec::with_capacity(key.len() + 1 + body.len());
        data.extend_from_slice(key.as_bytes());
        data.push(b'\n');
        data.extend_from_slice(body);
        // Se escribe a un temporal y se renombra: así otro volgit ejecutándose a
        // la vez nunca lee un archivo a medio escribir.
        let path = self.path(key);
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        if fs::write(&tmp, &data).is_ok() && fs::rename(&tmp, &path).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }

    /// Borra las entradas muy viejas para que la carpeta no crezca sin límite.
    fn prune(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else { return };
        let limit = self.ttl.max(PRUNE_AFTER);
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > limit);
            if old {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Hash FNV-1a de 64 bits. Se usa en vez del hasher de la librería estándar
/// porque este da siempre el mismo resultado, en cualquier versión de Rust.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache(ttl: Duration) -> Cache {
        let dir = std::env::temp_dir().join(format!("volgit-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = fs::remove_dir_all(&dir);
        Cache::at(dir, ttl)
    }

    #[test]
    fn guarda_y_recupera() {
        let c = temp_cache(Duration::from_secs(60));
        assert_eq!(c.get("https://x/a"), None);
        c.put("https://x/a", b"{\"n\":1}");
        assert_eq!(c.get("https://x/a").as_deref(), Some(&b"{\"n\":1}"[..]));
        assert_eq!(c.get("https://x/b"), None);
        let _ = fs::remove_dir_all(&c.dir);
    }

    #[test]
    fn caduca() {
        let c = temp_cache(Duration::from_nanos(1));
        c.put("k", b"v");
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(c.get("k"), None);
        let _ = fs::remove_dir_all(&c.dir);
    }

    #[test]
    fn hash_estable() {
        // Valores conocidos de FNV-1a 64: si cambian, las cachés existentes dejarían de valer.
        assert_eq!(fnv1a(""), 0xcbf29ce484222325);
        assert_eq!(fnv1a("a"), 0xaf63dc4c8601ec8c);
    }
}
