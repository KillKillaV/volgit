//! Side-by-side comparison of several repos (or several users).
//!
//! Each row is a metric (stars, last push...) and each column a repo. In rows
//! where "more is better", the best value is highlighted in green.

use crate::github::{Report, UserReport};
use crate::render::{DEFAULT_ACCENT, MARGIN, human, lang_rgb, month_year, relative, truncate};
use chrono::{DateTime, Utc};
use colored::Colorize;

/// Maximum column width; longer text is cut with "…".
const MAX_COL_W: usize = 30;
const COL_GAP: usize = 3;

struct Cell {
    text: String,
    /// Value used to pick the best in the row (higher = better). None = not compared.
    score: Option<f64>,
    /// Color of the dot before the text (used for languages).
    dot: Option<(u8, u8, u8)>,
}

impl Cell {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            score: None,
            dot: None,
        }
    }

    fn number(n: u64) -> Self {
        Self {
            text: human(n),
            score: Some(n as f64),
            dot: None,
        }
    }

    /// Relative date ("3 days ago"); the more recent, the better.
    fn recent(iso: Option<&str>) -> Self {
        match iso {
            Some(iso) => Self {
                text: relative(iso),
                score: timestamp(iso),
                dot: None,
            },
            None => Self::missing(),
        }
    }

    fn missing() -> Self {
        Self::plain("—")
    }
}

struct Row {
    label: &'static str,
    cells: Vec<Cell>,
}

fn timestamp(iso: &str) -> Option<f64> {
    iso.parse::<DateTime<Utc>>()
        .ok()
        .map(|t| t.timestamp() as f64)
}

/// Main language with its percentage, from the (already sorted) language list.
fn main_language(langs: &[(String, u64)]) -> Cell {
    let total: u64 = langs.iter().map(|l| l.1).sum();
    match langs.first() {
        Some((name, n)) if total > 0 => Cell {
            text: format!("{name} {:.0}%", *n as f64 * 100.0 / total as f64),
            score: None,
            dot: Some(lang_rgb(name)),
        },
        _ => Cell::missing(),
    }
}

pub fn print_repos(reports: &[Report]) {
    let headers: Vec<(String, String)> = reports
        .iter()
        .map(|r| {
            let (owner, name) = r
                .repo
                .full_name
                .split_once('/')
                .unwrap_or(("", &r.repo.full_name));
            (format!("{owner}/"), name.to_string())
        })
        .collect();

    let row = |label, f: &dyn Fn(&Report) -> Cell| Row {
        label,
        cells: reports.iter().map(f).collect(),
    };
    let mut rows = vec![
        row("Stars", &|r| Cell::number(r.repo.stargazers_count)),
        row("Forks", &|r| Cell::number(r.repo.forks_count)),
        row("Watchers", &|r| {
            r.repo
                .subscribers_count
                .map_or_else(Cell::missing, Cell::number)
        }),
        // open_issues_count includes PRs; subtract them when we know how many there are.
        row("Open issues", &|r| match r.open_prs {
            Some(prs) => Cell::plain(human(r.repo.open_issues_count.saturating_sub(prs))),
            None => Cell::plain(format!("{} (incl. PRs)", human(r.repo.open_issues_count))),
        }),
        row("Open PRs", &|r| {
            r.open_prs
                .map_or_else(Cell::missing, |n| Cell::plain(human(n)))
        }),
        row("Last push", &|r| Cell::recent(r.repo.pushed_at.as_deref())),
        row("Last release", &|r| match &r.latest_release {
            Some(rel) => {
                let when = rel.published_at.as_deref();
                let age = when.map(relative).unwrap_or_default();
                Cell {
                    text: format!("{} · {age}", rel.tag_name),
                    score: when.and_then(timestamp),
                    dot: None,
                }
            }
            None => Cell::missing(),
        }),
        row("Created", &|r| Cell::plain(month_year(&r.repo.created_at))),
        row("Language", &|r| main_language(&r.languages)),
        row("License", &|r| {
            let l = r.repo.license.as_ref();
            Cell::plain(
                l.and_then(|l| l.spdx_id.clone().filter(|s| s != "NOASSERTION"))
                    .unwrap_or_else(|| "—".into()),
            )
        }),
        row("Size", &|r| {
            let mb = r.repo.size as f64 / 1024.0;
            Cell::plain(if mb >= 1024.0 {
                format!("{:.1} GB", mb / 1024.0)
            } else {
                format!("{mb:.1} MB")
            })
        }),
    ];
    // The status row only appears if some repo is archived or a fork.
    if reports.iter().any(|r| r.repo.archived || r.repo.fork) {
        rows.push(row("Status", &|r| {
            Cell::plain(match (r.repo.archived, r.repo.fork) {
                (true, _) => "archived",
                (_, true) => "fork",
                _ => "active",
            })
        }));
    }
    print_table(&headers, &rows);
}

