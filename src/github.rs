use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Mine,
    ReviewRequested,
    Assigned,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Mine, Kind::ReviewRequested, Kind::Assigned];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Mine => "Mine",
            Kind::ReviewRequested => "Review requested",
            Kind::Assigned => "Assigned",
        }
    }

    pub fn next(self) -> Kind {
        Kind::ALL[(self.index() + 1) % Kind::ALL.len()]
    }

    pub fn prev(self) -> Kind {
        Kind::ALL[(self.index() + Kind::ALL.len() - 1) % Kind::ALL.len()]
    }

    fn search_filter(self) -> &'static str {
        match self {
            Kind::Mine => "author:@me",
            Kind::ReviewRequested => "review-requested:@me",
            Kind::Assigned => "assignee:@me",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Checks {
    Success,
    Failure,
    Pending,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Review {
    Approved,
    ChangesRequested,
    Pending,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pr {
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub author: String,
    pub is_draft: bool,
    pub base_ref: String,
    pub head_ref: String,
    pub checks: Checks,
    pub review: Review,
}

pub type PrKey = (String, u64);

impl Pr {
    pub fn key(&self) -> PrKey {
        (self.repo.clone(), self.number)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub lists: [Vec<Pr>; 3],
}

impl Snapshot {
    pub fn get(&self, kind: Kind) -> &[Pr] {
        &self.lists[kind.index()]
    }

    pub fn get_mut(&mut self, kind: Kind) -> &mut Vec<Pr> {
        &mut self.lists[kind.index()]
    }
}

const PR_FIELDS: &str = "number title url isDraft baseRefName headRefName repository { nameWithOwner } author { login } reviewDecision";

// Fetched separately so the list can render while this equally slow query is still running.
const CHECK_FIELDS: &str = "number repository { nameWithOwner } commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }";

fn search_query(kind: Kind, fields: &str) -> String {
    format!(
        "query {{ search(query: \"is:pr is:open archived:false sort:updated-desc {}\", type: ISSUE, first: 100) {{ nodes {{ ... on PullRequest {{ {fields} }} }} }} }}",
        kind.search_filter()
    )
}

/// Open PRs for one list, with `checks` left as `Checks::None`.
pub fn fetch_list(kind: Kind) -> Result<Vec<Pr>> {
    let data: raw::Data<raw::Pr> = graphql(&search_query(kind, PR_FIELDS))?;
    Ok(data
        .search
        .nodes
        .into_iter()
        .flatten()
        .map(Pr::from)
        .collect())
}

/// CI state for one list, keyed by (repo, number).
pub fn fetch_checks(kind: Kind) -> Result<HashMap<PrKey, Checks>> {
    let data: raw::Data<raw::PrChecks> = graphql(&search_query(kind, CHECK_FIELDS))?;
    Ok(data
        .search
        .nodes
        .into_iter()
        .flatten()
        .map(|n| {
            (
                (n.repository.name_with_owner, n.number),
                checks_from(&n.commits),
            )
        })
        .collect())
}

/// Everything at once; used by `--json`.
pub fn fetch_all() -> Result<Snapshot> {
    let mut snapshot = Snapshot::default();
    for kind in Kind::ALL {
        let mut prs = fetch_list(kind)?;
        let checks = fetch_checks(kind)?;
        for pr in &mut prs {
            if let Some(c) = checks.get(&pr.key()) {
                pr.checks = *c;
            }
        }
        *snapshot.get_mut(kind) = prs;
    }
    Ok(snapshot)
}

fn graphql<T: for<'de> Deserialize<'de>>(query: &str) -> Result<T> {
    let out = Command::new("gh")
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .output()
        .context("failed to run `gh`; is GitHub CLI installed?")?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let resp: raw::Response<T> =
        serde_json::from_slice(&out.stdout).context("unexpected GraphQL response")?;
    if let Some(errors) = resp.errors {
        let msgs: Vec<_> = errors.into_iter().map(|e| e.message).collect();
        bail!("{}", msgs.join("; "));
    }
    resp.data.context("GraphQL response has no data")
}

fn checks_from(commits: &raw::Commits) -> Checks {
    let state = commits
        .nodes
        .first()
        .and_then(|n| n.commit.status_check_rollup.as_ref())
        .map(|s| s.state.as_str());
    match state {
        Some("SUCCESS") => Checks::Success,
        Some("FAILURE" | "ERROR") => Checks::Failure,
        Some("PENDING" | "EXPECTED") => Checks::Pending,
        _ => Checks::None,
    }
}

impl From<raw::Pr> for Pr {
    fn from(r: raw::Pr) -> Self {
        let review = match r.review_decision.as_deref() {
            Some("APPROVED") => Review::Approved,
            Some("CHANGES_REQUESTED") => Review::ChangesRequested,
            Some("REVIEW_REQUIRED") => Review::Pending,
            _ => Review::None,
        };
        Pr {
            repo: r.repository.name_with_owner,
            number: r.number,
            title: r.title,
            url: r.url,
            author: r.author.map(|a| a.login).unwrap_or_default(),
            is_draft: r.is_draft,
            base_ref: r.base_ref_name,
            head_ref: r.head_ref_name,
            checks: Checks::None,
            review,
        }
    }
}

mod raw {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct Response<T> {
        pub data: Option<T>,
        pub errors: Option<Vec<GraphqlError>>,
    }

    #[derive(Deserialize)]
    pub struct GraphqlError {
        pub message: String,
    }

    #[derive(Deserialize)]
    pub struct Data<T> {
        pub search: Search<T>,
    }

    #[derive(Deserialize)]
    pub struct Search<T> {
        pub nodes: Vec<Option<T>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Pr {
        pub number: u64,
        pub title: String,
        pub url: String,
        pub is_draft: bool,
        pub base_ref_name: String,
        pub head_ref_name: String,
        pub repository: Repository,
        pub author: Option<Author>,
        pub review_decision: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct PrChecks {
        pub number: u64,
        pub repository: Repository,
        pub commits: Commits,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Repository {
        pub name_with_owner: String,
    }

    #[derive(Deserialize)]
    pub struct Author {
        pub login: String,
    }

    #[derive(Deserialize)]
    pub struct Commits {
        pub nodes: Vec<CommitNode>,
    }

    #[derive(Deserialize)]
    pub struct CommitNode {
        pub commit: Commit,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Commit {
        pub status_check_rollup: Option<Rollup>,
    }

    #[derive(Deserialize)]
    pub struct Rollup {
        pub state: String,
    }
}
