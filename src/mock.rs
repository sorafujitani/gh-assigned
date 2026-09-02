use crate::github::{Checks, Kind, Pr, Review, Snapshot};

/// A deterministic fixture for previewing the client without contacting GitHub.
pub fn snapshot() -> Snapshot {
    Snapshot {
        lists: Kind::ALL.map(list),
    }
}

pub fn list(kind: Kind) -> Vec<Pr> {
    match kind {
        Kind::Mine => mine(),
        Kind::ReviewRequested => review_requested(),
        Kind::Assigned => assigned(),
    }
}

fn mine() -> Vec<Pr> {
    vec![
        pr(
            "northstar/checkout-service",
            1842,
            "Add idempotency keys to payment capture",
            "alex.morgan",
            state(
                "main",
                "feat/payment-idempotency",
                false,
                Checks::Success,
                Review::Approved,
            ),
        ),
        pr(
            "northstar/checkout-service",
            1843,
            "Cover payment retries with gateway contract tests",
            "alex.morgan",
            state(
                "feat/payment-idempotency",
                "feat/payment-idempotency-tests",
                false,
                Checks::Pending,
                Review::Pending,
            ),
        ),
        pr(
            "northstar/checkout-service",
            1844,
            "Document retry semantics for capture endpoints",
            "alex.morgan",
            state(
                "feat/payment-idempotency-tests",
                "feat/payment-idempotency-docs",
                true,
                Checks::None,
                Review::Pending,
            ),
        ),
        pr(
            "northstar/account-console",
            932,
            "Preserve the active organization after session refresh",
            "alex.morgan",
            state(
                "main",
                "feat/session-refresh",
                false,
                Checks::Success,
                Review::Approved,
            ),
        ),
        pr(
            "meridian/data-platform",
            617,
            "Backfill invoice dimensions without locking the ledger",
            "alex.morgan",
            state(
                "main",
                "fix/invoice-backfill",
                true,
                Checks::Pending,
                Review::None,
            ),
        ),
        pr(
            "meridian/edge-gateway",
            1205,
            "Add bounded retries for transient upstream failures",
            "alex.morgan",
            state(
                "main",
                "feat/upstream-retries",
                false,
                Checks::Failure,
                Review::ChangesRequested,
            ),
        ),
        pr(
            "northstar/design-system",
            288,
            "Add reduced-motion variants for toast notifications",
            "alex.morgan",
            state(
                "main",
                "feat/reduced-motion-toasts",
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
            "northstar/checkout-service",
            1851,
            "Move webhook signature verification into the shared gateway",
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
            "northstar/checkout-service",
            1852,
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
            "meridian/edge-gateway",
            1210,
            "Retry upstream connections with bounded exponential backoff",
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
            "northstar/mobile-app",
            441,
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
            "meridian/analytics-api",
            139,
            "Expose cohort retention windows in the reporting endpoint",
            "daniel.kim",
            state(
                "main",
                "feat/cohort-retention",
                false,
                Checks::None,
                Review::Pending,
            ),
        ),
        pr(
            "northstar/design-system",
            291,
            "Align modal focus trapping with the keyboard specification",
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
            "meridian/warehouse-jobs",
            326,
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
            "northstar/checkout-service",
            1860,
            "Document the refund reconciliation runbook",
            "liam.nguyen",
            state(
                "main",
                "docs/refund-reconciliation",
                false,
                Checks::Success,
                Review::None,
            ),
        ),
        pr(
            "meridian/edge-gateway",
            1214,
            "Remove legacy X-Forwarded-For parsing",
            "jordan.lee",
            state(
                "main",
                "cleanup/forwarded-for",
                false,
                Checks::Success,
                Review::Pending,
            ),
        ),
        pr(
            "meridian/edge-gateway",
            1215,
            "Add coverage for forwarded header normalization",
            "jordan.lee",
            state(
                "cleanup/forwarded-for",
                "cleanup/forwarded-for-tests",
                false,
                Checks::Success,
                Review::Pending,
            ),
        ),
        pr(
            "northstar/account-console",
            948,
            "Keep organization filters when switching workspaces",
            "sophie.martin",
            state(
                "main",
                "feat/persistent-org-filter",
                false,
                Checks::Failure,
                Review::ChangesRequested,
            ),
        ),
        pr(
            "meridian/data-platform",
            633,
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
            "northstar/mobile-sdk",
            88,
            "Add offline upload retries for background sync",
            "isabella.rossi",
            state(
                "main",
                "feat/offline-upload-retries",
                true,
                Checks::None,
                Review::None,
            ),
        ),
        pr(
            "northstar/design-system",
            299,
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

        assert_eq!(prs[1].base_ref, prs[0].head_ref);
        assert_eq!(prs[2].base_ref, prs[1].head_ref);
    }
}
