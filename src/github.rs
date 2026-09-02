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

impl PrKey {
    /// Compares without building a key, so hot loops do not clone `repo`.
    pub fn matches(&self, pr: &Pr) -> bool {
        self.number == pr.number && self.repo == pr.repo
    }
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

/// Copies CI state onto the PRs it was fetched for; PRs without a result keep theirs.
pub fn apply_checks(prs: &mut [Pr], checks: &HashMap<PrKey, Checks>) {
    for pr in prs {
        if let Some(c) = checks.get(&pr.key()) {
            pr.checks = *c;
        }
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
    parse_list(&run_gh(&search_query(kind, PR_FIELDS))?)
}

/// Open PRs for the temporary demo, excluding company-owned repositories.
pub fn fetch_demo_list(kind: Kind) -> Result<Vec<Pr>> {
    let mut prs = fetch_list(kind)?;
    prs.retain(|pr| !belongs_to_org(&pr.repo, "toridori-inc"));
    Ok(prs)
}

fn belongs_to_org(repo: &str, org: &str) -> bool {
    repo.split_once('/')
        .is_some_and(|(owner, _)| owner.eq_ignore_ascii_case(org))
}

/// CI state for one list.
pub fn fetch_checks(kind: Kind) -> Result<HashMap<PrKey, Checks>> {
    parse_checks(&run_gh(&search_query(kind, CHECK_FIELDS))?)
}

fn parse_list(body: &[u8]) -> Result<Vec<Pr>> {
    let data: raw::Data<raw::Pr> = parse_response(body)?;
    Ok(data
        .search
        .nodes
        .into_iter()
        .flatten()
        .map(Pr::from)
        .collect())
}

fn parse_checks(body: &[u8]) -> Result<HashMap<PrKey, Checks>> {
    let data: raw::Data<raw::PrChecks> = parse_response(body)?;
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

/// Everything at once; used by `--json`. All six queries run in parallel, as in the TUI.
pub fn fetch_all() -> Result<Snapshot> {
    let fetched = std::thread::scope(|s| {
        let handles = Kind::ALL.map(|kind| {
            (
                s.spawn(move || fetch_list(kind)),
                s.spawn(move || fetch_checks(kind)),
            )
        });
        handles.map(|(list, checks)| (join(list), join(checks)))
    });
    let mut snapshot = Snapshot::default();
    for (kind, (prs, checks)) in Kind::ALL.into_iter().zip(fetched) {
        let mut prs = prs?;
        apply_checks(&mut prs, &checks?);
        *snapshot.get_mut(kind) = prs;
    }
    Ok(snapshot)
}

fn join<T>(handle: std::thread::ScopedJoinHandle<'_, T>) -> T {
    match handle.join() {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Runs a query through `gh`, returning the response body when it holds usable data.
///
/// `gh` exits non-zero whenever the response carries `errors`, even alongside
/// partial `data`, so the body is parsed before the exit status is trusted.
fn run_gh(query: &str) -> Result<Vec<u8>> {
    let out = Command::new("gh")
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .output()
        .context("failed to run `gh`; is GitHub CLI installed?")?;
    if out.status.success() || has_data(&out.stdout) {
        return Ok(out.stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let message = one_line(&stderr);
    if message.is_empty() {
        bail!("`gh` exited with {}", out.status);
    }
    bail!("{message}")
}

fn has_data(body: &[u8]) -> bool {
    serde_json::from_slice::<raw::Response<serde::de::IgnoredAny>>(body)
        .is_ok_and(|r| r.data.is_some())
}

/// The info line has one row, so multi-line `gh` output is joined.
fn one_line(text: &str) -> String {
    text.lines()
        .map(|l| l.trim().trim_start_matches("gh: ").trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Partial results win over `errors`: a failing node is dropped, the rest still show.
fn parse_response<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T> {
    let resp: raw::Response<T> =
        serde_json::from_slice(body).context("unexpected GraphQL response")?;
    match (resp.data, resp.errors) {
        (Some(data), _) => Ok(data),
        (None, Some(errors)) if !errors.is_empty() => {
            let msgs: Vec<_> = errors.into_iter().map(|e| e.message).collect();
            bail!("{}", msgs.join("; "))
        }
        (None, _) => bail!("GraphQL response has no data"),
    }
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

impl From<raw::ReviewDecision> for Review {
    fn from(decision: raw::ReviewDecision) -> Self {
        match decision {
            raw::ReviewDecision::Approved => Review::Approved,
            raw::ReviewDecision::ChangesRequested => Review::ChangesRequested,
            raw::ReviewDecision::ReviewRequired => Review::Pending,
            raw::ReviewDecision::Other => Review::None,
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
            review: r.review_decision.map_or(Review::None, Review::from),
        }
    }
}

/// Wire shapes of the GraphQL responses; unknown enum values map to `Other`.
mod raw {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct Response<T> {
        pub data: Option<T>,
        pub errors: Option<Vec<GraphqlError>>,
    }

    #[derive(Debug, Deserialize)]
    pub struct GraphqlError {
        pub message: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct Data<T> {
        pub search: Search<T>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Search<T> {
        pub nodes: Vec<Option<T>>,
    }

    #[derive(Debug, Deserialize)]
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

    #[derive(Debug, Deserialize, Clone, Copy)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
    pub enum ReviewDecision {
        Approved,
        ChangesRequested,
        ReviewRequired,
        #[serde(other)]
        Other,
    }

    #[derive(Debug, Deserialize)]
    pub struct PrChecks {
        pub number: u64,
        pub repository: Repository,
        pub commits: Commits,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Repository {
        pub name_with_owner: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct Author {
        pub login: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct Commits {
        pub nodes: Vec<CommitNode>,
    }

    #[derive(Debug, Deserialize)]
    pub struct CommitNode {
        pub commit: Commit,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Commit {
        pub status_check_rollup: Option<Rollup>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Rollup {
        pub state: RollupState,
    }

    #[derive(Debug, Deserialize, Clone, Copy)]
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

    #[test]
    fn tab_order_matches_list_index() {
        for (i, kind) in Kind::ALL.into_iter().enumerate() {
            assert_eq!(kind.index(), i, "{kind:?}");
        }
    }

    #[test]
    fn search_query_carries_filter_and_page_size() {
        let q = search_query(Kind::ReviewRequested, PR_FIELDS);
        assert!(q.contains("review-requested:@me"), "{q}");
        assert!(q.contains("first: 100"), "{q}");
        assert!(q.contains("reviewDecision"), "{q}");
    }

    #[test]
    fn demo_filter_matches_only_the_excluded_org_owner() {
        assert!(belongs_to_org("toridori-inc/checkout", "toridori-inc"));
        assert!(belongs_to_org("TORIDORI-INC/checkout", "toridori-inc"));
        assert!(!belongs_to_org(
            "toridori-incubator/checkout",
            "toridori-inc"
        ));
        assert!(!belongs_to_org("other/toridori-inc", "toridori-inc"));
    }

    #[test]
    fn errors_without_data_become_one_message() {
        let body = br#"{"data":null,"errors":[{"message":"a"},{"message":"b"}]}"#;
        assert_eq!(
            parse_list(body)
                .expect_err("missing GraphQL data should return the reported errors")
                .to_string(),
            "a; b"
        );
    }

    #[test]
    fn partial_data_beats_errors() {
        let body = br#"{"data":{"search":{"nodes":[null]}},"errors":[{"message":"x"}]}"#;
        assert!(parse_list(body).is_ok_and(|prs| prs.is_empty()));
        assert!(has_data(body));
        assert!(!has_data(br#"{"data":null,"errors":[]}"#));
        assert!(!has_data(b"not json"));
    }

    #[test]
    fn gh_stderr_is_flattened_to_one_line() {
        assert_eq!(one_line("gh: first\n  second\n\n"), "first; second");
    }

    #[test]
    fn null_nodes_and_missing_author_are_tolerated() {
        let body = br#"{"data":{"search":{"nodes":[null,{
            "number":7,"title":"t","url":"u","isDraft":false,
            "baseRefName":"main","headRefName":"x",
            "repository":{"nameWithOwner":"o/r"},"author":null,
            "reviewDecision":"SOMETHING_NEW"}]}}}"#;
        let prs = match parse_list(body) {
            Ok(prs) => prs,
            Err(error) => {
                panic!("valid pull request fixture should parse: {error}");
            }
        };
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
        let body = br#"{"data":{"search":{"nodes":[
            {"number":1,"repository":{"nameWithOwner":"o/r"},
             "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"ERROR"}}}]}},
            {"number":2,"repository":{"nameWithOwner":"o/r"},
             "commits":{"nodes":[{"commit":{"statusCheckRollup":null}}]}},
            {"number":3,"repository":{"nameWithOwner":"o/r"},
             "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"EXPECTED"}}}]}}
        ]}}}"#;
        let checks = match parse_checks(body) {
            Ok(checks) => checks,
            Err(error) => {
                panic!("valid checks fixture should parse: {error}");
            }
        };
        let key = |n| PrKey {
            repo: "o/r".into(),
            number: n,
        };
        assert_eq!(checks[&key(1)], Checks::Failure);
        assert_eq!(checks[&key(2)], Checks::None);
        assert_eq!(checks[&key(3)], Checks::Pending);
    }
}
