use crate::avatar::{Avatar, Rgb};
use crate::github::{Contributions, Event, Report, Sections, UserReport};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use colored::{ColoredString, Colorize};

pub(crate) const MARGIN: &str = "  ";
const GAP: &str = "   ";
const LABEL_W: usize = 12;
const BAR_W: usize = 48;
const TEXT_W: usize = 56;
pub(crate) const DEFAULT_ACCENT: Rgb = (122, 162, 247);

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Approximate GitHub linguist colors for the most common languages.
pub(crate) fn lang_rgb(lang: &str) -> Rgb {
    match lang {
        "Rust" => (222, 165, 132),
        "Python" => (53, 114, 165),
        "JavaScript" => (241, 224, 90),
        "TypeScript" => (49, 120, 198),
        "Go" => (0, 173, 216),
        "C" => (120, 120, 120),
        "C++" => (243, 75, 125),
        "C#" => (23, 134, 0),
        "Java" => (176, 114, 25),
        "Kotlin" => (169, 123, 255),
        "Swift" => (240, 81, 56),
        "Ruby" => (170, 40, 40),
        "PHP" => (79, 93, 149),
        "Shell" => (137, 224, 81),
        "HTML" => (227, 76, 38),
        "CSS" => (86, 61, 124),
        "Dart" => (0, 180, 171),
        "Lua" => (40, 40, 160),
        "Nix" => (126, 126, 255),
        "Zig" => (236, 145, 92),
        "Vue" => (65, 184, 131),
        "Haskell" => (94, 80, 134),
        "Makefile" => (66, 120, 25),
        "Dockerfile" => (56, 77, 84),
        "Other" => (110, 110, 110),
        _ => {
            // Stable color derived from the name for the rest.
            let h = lang.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            (100 + (h % 156) as u8, 100 + (h / 7 % 156) as u8, 100 + (h / 49 % 156) as u8)
        }
    }
}

struct Theme {
    accent: Rgb,
}

impl Theme {
    fn new(avatar: Option<&Avatar>) -> Self {
        Self { accent: avatar.map_or(DEFAULT_ACCENT, |a| a.accent) }
    }

    fn a(&self, s: &str) -> ColoredString {
        let (r, g, b) = self.accent;
        s.truecolor(r, g, b)
    }

    fn kv(&self, label: &str, value: impl std::fmt::Display) -> String {
        format!("{} {value}", self.a(&format!("{label:<LABEL_W$}")).bold())
    }

    fn section(&self, title: &str) {
        println!("\n{MARGIN}{}", self.a(title).bold());
    }
}

pub(crate) fn human(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

pub(crate) fn relative(iso: &str) -> String {
    let Ok(t) = iso.parse::<DateTime<Utc>>() else { return iso.to_string() };
    match (Utc::now() - t).num_days() {
        0 => "today".into(),
        1 => "yesterday".into(),
        d if d < 30 => format!("{d} days ago"),
        d if d < 60 => "1 month ago".into(),
        d if d < 365 => format!("{} months ago", d / 30),
        d if d < 730 => "1 year ago".into(),
        d => format!("{} years ago", d / 365),
    }
}

pub(crate) fn month_year(iso: &str) -> String {
    match iso.parse::<DateTime<Utc>>() {
        Ok(t) => format!("{} {}", MONTHS[t.month0() as usize], t.year()),
        Err(_) => iso.to_string(),
    }
}

fn short_ago(iso: &str) -> String {
    let Ok(t) = iso.parse::<DateTime<Utc>>() else { return String::new() };
    let d = Utc::now() - t;
    match (d.num_days(), d.num_hours(), d.num_minutes()) {
        (0, 0, m) => format!("{m}m"),
        (0, h, _) => format!("{h}h"),
        (d, _, _) => format!("{d}d"),
    }
}

fn strip_scheme(url: &str) -> &str {
    url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/')
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Wraps text into lines of at most `width` characters without splitting words.
fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur += word;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last_mut().unwrap();
        *last = truncate(&format!("{last} …"), width);
    }
    lines
}

