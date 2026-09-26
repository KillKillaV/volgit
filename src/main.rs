mod avatar;
mod cache;
mod config;
mod github;
mod render;
mod term;

use anyhow::{Context, Result, bail};
use clap::{ArgAction, CommandFactory, Parser};
use std::io::IsTerminal;
use std::process::Command;

const EXAMPLES: &str = "\
Examples:
  volgit sharkdp/bat                  Repository card
  volgit https://github.com/o/r.git   URLs work too (https or ssh)
  volgit                              Repo in the current directory (remote origin)
  volgit @BurntSushi                  User profile
  volgit rust-lang                    Organization profile
  volgit @BurntSushi --repos          With their top repos
  volgit @BurntSushi --activity       With their recent activity
  volgit @BurntSushi -r --top all     All their repos (forks excluded)
  volgit @BurntSushi --panel          With the contribution graph
  volgit @BurntSushi --image blocks   Block avatar even inside kitty
  volgit sharkdp/bat --json | jq .    JSON output for scripts

Config file:
  ~/.config/volgit/config.toml sets your defaults (e.g. repos = true).
  Create a commented template with --init-config; --no-config ignores it.

Cache:
  GitHub responses are cached for 10 minutes in ~/.cache/volgit.
  Use --no-cache to force fresh data, or set cache_minutes in the config.

Shell completions:
  volgit --completions bash > ~/.local/share/bash-completion/completions/volgit
  volgit --completions zsh  > ~/.zfunc/_volgit   (with ~/.zfunc in your fpath)
  volgit --completions fish > ~/.config/fish/completions/volgit.fish

API rate limit:
  Without a token GitHub allows 60 requests/hour; with one, 5000.
  Create one with no permissions at https://github.com/settings/personal-access-tokens
  and export it:  export GITHUB_TOKEN=github_pat_...";

/// Look up GitHub repositories, users and organizations from the terminal.
#[derive(Parser)]
#[command(
    version,
    about,
    after_help = EXAMPLES,
    disable_help_flag = true,
    disable_version_flag = true,
    next_help_heading = "Options",
    override_usage = "volgit [OPTIONS] [TARGET]",
    help_template = "{name} {version}\n{about}\n\nUsage: {usage}\n\n{all-args}{after-help}",
)]
struct Cli {
    /// "owner/repo", "@user" or a GitHub URL. If omitted, uses the "origin" remote of the current directory
    #[arg(value_name = "TARGET", help_heading = "Arguments")]
    repo: Option<String>,

    /// How many contributors / repos to show, or "all" (default 5)
    #[arg(short, long, value_name = "N|all", value_parser = parse_top)]
    top: Option<usize>,

    /// JSON output (for scripts)
    #[arg(long)]
    json: bool,

    /// Disable colors (and the avatar)
    #[arg(long)]
    no_color: bool,

    /// Hide the avatar
    #[arg(long)]
    no_avatar: bool,

    /// Show the contribution graph for the last year (users only, needs a token)
    #[arg(short, long)]
    panel: bool,

    /// Show the profile's top repos
    #[arg(short, long)]
    repos: bool,

    /// Show the profile's recent activity
    #[arg(short, long)]
    activity: bool,

    /// How to draw the avatar: auto (kitty if supported), kitty or blocks
    #[arg(long, value_enum, value_name = "MODE", hide_possible_values = true)]
    image: Option<ImageArg>,

    /// Avatar width in columns, 8 to 80 (default 28)
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u16).range(8..=80))]
    avatar_size: Option<u16>,

    /// Always fetch fresh data, skipping the cache
    #[arg(long)]
    no_cache: bool,

    /// Ignore the config file
    #[arg(long)]
    no_config: bool,

    /// Create a config file template and print its path
    #[arg(long)]
    init_config: bool,

    /// Print a completion script for your shell (bash, zsh, fish, elvish, powershell)
    #[arg(long, value_name = "SHELL", hide_possible_values = true)]
    completions: Option<clap_complete::Shell>,

    /// GitHub token (defaults to GITHUB_TOKEN)
    #[arg(long, env = "GITHUB_TOKEN", hide_env_values = true, value_name = "TOKEN")]
    token: Option<String>,

    /// Print help
    #[arg(short, long, action = ArgAction::Help)]
    help: Option<bool>,

    /// Print version
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
        "all" => Ok(usize::MAX),
        n => n.parse().map_err(|_| format!("expected a number or \"all\", got \"{s}\"")),
    }
}

