mod avatar;
mod github;
mod render;

use anyhow::{Context, Result, bail};
use clap::{ArgAction, CommandFactory, Parser};
use std::io::IsTerminal;
use std::process::Command;

const EXAMPLES: &str = "\
Ejemplos:
  volgit sharkdp/bat                  Ficha de un repositorio
  volgit https://github.com/o/r.git   También acepta URLs (https o ssh)
  volgit                              Repo del directorio actual (remote origin)
  volgit @BurntSushi                  Perfil de un usuario
  volgit rust-lang                    Perfil de una organización
  volgit @BurntSushi --top 10         Más repos y más actividad
  volgit @BurntSushi --top all        Todos sus repos (sin forks)
  volgit @BurntSushi --panel          Con el panel de contribuciones
  volgit sharkdp/bat --json | jq .    Datos en JSON para scripts

Límite de la API:
  Sin token GitHub permite 60 peticiones/hora; con token, 5000.
  Crea uno sin permisos en https://github.com/settings/personal-access-tokens
  y expórtalo:  export GITHUB_TOKEN=github_pat_...";

/// Consulta repositorios, usuarios y organizaciones de GitHub desde la terminal.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = EXAMPLES,
    disable_help_flag = true,
    disable_version_flag = true,
    next_help_heading = "Opciones",
    override_usage = "volgit [OPCIONES] [OBJETIVO]",
    help_template = "{name} {version}\n{about}\n\nUso: {usage}\n\n{all-args}{after-help}",
)]
struct Cli {
    /// "owner/repo", "@usuario" o URL de GitHub. Si se omite, usa el remote "origin" del directorio actual
    #[arg(value_name = "OBJETIVO", help_heading = "Argumentos")]
    repo: Option<String>,

    /// Cuántos contribuidores / repos mostrar, o "all" para todos (por defecto 5)
    #[arg(short, long, default_value = "5", value_name = "N|all", hide_default_value = true, value_parser = parse_top)]
    top: usize,

    /// Salida en JSON (para scripts)
    #[arg(long)]
    json: bool,

    /// Desactiva los colores (y la foto)
    #[arg(long)]
    no_color: bool,

    /// No muestra la foto de perfil
    #[arg(long)]
    no_avatar: bool,

    /// Muestra el panel de contribuciones del último año (solo usuarios, requiere token)
    #[arg(short, long)]
    panel: bool,

    /// Ancho de la foto en columnas, de 8 a 80 (por defecto 28)
    #[arg(long, default_value_t = 28, value_name = "N", hide_default_value = true, value_parser = clap::value_parser!(u16).range(8..=80))]
    avatar_size: u16,

    /// Token de GitHub (por defecto lee GITHUB_TOKEN)
    #[arg(long, env = "GITHUB_TOKEN", hide_env_values = true, value_name = "TOKEN")]
    token: Option<String>,

    /// Muestra esta ayuda
    #[arg(short, long, action = ArgAction::Help)]
    help: Option<bool>,

    /// Muestra la versión
    #[arg(short = 'V', long, action = ArgAction::Version)]
    version: Option<bool>,
}

/// Extrae (owner, repo) de "owner/repo", "https://github.com/o/r(.git)" o "git@github.com:o/r.git".
fn parse_slug(s: &str) -> Option<(String, String)> {
    let s = s.trim().trim_end_matches('/').trim_end_matches(".git");
    let s = s
        .split_once("github.com")
        .map(|(_, rest)| rest.trim_start_matches([':', '/']))
        .unwrap_or(s);
    let mut parts = s.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !name.is_empty()).then(|| (owner.into(), name.into()))
}

/// `--top`: un número o "all" (sin límite, representado como usize::MAX).
fn parse_top(s: &str) -> Result<usize, String> {
    match s.to_lowercase().as_str() {
        "all" | "todos" => Ok(usize::MAX),
        n => n.parse().map_err(|_| format!("se esperaba un número o \"all\", no \"{s}\"")),
    }
}