/// Prints the avatar on the left and `info` on the right, row by row.
fn side_by_side(avatar: Option<&Avatar>, info: &[String]) {
    println!();
    let left = avatar.map_or(&[][..], |a| &a.lines[..]);
    let blank = avatar.map_or(String::new(), |a| format!("{}{GAP}", " ".repeat(a.width)));
    for i in 0..left.len().max(info.len()) {
        let l = left.get(i).map_or(blank.clone(), |l| format!("{l}{GAP}"));
        let r = info.get(i).map_or("", String::as_str);
        println!("{MARGIN}{l}{r}");
    }
}

fn title_block(plain_len: usize, title: String, desc: Option<&str>) -> Vec<String> {
    let mut out = vec![title, "─".repeat(plain_len).dimmed().to_string()];
    if let Some(d) = desc.map(str::trim).filter(|d| !d.is_empty()) {
        out.extend(wrap(d, TEXT_W, 3).into_iter().map(|l| l.italic().to_string()));
    }
    out.push(String::new());
    out
}

pub fn print(rep: &Report, avatar: Option<&Avatar>) {
    let t = Theme::new(avatar);
    let r = &rep.repo;
    let (owner, name) = r.full_name.split_once('/').unwrap_or(("", &r.full_name));

    let mut title = format!("{}{}", format!("{owner}/").dimmed(), t.a(name).bold());
    if r.archived {
        title += &format!("  {}", "archived".yellow());
    }
    if r.fork {
        title += &format!("  {}", "fork".dimmed());
    }
    let mut info = title_block(r.full_name.chars().count(), title, r.description.as_deref());

    info.push(t.kv("Stars", human(r.stargazers_count)));
    info.push(t.kv("Forks", human(r.forks_count)));
    if let Some(w) = r.subscribers_count {
        info.push(t.kv("Watchers", human(w)));
    }
    // GitHub's open_issues_count includes PRs; split them out when we know the PR count.
    match rep.open_prs {
        Some(prs) => {
            let issues = r.open_issues_count.saturating_sub(prs);
            info.push(t.kv("Issues", format!("{} open", human(issues))));
            info.push(t.kv("PRs", format!("{} open", human(prs))));
        }
        None => info.push(t.kv("Issues", format!("{} open (incl. PRs)", human(r.open_issues_count)))),
    }
    if let Some(rel) = &rep.latest_release {
        let when = rel.published_at.as_deref().map(relative).unwrap_or_default();
        info.push(t.kv("Release", format!("{}  {}", rel.tag_name, when.dimmed())));
    }
    if let Some(l) = &r.license {
        let id = l.spdx_id.clone().filter(|s| s != "NOASSERTION").unwrap_or(l.name.clone());
        info.push(t.kv("License", id));
    }
    info.push(t.kv("Branch", &r.default_branch));
    if let Some(p) = &r.pushed_at {
        info.push(t.kv("Last push", relative(p)));
    }
    info.push(t.kv("Created", month_year(&r.created_at)));
    // The API reports size in KB.
    let size = match r.size as f64 / 1024.0 {
        mb if mb >= 1024.0 => format!("{:.1} GB", mb / 1024.0),
        mb => format!("{mb:.1} MB"),
    };
    info.push(t.kv("Size", size));
    let url = r.homepage.as_deref().filter(|h| !h.is_empty()).unwrap_or(&r.html_url);
    info.push(t.kv("Web", strip_scheme(url)));

    side_by_side(avatar, &info);

    if !r.topics.is_empty() {
        let topics = r.topics.join("  ");
        println!();
        for line in wrap(&topics, BAR_W + 20, 3) {
            println!("{MARGIN}{}", line.dimmed());
        }
    }

    languages(&t, "Languages", &rep.languages);

    if !rep.contributors.is_empty() {
        t.section("Contributors");
        let max = rep.contributors[0].contributions.max(1);
        let name_w = rep.contributors.iter().map(|c| c.login.chars().count()).max().unwrap_or(0).min(22);
        for c in &rep.contributors {
            let w = ((c.contributions as f64 / max as f64) * 24.0).round().max(1.0) as usize;
            println!(
                "{MARGIN}{:<name_w$}  {} {}",
                truncate(&c.login, 22),
                t.a(&"━".repeat(w)),
                c.contributions.to_string().dimmed()
            );
        }
    }
    println!();
}

