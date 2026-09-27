use crate::cache::Cache;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const API: &str = "https://api.github.com";

#[derive(Debug, Deserialize, Serialize)]
pub struct Repo {
    pub full_name: String,
    pub owner: Owner,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub html_url: String,
    pub stargazers_count: u64,
    pub forks_count: u64,
    pub subscribers_count: Option<u64>,
    pub open_issues_count: u64,
    pub size: u64,
    pub default_branch: String,
    pub created_at: String,
    pub pushed_at: Option<String>,
    pub archived: bool,
    pub fork: bool,
    #[serde(default)]
    pub topics: Vec<String>,
    pub license: Option<License>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Owner {
    pub login: String,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct License {
    pub spdx_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Contributor {
    pub login: String,
    pub contributions: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Release {
    pub tag_name: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchCount {
    total_count: u64,
}

/// Everything we know about a repo, ready to render or dump as JSON.
#[derive(Debug, Serialize)]
pub struct Report {
    pub repo: Repo,
    pub languages: Vec<(String, u64)>,
    pub contributors: Vec<Contributor>,
    pub latest_release: Option<Release>,
    pub open_prs: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub login: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub blog: Option<String>,
    pub twitter_username: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
    pub followers: u64,
    pub following: u64,
    pub public_repos: u64,
    pub public_gists: u64,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserRepo {
    pub name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stargazers_count: u64,
    pub forks_count: u64,
    pub fork: bool,
    pub pushed_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventRepo {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub repo: EventRepo,
    pub created_at: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub date: String,
    pub contribution_count: u64,
    /// NONE, FIRST_QUARTILE … FOURTH_QUARTILE: the intensity GitHub uses.
    pub contribution_level: String,
    /// 0 = Sunday … 6 = Saturday.
    pub weekday: u8,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Week {
    pub contribution_days: Vec<Day>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub total_contributions: u64,
    pub weeks: Vec<Week>,
}

/// Contribution calendar for the last year and its breakdown by type.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributions {
    pub total_commit_contributions: u64,
    pub total_issue_contributions: u64,
    pub total_pull_request_contributions: u64,
    pub total_pull_request_review_contributions: u64,
    pub contribution_calendar: Calendar,
}

const CONTRIBUTIONS_QUERY: &str = "query($login: String!) {
  user(login: $login) {
    contributionsCollection {
      totalCommitContributions
      totalIssueContributions
      totalPullRequestContributions
      totalPullRequestReviewContributions
      contributionCalendar {
        totalContributions
        weeks { contributionDays { date contributionCount contributionLevel weekday } }
      }
    }
  }
}";

/// Optional profile sections, each enabled by its own flag.
#[derive(Clone, Copy, Default)]
pub struct Sections {
    pub panel: bool,
    pub repos: bool,
    pub activity: bool,
}

#[derive(Debug, Serialize)]
pub struct UserReport {
    pub user: User,
    /// Own repos (no forks), sorted by stars.
    pub repos: Vec<UserRepo>,
    pub total_stars: u64,
    pub total_forks: u64,
    /// Main language → number of repos using it.
    pub languages: Vec<(String, u64)>,
    pub events: Vec<Event>,
    /// Only with a token (GraphQL requires one) and only for users, not organizations.
    pub contributions: Option<Contributions>,
}

pub struct GitHub {
    client: Client,
    token: Option<String>,
    cache: Option<Cache>,
}

impl GitHub {
    pub fn new(token: Option<String>, cache: Option<Cache>) -> Result<Self> {
        let client = Client::builder()
            .user_agent(concat!("volgit/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            token,
            cache,
        })
    }

    /// GET from the API. Returns Ok(None) on 404 (e.g. a repo with no releases).
    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        let url = format!("{API}{path}");
        // The cache stores the raw JSON, or "null" for 404s, so it is read back as
        // Option<T>. If it can't be parsed (e.g. an old format), go to the network.
        if let Some(body) = self.cache.as_ref().and_then(|c| c.get(&url))
            && let Ok(value) = serde_json::from_slice::<Option<T>>(&body)
        {
            return Ok(value);
        }

        let mut req = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github+json");
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let resp = req
            .send()
            .with_context(|| format!("network error on {path}"))?;
        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            self.store(&url, b"null");
            return Ok(None);
        }
        if status.is_success() {
            let body = resp.bytes()?;
            let value = serde_json::from_slice(&body)?;
            self.store(&url, &body);
            return Ok(Some(value));
        }

        // A 403 isn't always the rate limit: GitHub also uses it e.g. when the
        // contributor list is too large. It's only the rate limit if the header
        // says no requests are left (or on a 429).
        let header =
            |name: &str| -> Option<i64> { resp.headers().get(name)?.to_str().ok()?.parse().ok() };
        let remaining = header("x-ratelimit-remaining");
        let reset = header("x-ratelimit-reset");
        if status == StatusCode::TOO_MANY_REQUESTS || remaining == Some(0) {
            let when = reset
                .and_then(|ts| DateTime::from_timestamp(ts, 0))
                .map(|t| format!(" (resets at {})", t.with_timezone(&Local).format("%H:%M")))
                .unwrap_or_default();
            let hint = if self.token.is_none() {
                "; export GITHUB_TOKEN to raise it to 5000/hour"
            } else {
                ""
            };
            bail!("GitHub rate limit exceeded{when}{hint}");
        }
        let msg = resp
            .json::<serde_json::Value>()
            .ok()
            .and_then(|v| v["message"].as_str().map(String::from))
            .unwrap_or_default();
        bail!("GitHub returned {status} on {path}: {msg}")
    }

    fn store(&self, key: &str, body: &[u8]) {
        if let Some(c) = &self.cache {
            c.put(key, body);
        }
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    /// Contribution calendar through GraphQL. None without a token, for
    /// organizations or on failure: it's an optional section.
    pub fn contributions(&self, login: &str) -> Option<Contributions> {
        let token = self.token.as_ref()?;
        // GraphQL uses POST, so the cache key can't be the URL.
        let key = format!("graphql:contributions:{login}");
        if let Some(body) = self.cache.as_ref().and_then(|c| c.get(&key))
            && let Ok(c) = serde_json::from_slice(&body)
        {
            return Some(c);
        }
        let body =
            serde_json::json!({ "query": CONTRIBUTIONS_QUERY, "variables": { "login": login } });
        let resp = self
            .client
            .post(format!("{API}/graphql"))
            .bearer_auth(token)
            .json(&body)
            .send()
            .ok()?;
        let mut json: serde_json::Value = resp.json().ok()?;
        let collection = json
            .pointer_mut("/data/user/contributionsCollection")?
            .take();
        let contributions: Contributions = serde_json::from_value(collection).ok()?;
        if let Ok(bytes) = serde_json::to_vec(&contributions) {
            self.store(&key, &bytes);
        }
        Some(contributions)
    }

    /// Downloads an image (avatar). Failures are ignored: the avatar is optional.
    pub fn download(&self, url: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.cache.as_ref().and_then(|c| c.get(url)) {
            return Some(bytes);
        }
        let resp = self.client.get(url).send().ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let bytes = resp.bytes().ok()?.to_vec();
        self.store(url, &bytes);
        Some(bytes)
    }

    pub fn report(&self, owner: &str, name: &str, top: usize) -> Result<Report> {
        let base = format!("/repos/{owner}/{name}");
        let repo: Repo = self
            .get(&base)?
            .with_context(|| format!("repo {owner}/{name} not found (or it is private)"))?;

        let langs: HashMap<String, u64> =
            self.get(&format!("{base}/languages"))?.unwrap_or_default();
        let mut languages: Vec<_> = langs.into_iter().collect();
        languages.sort_by_key(|l| std::cmp::Reverse(l.1));

        // For huge repos (e.g. torvalds/linux) GitHub refuses to list contributors;
        // the section is simply left out. With top = 0 (comparison mode doesn't
        // use them) they aren't requested at all.
        let contributors = if top == 0 {
            vec![]
        } else {
            self
                // The API returns at most 100 per page.
                .get(&format!("{base}/contributors?per_page={}", top.min(100)))
                .ok()
                .flatten()
                .unwrap_or_default()
        };
        let latest_release = self.get(&format!("{base}/releases/latest"))?;

        // Search has a stricter rate limit; if it fails, the report still works.
        let open_prs = self
            .get::<SearchCount>(&format!(
                "/search/issues?q=repo:{owner}/{name}+type:pr+state:open&per_page=1"
            ))
            .ok()
            .flatten()
            .map(|s| s.total_count);

        Ok(Report {
            repo,
            languages,
            contributors,
            latest_release,
            open_prs,
        })
    }

    pub fn user_report(&self, login: &str, sections: Sections) -> Result<UserReport> {
        let user: User = self
            .get(&format!("/users/{login}"))?
            .with_context(|| format!("user {login} not found"))?;

        // At most 5 pages (500 repos) to avoid burning through the rate limit.
        let mut repos: Vec<UserRepo> = vec![];
        let pages = user.public_repos.div_ceil(100).clamp(1, 5);
        for page in 1..=pages {
            let batch: Vec<UserRepo> = self
                .get(&format!(
                    "/users/{login}/repos?per_page=100&type=owner&page={page}"
                ))?
                .unwrap_or_default();
            repos.extend(batch);
        }
        repos.retain(|r| !r.fork);
        repos.sort_by_key(|r| std::cmp::Reverse(r.stargazers_count));

        let total_stars = repos.iter().map(|r| r.stargazers_count).sum();
        let total_forks = repos.iter().map(|r| r.forks_count).sum();

        let mut langs: HashMap<String, u64> = HashMap::new();
        for lang in repos.iter().filter_map(|r| r.language.clone()) {
            *langs.entry(lang).or_default() += 1;
        }
        let mut languages: Vec<_> = langs.into_iter().collect();
        languages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        // Activity and the contribution graph are only fetched when shown, to save
        // requests. (Repos are always fetched: stars and languages come from them.)
        let events = if sections.activity {
            self.get(&format!("/users/{login}/events/public?per_page=30"))?
                .unwrap_or_default()
        } else {
            vec![]
        };
        let contributions = (sections.panel && user.kind == "User")
            .then(|| self.contributions(login))
            .flatten();

        Ok(UserReport {
            user,
            repos,
            total_stars,
            total_forks,
            languages,
            events,
            contributions,
        })
    }
}
