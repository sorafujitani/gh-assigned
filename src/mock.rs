use crate::github::{Checks, Kind, Pr, Review, Snapshot};

/// Fixed data for recordings: a frozen copy of real public PRs followed by invented ones.
pub fn snapshot() -> Snapshot {
    Snapshot {
        lists: Kind::ALL.map(list),
    }
}

fn list(kind: Kind) -> Vec<Pr> {
    let (mut live, mut invented) = match kind {
        Kind::Mine => (live_mine(), mine()),
        Kind::ReviewRequested => (live_review_requested(), review_requested()),
        Kind::Assigned => (live_assigned(), assigned()),
    };
    live.append(&mut invented);
    live
}

fn live_mine() -> Vec<Pr> {
    vec![
        pr(
            "sorafujitani/kaguya",
            8,
            "Cut runtime RSS with leaner buffers, caches, and atlas caps",
            "sorafujitani",
            state(
                "main",
                "cursor/reduce-memory-usage-06aa",
                true,
                Checks::Failure,
                Review::None,
            ),
        ),
        pr(
            "oxc-project/oxc",
            22825,
            "feat(linter/jest/vitest): implement padding-around-before-all-blocks",
            "sorafujitani",
            state(
                "main",
                "492/padding-around-before-all-blocks",
                false,
                Checks::None,
                Review::None,
            ),
        ),
        pr(
            "oxc-project/oxc",
            23286,
            "feat(linter/jest/vitest): implement padding-around-expect-groups",
            "sorafujitani",
            state(
                "main",
                "492/padding-around-expect-groups",
                false,
                Checks::None,
                Review::None,
            ),
        ),
        pr(
            "xwmx/nb",
            418,
            "Auto-detect remote's default branch in `nb remote set`",
            "sorafujitani",
            state(
                "master",
                "feature/git-branch-remote-sync",
                false,
                Checks::Success,
                Review::None,
            ),
        ),
        pr(
            "sorafujitani/nobunaga",
            2,
            "Rust Prism extension, RuboCop parity docs, and CI",
            "sorafujitani",
            state(
                "main",
                "cursor/cloud-agent-1775329011757-j8slq",
                false,
                Checks::Success,
                Review::None,
            ),
        ),
        pr(
            "sorafujitani/rfmt",
            64,
            "feat: Add insta snapshot tests for formatter",
            "sorafujitani",
            state(
                "main",
                "feat/insta-snapshot-tests",
                false,
                Checks::Success,
                Review::None,
            ),
        ),
        pr(
            "sorafujitani/ruby-lsp-addon-template",
            1,
            "Create Ruby LSP custom addon implementation guide",
            "sorafujitani",
            state(
                "main",
                "claude/ruby-lsp-addon-guide-011CUiYCy2jceg9QdQB4jDK6",
                false,
                Checks::Success,
                Review::None,
            ),
        ),
    ]
}

fn live_review_requested() -> Vec<Pr> {
    vec![
        pr(
            "yamada-ui/yamada-ui",
            7609,
            "Added tip section for list components",
            "108yen",
            state(
                "main",
                "docs/list",
                false,
                Checks::Success,
                Review::ChangesRequested,
            ),
        ),
        pr(
            "yamada-ui/yamada-ui",
            6124,
            "feat(cli): add view command to inspect and download component files",
            "SahilJat",
            state(
                "v2.3",
                "feat/cli-view-command",
                false,
                Checks::Failure,
                Review::None,
            ),
        ),
    ]
}

fn live_assigned() -> Vec<Pr> {
    vec![]
}

const ME: &str = "sorafujitani";

fn mine() -> Vec<Pr> {
    vec![
        pr(
            "sorafujitani/rfmt",
            312,
            "Preserve trailing commas in multi-line match arms",
            ME,
            state(
                "main",
                "feat/match-arm-commas",
                false,
                Checks::Success,
                Review::Approved,
            ),
        ),
        pr(
            "sorafujitani/rfmt",
            313,
            "Cover match arm formatting with snapshot tests",
            ME,
            state(
                "feat/match-arm-commas",
                "feat/match-arm-commas-tests",
                false,
                Checks::Pending,
                Review::Pending,
            ),
        ),
        pr(
            "sorafujitani/rfmt",
            314,
            "Document the comma rules in the style guide",
            ME,
            state(
                "feat/match-arm-commas-tests",
                "feat/match-arm-commas-docs",
                true,
                Checks::None,
                Review::Pending,
            ),
        ),
        pr(
            "sorafujitani/ccsession",
            148,
            "Resume the most recent session when no id is given",
            ME,
            state(
                "main",
                "feat/resume-latest",
                false,
                Checks::Success,
                Review::Approved,
            ),
        ),
        pr(
            "sorafujitani/rt",
            97,
            "Stream task output instead of buffering it",
            ME,
            state(
                "main",
                "fix/stream-output",
                true,
                Checks::Pending,
                Review::None,
            ),
        ),
        pr(
            "oxc-project/oxc",
            9821,
            "linter: add bounded retries to the language server restart",
            ME,
            state(
                "main",
                "feat/lsp-restart-retries",
                false,
                Checks::Failure,
                Review::ChangesRequested,
            ),
        ),
        pr(
            "sorafujitani/sorafujitani.me",
            61,
            "Add reduced-motion variants for page transitions",
            ME,
            state(
                "main",
                "feat/reduced-motion",
                false,
                Checks::Success,
                Review::Pending,
            ),
        ),
    ]
}