pub fn print_user(rep: &UserReport, avatar: Option<&Avatar>, top: usize, sections: Sections) {
    let t = Theme::new(avatar);
    let u = &rep.user;
    let org = u.kind == "Organization";

    let mut title = t.a(&u.login).bold().to_string();
    let mut plain_len = u.login.chars().count();
    if let Some(n) = u.name.as_deref().map(str::trim).filter(|n| !n.is_empty() && *n != u.login) {
        title += &format!("  {}", n.dimmed());
        plain_len += 2 + n.chars().count();
    }
    let mut info = title_block(plain_len, title, u.bio.as_deref());

    let followers = if org {
        human(u.followers)
    } else {
        format!("{}  {}", human(u.followers), format!("following {}", human(u.following)).dimmed())
    };
    info.push(t.kv("Followers", followers));
    info.push(t.kv("Repos", format!("{}  {}", u.public_repos, format!("{} gists", u.public_gists).dimmed())));
    info.push(t.kv("Stars", human(rep.total_stars)));
    info.push(t.kv("Forks", human(rep.total_forks)));

    let opt = |o: &Option<String>| o.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    if let Some(c) = opt(&u.company) {
        info.push(t.kv("Company", c));
    }
    if let Some(l) = opt(&u.location) {
        info.push(t.kv("Location", l));
    }
    if let Some(b) = opt(&u.blog) {
        info.push(t.kv("Web", strip_scheme(&b)));
    }
    if let Some(x) = opt(&u.twitter_username) {
        info.push(t.kv("X", format!("@{x}")));
    }
    info.push(t.kv("Joined", format!("{}  {}", month_year(&u.created_at), relative(&u.created_at).dimmed())));
    info.push(t.kv("Profile", strip_scheme(&u.html_url)));

    side_by_side(avatar, &info);

    if let Some(c) = &rep.contributions {
        calendar(&t, c);
    }

    languages(&t, "Languages", &rep.languages);

    if sections.repos && !rep.repos.is_empty() {
        let title = if top >= rep.repos.len() {
            format!("Repos ({})", rep.repos.len())
        } else {
            "Top repos".to_string()
        };
        t.section(&title);
        for r in rep.repos.iter().take(top) {
            let lang = r.language.as_deref().map_or(String::new(), |l| {
                let (cr, cg, cb) = lang_rgb(l);
                format!("{} {}", "●".truecolor(cr, cg, cb), l.dimmed())
            });
            println!(
                "{MARGIN}{:<28}{:>8}  {lang}",
                truncate(&r.name, 27).bold(),
                format!("★ {}", human(r.stargazers_count))
            );
            if let Some(d) = r.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
                println!("{MARGIN}  {}", truncate(d, TEXT_W + 10).dimmed());
            }
        }
    }

    if sections.activity {
        activity(&t, &rep.events, top.max(8));
    }
    println!();
}

/// GitHub dark-mode colors, from "no contributions" to "most".
const CAL_LEVELS: [Rgb; 5] = [(45, 51, 59), (14, 68, 41), (0, 109, 50), (38, 166, 65), (57, 211, 83)];
const CAL_LABEL_W: usize = 4;

fn level_index(level: &str) -> usize {
    match level {
        "FIRST_QUARTILE" => 1,
        "SECOND_QUARTILE" => 2,
        "THIRD_QUARTILE" => 3,
        "FOURTH_QUARTILE" => 4,
        _ => 0,
    }
}

fn square(level: usize) -> ColoredString {
    let (r, g, b) = CAL_LEVELS[level];
    "■".truecolor(r, g, b)
}

