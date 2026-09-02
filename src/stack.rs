use crate::github::Pr;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Row {
    pub pr: Pr,
    pub depth: usize,
}

/// Orders PRs as stacks: a PR whose base branch is another PR's head branch
/// in the same repository is listed right after its parent, indented.
pub fn arrange(prs: &[Pr]) -> Vec<Row> {
    let by_head: HashMap<(&str, &str), usize> = prs
        .iter()
        .enumerate()
        .map(|(i, p)| ((p.repo.as_str(), p.head_ref.as_str()), i))
        .collect();

    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut roots = Vec::new();
    for (i, p) in prs.iter().enumerate() {
        match by_head.get(&(p.repo.as_str(), p.base_ref.as_str())) {
            Some(&parent) if parent != i => children.entry(parent).or_default().push(i),
            _ => roots.push(i),
        }
    }

    let mut rows = Vec::with_capacity(prs.len());
    let mut seen = HashSet::new();
    let mut stack: Vec<(usize, usize)> = roots.iter().rev().map(|&i| (i, 0)).collect();
    while let Some((i, depth)) = stack.pop() {
        if !seen.insert(i) {
            continue;
        }
        rows.push(Row {
            pr: prs[i].clone(),
            depth,
        });
        if let Some(kids) = children.get(&i) {
            stack.extend(kids.iter().rev().map(|&k| (k, depth + 1)));
        }
    }
    // Cycles (A based on B, B based on A) never reach a root; emit them flat.
    for (i, p) in prs.iter().enumerate() {
        if seen.insert(i) {
            rows.push(Row {
                pr: p.clone(),
                depth: 0,
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Checks, Review};

    fn pr(repo: &str, n: u64, base: &str, head: &str) -> Pr {
        Pr {
            repo: repo.into(),
            number: n,
            title: format!("pr {n}"),
            url: String::new(),
            author: String::new(),
            is_draft: false,
            base_ref: base.into(),
            head_ref: head.into(),
            checks: Checks::None,
            review: Review::None,
        }
    }

    fn shape(rows: &[Row]) -> Vec<(u64, usize)> {
        rows.iter().map(|r| (r.pr.number, r.depth)).collect()
    }

    #[test]
    fn nests_stacked_prs_under_their_base() {
        let prs = vec![
            pr("o/r", 3, "feat-b", "feat-c"),
            pr("o/r", 1, "main", "feat-a"),
            pr("o/r", 2, "feat-a", "feat-b"),
            pr("o/r", 4, "main", "other"),
        ];
        assert_eq!(shape(&arrange(&prs)), vec![(1, 0), (2, 1), (3, 2), (4, 0)]);
    }

    #[test]
    fn same_branch_names_in_different_repos_do_not_link() {
        let prs = vec![pr("o/a", 1, "main", "x"), pr("o/b", 2, "x", "y")];
        assert_eq!(shape(&arrange(&prs)), vec![(1, 0), (2, 0)]);
    }

    #[test]
    fn cycles_are_still_listed() {
        let prs = vec![pr("o/r", 1, "b", "a"), pr("o/r", 2, "a", "b")];
        assert_eq!(shape(&arrange(&prs)), vec![(1, 0), (2, 0)]);
    }
}