/// Acepta "@login", "login" o "https://github.com/login".
fn parse_user(s: &str) -> Option<String> {
    let s = s.trim().trim_end_matches('/');
    let s = s.split_once("github.com/").map(|(_, r)| r).unwrap_or(s);
    let s = s.strip_prefix('@').unwrap_or(s);
    (!s.is_empty() && !s.contains(['/', ':'])).then(|| s.to_string())
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ImageArg {
    Auto,
    Kitty,
    Blocks,
}

impl ImageArg {
    fn resolve(self) -> avatar::Mode {
        match self {
            ImageArg::Kitty => avatar::Mode::Kitty,
            ImageArg::Blocks => avatar::Mode::Blocks,
            ImageArg::Auto if term::supports_kitty_graphics() => avatar::Mode::Kitty,
            ImageArg::Auto => avatar::Mode::Blocks,
        }
    }
}

/// Descarga y convierte la foto. GitHub acepta `s=` para pedirla ya al tamaño
/// justo: pequeña para los bloques, a buena resolución para kitty.
fn load_avatar(gh: &github::GitHub, url: &str, cols: usize, mode: avatar::Mode) -> Option<avatar::Avatar> {
    let px = match mode {
        avatar::Mode::Blocks => cols * 4,
        avatar::Mode::Kitty => 460,
    };
    let sep = if url.contains('?') { '&' } else { '?' };
    let bytes = gh.download(&format!("{url}{sep}s={px}"))?;
    avatar::Avatar::from_bytes(&bytes, cols, mode)
}

fn origin_remote() -> Result<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .context("could not run git")?;
    if !out.status.success() {
        bail!("no target given and the current directory has no 'origin' remote");
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

    // Acciones que no consultan GitHub.
    if let Some(shell) = cli.completions {
        clap_complete::generate(shell, &mut Cli::command(), "volgit", &mut std::io::stdout());
        return Ok(());
    }
    if cli.init_config {
        let (path, created) = config::Config::init()?;
        let what = if created { "created" } else { "already exists, left untouched:" };
        println!("config file {what} {}", path.display());
        return Ok(());
    }

    // Cada opción sale de la línea de comandos si se ha escrito; si no, del
    // archivo de configuración; y si tampoco, del valor por defecto.
    let cfg = if cli.no_config { config::Config::default() } else { config::Config::load()? };
    let top = cli.top.or(cfg.top()?).unwrap_or(5);
    let sections = github::Sections {
        panel: cli.panel || cfg.panel,
        repos: cli.repos || cfg.repos,
        activity: cli.activity || cfg.activity,
    };
    let no_color = cli.no_color || cfg.color == Some(false);
    let no_avatar = cli.no_avatar || cfg.avatar == Some(false);
    let cols = cli.avatar_size.or(cfg.avatar_size).unwrap_or(28) as usize;
    let mode = cli.image.or(cfg.image).unwrap_or(ImageArg::Auto).resolve();
    let cache_minutes = if cli.no_cache { 0 } else { cfg.cache_minutes.unwrap_or(10) };

    if no_color {
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
    let cache = cache::Cache::new(std::time::Duration::from_secs(cache_minutes * 60));
    let gh = github::GitHub::new(cli.token.filter(|t| !t.trim().is_empty()), cache)?;
    // La foto va con códigos ANSI crudos: solo tiene sentido en una terminal con color.
    let show_avatar = !cli.json && !no_avatar && !no_color && std::io::stdout().is_terminal();

    // "@usuario" o un nombre sin "/" → perfil de usuario/organización.
    if let Some(login) = parse_user(&input) {
        let report = gh.user_report(&login, sections)?;
        if sections.panel && report.contributions.is_none() {
            let why = if report.user.kind != "User" {
                "organizations don't have a contribution graph"
            } else if !gh.has_token() {
                "the contribution graph needs a token (GITHUB_TOKEN)"
            } else {
                "GitHub did not return the contribution calendar"
            };
            eprintln!("warning: {why}");
        }
        if cli.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            let avatar = show_avatar.then(|| load_avatar(&gh, &report.user.avatar_url, cols, mode)).flatten();
            render::print_user(&report, avatar.as_ref(), top, sections);
        }
        return Ok(());
    }

    let (owner, name) = parse_slug(&input).with_context(|| format!("invalid repo: {input}"))?;
    let report = gh.report(&owner, &name, top)?;

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let avatar = show_avatar.then(|| load_avatar(&gh, &report.repo.owner.avatar_url, cols, mode)).flatten();
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