/// 1297 → "1,297" (thousands separator).
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// GitHub's contribution graph: one column per week, one row per day.
fn calendar(t: &Theme, c: &Contributions) {
    let cal = &c.contribution_calendar;
    // Each week takes 2 columns ("■ "). In a narrow terminal, only the most
    // recent weeks that fit are shown.
    let fit = crate::term::width().saturating_sub(MARGIN.len() + CAL_LABEL_W) / 2;
    let weeks = &cal.weeks[cal.weeks.len().saturating_sub(fit)..];
    if weeks.is_empty() {
        return;
    }
    let grid_w = CAL_LABEL_W + weeks.len() * 2 - 1;

    let total = format!("{} in the last year", thousands(cal.total_contributions));
    println!(
        "\n{MARGIN}{}{}{}",
        t.a("Contributions").bold(),
        " ".repeat(grid_w.saturating_sub("Contributions".len() + total.chars().count()).max(2)),
        total.dimmed()
    );

    // Month row: each label goes on the week where the month changes.
    let mut starts: Vec<(usize, &str)> = vec![]; // (week, month)
    let mut last_month = None;
    for (i, week) in weeks.iter().enumerate() {
        let Some(first) = week.contribution_days.first() else { continue };
        let Ok(date) = NaiveDate::parse_from_str(&first.date, "%Y-%m-%d") else { continue };
        if last_month != Some(date.month0()) {
            last_month = Some(date.month0());
            starts.push((i, MONTHS[date.month0() as usize]));
        }
    }
    // Like GitHub: if the first month only shows for a week or two, skip its label.
    if starts.len() >= 2 && starts[1].0 - starts[0].0 < 3 {
        starts.remove(0);
    }
    let mut months = " ".repeat(CAL_LABEL_W);
    for (i, label) in starts {
        let col = CAL_LABEL_W + i * 2;
        // Without overlapping the previous label (leaving a space) or overflowing on the right.
        if col > months.chars().count() && col + label.len() <= grid_w {
            months += &" ".repeat(col - months.chars().count());
            months += label;
        }
    }
    println!("{MARGIN}{}", months.dimmed());

    // One row per weekday (0 = Sunday, as on GitHub).
    for weekday in 0..7u8 {
        let label = match weekday {
            1 => "Mon",
            3 => "Wed",
            5 => "Fri",
            _ => "",
        };
        let mut line = format!("{label:<CAL_LABEL_W$}").dimmed().to_string();
        for week in weeks {
            // The first and last weeks may be incomplete.
            match week.contribution_days.iter().find(|d| d.weekday == weekday) {
                Some(d) => line += &format!("{} ", square(level_index(&d.contribution_level))),
                None => line += "  ",
            }
        }
        println!("{MARGIN}{}", line.trim_end());
    }

    // Below: breakdown by type on the left, legend on the right.
    let kinds = [
        ("commits", c.total_commit_contributions),
        ("PRs", c.total_pull_request_contributions),
        ("reviews", c.total_pull_request_review_contributions),
        ("issues", c.total_issue_contributions),
    ];
    let sum: u64 = kinds.iter().map(|k| k.1).sum();
    let mut kinds: Vec<_> = kinds.into_iter().filter(|k| k.1 > 0).collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    let parts: Vec<(String, String)> = kinds
        .iter()
        .map(|(name, n)| (name.to_string(), format!("{}%", (*n as f64 * 100.0 / sum as f64).round())))
        .collect();
    let breakdown_len: usize = parts.iter().map(|(n, p)| n.len() + 1 + p.len()).sum::<usize>() + parts.len().saturating_sub(1) * 5;
    let breakdown = parts
        .iter()
        .map(|(n, p)| format!("{} {p}", n.dimmed()))
        .collect::<Vec<_>>()
        .join(&"  ·  ".dimmed().to_string());

    let legend = format!(
        "{} {}{}",
        "Less".dimmed(),
        (0..5).map(|l| format!("{} ", square(l))).collect::<String>(),
        "More".dimmed()
    );
    let legend_len = "Less ".len() + 5 * 2 + "More".len();

    let space = grid_w.saturating_sub(CAL_LABEL_W + breakdown_len + legend_len);
    println!();
    if space >= 3 {
        println!("{MARGIN}{}{breakdown}{}{legend}", " ".repeat(CAL_LABEL_W), " ".repeat(space));
    } else {
        // Narrow terminal: one below the other.
        println!("{MARGIN}{}{breakdown}", " ".repeat(CAL_LABEL_W));
        println!("{MARGIN}{}{legend}", " ".repeat(CAL_LABEL_W));
    }
}