pub fn print_users(reports: &[UserReport]) {
    let headers: Vec<(String, String)> = reports
        .iter()
        .map(|r| {
            (
                r.user.name.clone().unwrap_or_default(),
                r.user.login.clone(),
            )
        })
        .collect();

    let row = |label, f: &dyn Fn(&UserReport) -> Cell| Row {
        label,
        cells: reports.iter().map(f).collect(),
    };
    let opt = |o: &Option<String>| match o.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Cell::plain(s),
        None => Cell::missing(),
    };
    let rows = vec![
        row("Followers", &|r| Cell::number(r.user.followers)),
        row("Public repos", &|r| Cell::number(r.user.public_repos)),
        row("Stars", &|r| Cell::number(r.total_stars)),
        row("Forks", &|r| Cell::number(r.total_forks)),
        row("Language", &|r| main_language(&r.languages)),
        row("Top repo", &|r| match r.repos.first() {
            Some(repo) => Cell::plain(format!("{} ★ {}", repo.name, human(repo.stargazers_count))),
            None => Cell::missing(),
        }),
        row("Company", &|r| opt(&r.user.company)),
        row("Location", &|r| opt(&r.user.location)),
        row("Joined", &|r| Cell::plain(month_year(&r.user.created_at))),
    ];
    print_table(&headers, &rows);
}

/// Prints the table. If the columns don't fit in the terminal, they are split
/// into blocks that do, one below the other.
fn print_table(headers: &[(String, String)], rows: &[Row]) {
    let (ar, ag, ab) = DEFAULT_ACCENT;
    let label_w = rows.iter().map(|r| r.label.len()).max().unwrap_or(0) + 2;

    // Each column is as wide as its longest content, up to a limit.
    let widths: Vec<usize> = (0..headers.len())
        .map(|i| {
            let (sub, name) = &headers[i];
            let cells = rows.iter().map(|r| {
                r.cells[i].text.chars().count() + if r.cells[i].dot.is_some() { 2 } else { 0 }
            });
            cells
                .chain([sub.chars().count(), name.chars().count()])
                .max()
                .unwrap_or(0)
                .min(MAX_COL_W)
        })
        .collect();

    // Best value per row: only when there are at least two comparable values and they differ.
    let best: Vec<Vec<bool>> = rows
        .iter()
        .map(|r| {
            let scores: Vec<f64> = r.cells.iter().filter_map(|c| c.score).collect();
            let max = scores.iter().cloned().fold(f64::MIN, f64::max);
            let min = scores.iter().cloned().fold(f64::MAX, f64::min);
            let useful = scores.len() >= 2 && max > min;
            r.cells
                .iter()
                .map(|c| useful && c.score == Some(max))
                .collect()
        })
        .collect();

    // Split the columns into blocks that fit the terminal width.
    let available = crate::term::width().saturating_sub(MARGIN.len() + label_w);
    let mut chunks: Vec<Vec<usize>> = vec![vec![]];
    let mut used = 0;
    for (i, w) in widths.iter().enumerate() {
        let need = w + COL_GAP;
        if used + need > available && !chunks.last().unwrap().is_empty() {
            chunks.push(vec![]);
            used = 0;
        }
        chunks.last_mut().unwrap().push(i);
        used += need;
    }

    for cols in &chunks {
        println!();
        let pad = " ".repeat(label_w);
        // Two-line header: owner (or real name) on top, repo (or login) below.
        let line = |f: &dyn Fn(usize, usize) -> String| {
            cols.iter().map(|&i| f(i, widths[i])).collect::<String>()
        };
        println!(
            "{MARGIN}{pad}{}",
            line(&|i, w| cell_text(&headers[i].0, w).dimmed().to_string() + &" ".repeat(COL_GAP))
                .trim_end()
        );
        println!(
            "{MARGIN}{pad}{}",
            line(&|i, w| cell_text(&headers[i].1, w)
                .truecolor(ar, ag, ab)
                .bold()
                .to_string()
                + &" ".repeat(COL_GAP))
            .trim_end()
        );
        println!(
            "{MARGIN}{pad}{}",
            line(&|_, w| "─".repeat(w).dimmed().to_string() + &" ".repeat(COL_GAP)).trim_end()
        );

        for (r, row) in rows.iter().enumerate() {
            let label = format!("{:<label_w$}", row.label)
                .truecolor(ar, ag, ab)
                .bold();
            let cells = line(&|i, w| {
                let cell = &row.cells[i];
                // Pad the plain text first; color is applied afterwards.
                let (dot, dot_w) = match cell.dot {
                    Some((dr, dg, db)) => (format!("{} ", "●".truecolor(dr, dg, db)), 2),
                    None => (String::new(), 0),
                };
                let text = cell_text(&cell.text, w - dot_w);
                let text = if best[r][i] {
                    text.green().bold().to_string()
                } else if cell.text == "—" {
                    text.dimmed().to_string()
                } else {
                    text
                };
                format!("{dot}{text}{}", " ".repeat(COL_GAP))
            });
            println!("{MARGIN}{label}{}", cells.trim_end());
        }
    }
    println!();
}

/// Text truncated and padded to `w` columns (no color).
fn cell_text(s: &str, w: usize) -> String {
    let s = truncate(s, w);
    let pad = w.saturating_sub(s.chars().count());
    format!("{s}{}", " ".repeat(pad))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_width_cells() {
        assert_eq!(cell_text("abc", 5), "abc  ");
        assert_eq!(cell_text("Ubicación", 9).chars().count(), 9);
        let cut = cell_text("tokio-1.53.1 · 2 months ago", 10);
        assert_eq!(cut.chars().count(), 10);
        assert!(cut.ends_with('…'));
    }
}