fn review_requested() -> Vec<Pr> {
    vec![
        pr(
            "topi-log/topi-log",
            221,
            "Move webhook signature verification into the shared middleware",
            "maya.chen",
            state(
                "main",
                "feat/shared-webhook-verifier",
                false,
                Checks::Success,
                Review::Pending,
            ),
        ),
        pr(
            "topi-log/topi-log",
            222,
            "Reject replayed webhook deliveries after signature validation",
            "maya.chen",
            state(
                "feat/shared-webhook-verifier",
                "feat/webhook-replay-protection",
                false,
                Checks::Pending,
                Review::Pending,
            ),
        ),
        pr(
            "sorafujitani/pi",
            58,
            "Retry provider connections with bounded exponential backoff",
            "jordan.lee",
            state(
                "main",
                "feat/connection-retries",
                false,
                Checks::Failure,
                Review::ChangesRequested,
            ),
        ),
        pr(
            "sorafujitani/kaguya",
            34,
            "Support passkeys in the account recovery flow",
            "priya.shah",
            state(
                "main",
                "feat/passkey-recovery",
                false,
                Checks::Success,
                Review::Approved,
            ),
        ),
        pr(
            "sorafujitani/panopticon",
            79,
            "Expose retention windows in the reporting endpoint",
            "daniel.kim",
            state(
                "main",
                "feat/retention-windows",
                false,
                Checks::None,
                Review::Pending,
            ),
        ),
        pr(
            "yamada-ui/yamada-ui",
            4410,
            "fix(modal): align focus trapping with the keyboard specification",
            "elena.morales",
            state(
                "main",
                "fix/modal-focus-trap",
                false,
                Checks::Success,
                Review::Approved,
            ),
        ),
        pr(
            "sorafujitani/graphnote",
            126,
            "Make nightly exports resumable after a worker restart",
            "noah.wilson",
            state(
                "main",
                "feat/resumable-exports",
                false,
                Checks::Pending,
                Review::Pending,
            ),
        ),
    ]
}

fn assigned() -> Vec<Pr> {
    vec![
        pr(
            "sorafujitani/ccsession",
            151,
            "Document the session recovery runbook",
            "liam.nguyen",
            state(
                "main",
                "docs/session-recovery",
                false,
                Checks::Success,
                Review::None,
            ),
        ),
        pr(
            "sorafujitani/rt",
            101,
            "Remove legacy environment file parsing",
            "jordan.lee",
            state(
                "main",
                "cleanup/env-parsing",
                false,
                Checks::Success,
                Review::Pending,
            ),
        ),
        pr(
            "sorafujitani/rt",
            102,
            "Add coverage for environment normalization",
            "jordan.lee",
            state(
                "cleanup/env-parsing",
                "cleanup/env-parsing-tests",
                false,
                Checks::Success,
                Review::Pending,
            ),
        ),
        pr(
            "sorafujitani/kaguya",
            36,
            "Keep filters when switching workspaces",
            "sophie.martin",
            state(
                "main",
                "feat/persistent-filters",
                false,
                Checks::Failure,
                Review::ChangesRequested,
            ),
        ),
        pr(
            "sorafujitani/panopticon",
            82,
            "Add partition-pruning metrics to nightly exports",
            "omar.hassan",
            state(
                "main",
                "feat/partition-metrics",
                false,
                Checks::Pending,
                Review::Pending,
            ),
        ),
        pr(
            "honojs/hono",
            3877,
            "feat(jsx): add offline retries for streamed responses",
            "isabella.rossi",
            state(
                "main",
                "feat/stream-retries",
                true,
                Checks::None,
                Review::None,
            ),
        ),
        pr(
            "sorafujitani/sorafujitani.me",
            64,
            "Expose semantic colors for destructive actions",
            "ben.carter",
            state(
                "main",
                "feat/destructive-colors",
                false,
                Checks::Success,
                Review::Approved,
            ),
        ),
    ]
}

#[derive(Debug, Clone, Copy)]
struct FixtureState<'a> {
    base_ref: &'a str,
    head_ref: &'a str,
    is_draft: bool,
    checks: Checks,
    review: Review,
}

fn state<'a>(
    base_ref: &'a str,
    head_ref: &'a str,
    is_draft: bool,
    checks: Checks,
    review: Review,
) -> FixtureState<'a> {
    FixtureState {
        base_ref,
        head_ref,
        is_draft,
        checks,
        review,
    }
}

fn pr(repo: &str, number: u64, title: &str, author: &str, fixture: FixtureState<'_>) -> Pr {
    let FixtureState {
        base_ref,
        head_ref,
        is_draft,
        checks,
        review,
    } = fixture;
    Pr {
        repo: repo.into(),
        number,
        title: title.into(),
        url: format!("https://github.com/{repo}/pull/{number}"),
        author: author.into(),
        is_draft,
        base_ref: base_ref.into(),
        head_ref: head_ref.into(),
        checks,
        review,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tab_has_realistic_fixture_data() {
        let snapshot = snapshot();

        for kind in Kind::ALL {
            let prs = snapshot.get(kind);
            assert!(!prs.is_empty(), "{} should not be empty", kind.label());
            assert!(
                prs.iter()
                    .all(|pr| pr.url.starts_with("https://github.com/"))
            );
            assert!(prs.iter().all(|pr| !pr.title.is_empty()));
            assert!(prs.iter().all(|pr| !pr.author.is_empty()));
        }
    }

    #[test]
    fn fixture_contains_stacked_pull_requests() {
        let snapshot = snapshot();
        let prs = snapshot.get(Kind::Mine);
        let by_number = |n: u64| {
            prs.iter()
                .find(|pr| pr.number == n)
                .unwrap_or_else(|| panic!("fixture should contain #{n}"))
        };

        assert_eq!(by_number(313).base_ref, by_number(312).head_ref);
        assert_eq!(by_number(314).base_ref, by_number(313).head_ref);
    }
}