/// Splits `total` columns proportionally to `values`, adding up to exactly
/// `total` and giving each value at least 1 column (largest remainder method).
fn distribute(values: &[u64], total: usize) -> Vec<usize> {
    let sum: u64 = values.iter().sum();
    let exact: Vec<f64> = values.iter().map(|&v| v as f64 * total as f64 / sum as f64).collect();
    let mut out: Vec<usize> = exact.iter().map(|e| (e.floor() as usize).max(1)).collect();
    let mut used: usize = out.iter().sum();

    // Rounding left columns over: give them to the largest fractional parts.
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())));
    for &i in &order {
        if used >= total {
            break;
        }
        out[i] += 1;
        used += 1;
    }
    // The minimum of 1 overshot: take columns back from the widest.
    while used > total {
        let Some(i) = (0..out.len()).filter(|&i| out[i] > 1).max_by_key(|&i| out[i]) else { break };
        out[i] -= 1;
        used -= 1;
    }
    out
}

fn languages(t: &Theme, title: &str, langs: &[(String, u64)]) {
    let total: u64 = langs.iter().map(|l| l.1).sum();
    if total == 0 {
        return;
    }
    // Up to 6 languages with at least 1%; the rest goes into "Other".
    let mut shown: Vec<(&str, u64)> = vec![];
    let mut other = 0;
    for (name, n) in langs {
        if *n * 100 >= total && shown.len() < 6 {
            shown.push((name, *n));
        } else {
            other += n;
        }
    }
    if other > 0 {
        shown.push(("Other", other));
    }

    t.section(title);
    let widths = distribute(&shown.iter().map(|s| s.1).collect::<Vec<_>>(), BAR_W);
    let bar: String = shown
        .iter()
        .zip(&widths)
        .map(|((name, _), w)| {
            let (r, g, b) = lang_rgb(name);
            "━".repeat(*w).truecolor(r, g, b).to_string()
        })
        .collect();
    println!("{MARGIN}{bar}");

    let cells: Vec<String> = shown
        .iter()
        .map(|(name, n)| {
            let (r, g, b) = lang_rgb(name);
            let pct = format!("{:.1}%", *n as f64 * 100.0 / total as f64);
            // Padding is computed on the plain text so ANSI codes aren't counted.
            let pad = 22usize.saturating_sub(name.chars().count() + pct.len() + 3);
            format!("{} {name} {}{}", "●".truecolor(r, g, b), pct.dimmed(), " ".repeat(pad))
        })
        .collect();
    for row in cells.chunks(3) {
        println!("{MARGIN}{}", row.concat().trim_end());
    }
}

/// Turns an event into (tag, detail). None = an event we don't show.
fn describe(e: &Event) -> Option<(&'static str, String)> {
    let p = &e.payload;
    let action = p["action"].as_str().unwrap_or("");
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").to_string();
    Some(match e.kind.as_str() {
        "PushEvent" => ("push", s(&p["ref"]).trim_start_matches("refs/heads/").to_string()),
        "PullRequestEvent" => {
            let merged = action == "closed" && p["pull_request"]["merged"].as_bool() == Some(true);
            let tag = if merged { "merge" } else { "pr" };
            (tag, with_verb(verb(action, merged), &title(&p["pull_request"], p)))
        }
        "IssuesEvent" => ("issue", with_verb(verb(action, false), &title(&p["issue"], p))),
        "IssueCommentEvent" => ("comment", title(&p["issue"], p)),
        "PullRequestReviewEvent" => ("review", title(&p["pull_request"], p)),
        "PullRequestReviewCommentEvent" => ("review", title(&p["pull_request"], p)),
        "CreateEvent" => match p["ref_type"].as_str() {
            Some("repository") => ("create", "new repository".into()),
            Some(kind) => ("create", format!("{kind} {}", s(&p["ref"]))),
            None => ("create", String::new()),
        },
        "DeleteEvent" => ("delete", format!("{} {}", s(&p["ref_type"]), s(&p["ref"]))),
        "WatchEvent" => ("star", String::new()),
        "ForkEvent" => ("fork", String::new()),
        "ReleaseEvent" => ("release", s(&p["release"]["tag_name"])),
        "PublicEvent" => ("public", String::new()),
        _ => return None,
    })
}

