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
    /// Tab order; `index` relies on the discriminants matching this order.
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

/// Identifies a PR across fetches: numbers are only unique within a repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrKey {
    pub repo: String,
    pub number: u64,
}

impl Pr {
    pub fn key(&self) -> PrKey {
        PrKey {
            repo: self.repo.clone(),
            number: self.number,
        }
    }

    pub fn short_repo(&self) -> &str {
        self.repo.rsplit('/').next().unwrap_or(&self.repo)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub lists: [Vec<Pr>; Kind::ALL.len()],
}

impl Snapshot {
    pub fn get(&self, kind: Kind) -> &[Pr] {
        &self.lists[kind.index()]
    }

    pub fn get_mut(&mut self, kind: Kind) -> &mut Vec<Pr> {
        &mut self.lists[kind.index()]
    }
}

/// GitHub's search API caps a page at 100 nodes; more open PRs than that are dropped.
const PAGE_SIZE: usize = 100;

const PR_FIELDS: &str = "number title url isDraft baseRefName headRefName repository { nameWithOwner } author { login } reviewDecision";

// Fetched separately so the list can render while this equally slow query is still running.
const CHECK_FIELDS: &str = "number repository { nameWithOwner } commits(last: 1) { nodes { commit { statusCheckRollup { state } } } }";

fn search_query(kind: Kind, fields: &str) -> String {
    format!(
        "query {{ search(query: \"is:pr is:open archived:false sort:updated-desc {}\", type: ISSUE, first: {PAGE_SIZE}) {{ nodes {{ ... on PullRequest {{ {fields} }} }} }} }}",
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

/// CI state for one list.
pub fn fetch_checks(kind: Kind) -> Result<HashMap<PrKey, Checks>> {
    let data: raw::Data<raw::PrChecks> = graphql(&search_query(kind, CHECK_FIELDS))?;
    Ok(data
        .search
        .nodes
        .into_iter()
        .flatten()
        .map(|n| {
            let key = PrKey {
                repo: n.repository.name_with_owner,
                number: n.number,
            };
            (key, Checks::from(&n.commits))
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
    parse_response(&out.stdout)
}

fn parse_response<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T> {
    let resp: raw::Response<T> =
        serde_json::from_slice(body).context("unexpected GraphQL response")?;
    if let Some(errors) = resp.errors {
        let msgs: Vec<_> = errors.into_iter().map(|e| e.message).collect();
        bail!("{}", msgs.join("; "));
    }
    resp.data.context("GraphQL response has no data")
}

impl From<&raw::Commits> for Checks {
    fn from(commits: &raw::Commits) -> Self {
        let state = commits
            .nodes
            .first()
            .and_then(|n| n.commit.status_check_rollup.as_ref())
            .map(|s| s.state);
        match state {
            Some(raw::RollupState::Success) => Checks::Success,
            Some(raw::RollupState::Failure | raw::RollupState::Error) => Checks::Failure,
            Some(raw::RollupState::Pending | raw::RollupState::Expected) => Checks::Pending,
            Some(raw::RollupState::Other) | None => Checks::None,
        }
    }
}

impl From<Option<raw::ReviewDecision>> for Review {
    fn from(decision: Option<raw::ReviewDecision>) -> Self {
        match decision {
            Some(raw::ReviewDecision::Approved) => Review::Approved,
            Some(raw::ReviewDecision::ChangesRequested) => Review::ChangesRequested,
            Some(raw::ReviewDecision::ReviewRequired) => Review::Pending,
            Some(raw::ReviewDecision::Other) | None => Review::None,
        }
    }
}

impl From<raw::Pr> for Pr {
    fn from(r: raw::Pr) -> Self {
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
            review: Review::from(r.review_decision),
        }
    }
}

/// Wire shapes of the GraphQL responses; unknown enum values map to `Other`.
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
        pub review_decision: Option<ReviewDecision>,
    }

    #[derive(Deserialize, Clone, Copy)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum ReviewDecision {
        Approved,
        ChangesRequested,
        ReviewRequired,
        #[serde(other)]
        Other,
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
        pub state: RollupState,
    }

    #[derive(Deserialize, Clone, Copy)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum RollupState {
        Success,
        Failure,
        Error,
        Pending,
        Expected,
        #[serde(other)]
        Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_prs(body: &str) -> Result<Vec<Pr>> {
        let data: raw::Data<raw::Pr> = parse_response(body.as_bytes())?;
        Ok(data
            .search
            .nodes
            .into_iter()
            .flatten()
            .map(Pr::from)
            .collect())
    }

    #[test]
    fn graphql_errors_become_one_message() {
        let body = r#"{"data":null,"errors":[{"message":"a"},{"message":"b"}]}"#;
        let err = parse_prs(body).unwrap_err();
        assert_eq!(err.to_string(), "a; b");
    }

    #[test]
    fn null_nodes_and_missing_author_are_tolerated() {
        let body = r#"{"data":{"search":{"nodes":[null,{
            "number":7,"title":"t","url":"u","isDraft":false,
            "baseRefName":"main","headRefName":"x",
            "repository":{"nameWithOwner":"o/r"},"author":null,
            "reviewDecision":"SOMETHING_NEW"}]}}}"#;
        let prs = parse_prs(body).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].author, "");
        assert_eq!(prs[0].review, Review::None);
        assert_eq!(
            prs[0].key(),
            PrKey {
                repo: "o/r".into(),
                number: 7
            }
        );
    }

    #[test]
    fn rollup_states_map_onto_checks() {
        let body = r#"{"data":{"search":{"nodes":[
            {"number":1,"repository":{"nameWithOwner":"o/r"},
             "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"ERROR"}}}]}},
            {"number":2,"repository":{"nameWithOwner":"o/r"},
             "commits":{"nodes":[{"commit":{"statusCheckRollup":null}}]}},
            {"number":3,"repository":{"nameWithOwner":"o/r"},
             "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"EXPECTED"}}}]}}
        ]}}}"#;
        let data: raw::Data<raw::PrChecks> = parse_response(body.as_bytes()).unwrap();
        let checks: Vec<Checks> = data
            .search
            .nodes
            .into_iter()
            .flatten()
            .map(|n| Checks::from(&n.commits))
            .collect();
        assert_eq!(checks, [Checks::Failure, Checks::None, Checks::Pending]);
    }
}