/// Acepta "@login", "login" o "https://github.com/login".
fn parse_user(s: &str) -> Option<String> {
    let s = s.trim().trim_end_matches('/');
    let s = s.split_once("github.com/").map(|(_, r)| r).unwrap_or(s);
    let s = s.strip_prefix('@').unwrap_or(s);
    (!s.is_empty() && !s.contains(['/', ':'])).then(|| s.to_string())
}

/// Descarga y convierte la foto. GitHub acepta `s=` para pedirla ya reducida.
fn load_avatar(gh: &github::GitHub, url: &str, cols: usize) -> Option<avatar::Avatar> {
    let sep = if url.contains('?') { '&' } else { '?' };
    let bytes = gh.download(&format!("{url}{sep}s={}", cols * 4))?;
    avatar::Avatar::from_bytes(&bytes, cols)
}

fn origin_remote() -> Result<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("no se pudo ejecutar git")?;
    if !out.status.success() {
        bail!("no se indicó repo y el directorio actual no tiene remote 'origin'");
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn main() -> Result<()> {
    // Rust ignora SIGPIPE por defecto, así que `volgit ... | head` acabaría en
    // pánico al cerrarse la tubería. Restauramos el comportamiento clásico de
    // Unix: el proceso termina en silencio.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let cli = Cli::parse();
    if cli.no_color {
        colored::control::set_override(false);
    }

    let input = match cli.repo {
        // `volgit help` también muestra la ayuda (para un usuario llamado "help", usa @help).
        Some(r) if r == "help" => {
            Cli::command().print_help()?;
            return Ok(());
        }
        Some(r) => r,
        None => origin_remote()?,
    };
    // `GITHUB_TOKEN=` (vacío) cuenta como "sin token", no como token inválido.
    let gh = github::GitHub::new(cli.token.filter(|t| !t.trim().is_empty()))?;
    // La foto va con códigos ANSI crudos: solo tiene sentido en una terminal con color.
    let show_avatar = !cli.json && !cli.no_avatar && !cli.no_color && std::io::stdout().is_terminal();
    let cols = cli.avatar_size as usize;

    // "@usuario" o un nombre sin "/" → perfil de usuario/organización.
    if let Some(login) = parse_user(&input) {
        let report = gh.user_report(&login, cli.panel)?;
        if cli.panel && report.contributions.is_none() {
            let why = if report.user.kind != "User" {
                "las organizaciones no tienen panel de contribuciones"
            } else if !gh.has_token() {
                "el panel necesita un token (GITHUB_TOKEN)"
            } else {
                "GitHub no devolvió el calendario de contribuciones"
            };
            eprintln!("aviso: {why}");
        }
        if cli.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            let avatar = show_avatar.then(|| load_avatar(&gh, &report.user.avatar_url, cols)).flatten();
            render::print_user(&report, avatar.as_ref(), cli.top);
        }
        return Ok(());
    }

    let (owner, name) = parse_slug(&input).with_context(|| format!("repo no válido: {input}"))?;
    let report = gh.report(&owner, &name, cli.top)?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let avatar = show_avatar.then(|| load_avatar(&gh, &report.repo.owner.avatar_url, cols)).flatten();
        render::print(&report, avatar.as_ref());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repos_en_todos_los_formatos() {
        let want = Some(("o".to_string(), "r".to_string()));
        for input in ["o/r", "https://github.com/o/r", "https://github.com/o/r.git", "git@github.com:o/r.git", "github.com/o/r/"] {
            assert_eq!(parse_slug(input), want, "{input}");
        }
        assert_eq!(parse_slug("solo"), None);
    }

    #[test]
    fn top() {
        assert_eq!(parse_top("7"), Ok(7));
        assert_eq!(parse_top("all"), Ok(usize::MAX));
        assert_eq!(parse_top("ALL"), Ok(usize::MAX));
        assert!(parse_top("muchos").is_err());
    }

    #[test]
    fn usuarios() {
        assert_eq!(parse_user("@BurntSushi").as_deref(), Some("BurntSushi"));
        assert_eq!(parse_user("rust-lang").as_deref(), Some("rust-lang"));
        assert_eq!(parse_user("https://github.com/torvalds/").as_deref(), Some("torvalds"));
        assert_eq!(parse_user("o/r"), None);
        assert_eq!(parse_user("git@github.com:o/r.git"), None);
    }
}