fn verb(action: &str, merged: bool) -> &'static str {
    match action {
        "opened" => "opened",
        "closed" if !merged => "closed",
        "reopened" => "reopened",
        _ => "",
    }
}

/// Title of a PR/issue. GitHub sometimes omits it in events; then we use
/// its number ("#123").
fn title(item: &serde_json::Value, payload: &serde_json::Value) -> String {
    if let Some(t) = item["title"].as_str().filter(|t| !t.is_empty()) {
        return t.to_string();
    }
    item["number"]
        .as_u64()
        .or_else(|| payload["number"].as_u64())
        .map_or(String::new(), |n| format!("#{n}"))
}

/// "opened" + "Fix X" → "opened: Fix X"; "opened" + "#12" → "opened #12".
fn with_verb(verb: &str, title: &str) -> String {
    match (verb, title) {
        ("", t) => t.to_string(),
        (v, "") => v.to_string(),
        (v, t) if t.starts_with('#') => format!("{v} {t}"),
        (v, t) => format!("{v}: {t}"),
    }
}

fn tag_color(tag: &str) -> ColoredString {
    let padded = format!("{tag:<9}");
    match tag {
        "push" | "create" | "release" | "public" => padded.green(),
        "pr" | "merge" | "review" => padded.magenta(),
        "issue" => padded.yellow(),
        "delete" => padded.red(),
        "star" => padded.yellow(),
        _ => padded.normal(),
    }
}

fn activity(t: &Theme, events: &[Event], max: usize) {
    // Merge identical consecutive events (e.g. 5 pushes to the same branch) into one "×5" line.
    let mut rows: Vec<(&Event, &str, String, usize)> = vec![];
    for e in events {
        let Some((tag, detail)) = describe(e) else { continue };
        if let Some(last) = rows.last_mut().filter(|l| l.1 == tag && l.0.repo.name == e.repo.name && l.2 == detail) {
            last.3 += 1;
            continue;
        }
        rows.push((e, tag, detail, 1));
    }
    if rows.is_empty() {
        return;
    }

    t.section("Recent activity");
    for (e, tag, detail, n) in rows.into_iter().take(max) {
        let times = if n > 1 { format!(" ×{n}") } else { String::new() };
        let detail = truncate(&detail, 48);
        println!(
            "{MARGIN}{:>4}  {}{}{}  {}",
            short_ago(&e.created_at).dimmed(),
            tag_color(tag),
            e.repo.name,
            times.dimmed(),
            detail.dimmed()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribute_sums_exactly() {
        for values in [vec![955, 41, 4], vec![1, 1, 1], vec![100, 1, 1, 1, 1, 1, 1], vec![7]] {
            let out = distribute(&values, BAR_W);
            assert_eq!(out.iter().sum::<usize>(), BAR_W, "{values:?} -> {out:?}");
            assert!(out.iter().all(|&w| w >= 1));
        }
    }

    #[test]
    fn wrap_respects_width() {
        let lines = wrap("one two three four five six seven eight", 10, 10);
        assert!(lines.iter().all(|l| l.chars().count() <= 10), "{lines:?}");
        assert_eq!(lines.join(" "), "one two three four five six seven eight");
    }
}
