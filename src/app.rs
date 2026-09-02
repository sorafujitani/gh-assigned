use crate::github::{Checks, Kind, Pr, PrKey, Snapshot};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::widgets::ListState;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Fetching { showing_cache: bool },
    Fresh,
    Error(String),
}

pub struct App {
    snapshot: Snapshot,
    rows: Vec<crate::stack::Row>,
    haystacks: Vec<String>,
    pub filtered: Vec<usize>,
    pub tab: Kind,
    pub query: String,
    pub list_state: ListState,
    showing_cache: bool,
    pending: usize,
    generation: u64,
    error: Option<String>,
    notice: Option<Result<String, String>>,
    matcher: Matcher,
}

impl App {
    pub fn new(cached: Option<Snapshot>) -> Self {
        let showing_cache = cached.is_some();
        let mut app = App {
            snapshot: cached.unwrap_or_default(),
            rows: Vec::new(),
            haystacks: Vec::new(),
            filtered: Vec::new(),
            tab: Kind::Mine,
            query: String::new(),
            list_state: ListState::default(),
            showing_cache,
            pending: 0,
            generation: 0,
            error: None,
            notice: None,
            matcher: Matcher::new(Config::DEFAULT),
        };
        app.rebuild_rows();
        app
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn rows(&self) -> &[crate::stack::Row] {
        &self.rows
    }

    pub fn count(&self, kind: Kind) -> usize {
        self.snapshot.get(kind).len()
    }

    pub fn cursor(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    pub fn selected(&self) -> Option<&crate::stack::Row> {
        self.filtered.get(self.cursor()).map(|&i| &self.rows[i])
    }

    /// A one-shot message shown until the next key press, e.g. after copying.
    pub fn notify(&mut self, result: anyhow::Result<String>) {
        self.notice = Some(result.map_err(|e| e.to_string()));
    }

    pub fn clear_notice(&mut self) {
        self.notice = None;
    }

    pub fn notice(&self) -> Option<&Result<String, String>> {
        self.notice.as_ref()
    }

    pub fn status(&self) -> Status {
        if self.pending > 0 {
            return Status::Fetching {
                showing_cache: self.showing_cache,
            };
        }
        match &self.error {
            Some(e) => Status::Error(e.clone()),
            None => Status::Fresh,
        }
    }

    pub fn is_fetching(&self) -> bool {
        self.pending > 0
    }

    /// Starts a new fetch round; messages from earlier rounds are ignored.
    pub fn start_fetch(&mut self, steps: usize) -> u64 {
        self.generation += 1;
        self.pending = steps;
        self.error = None;
        self.generation
    }

    pub fn accepts(&self, generation: u64) -> bool {
        generation == self.generation
    }

    /// Replaces one list, carrying over known CI state until fresh checks arrive.
    pub fn set_list(&mut self, kind: Kind, mut prs: Vec<Pr>) {
        let known: HashMap<PrKey, Checks> = self
            .snapshot
            .get(kind)
            .iter()
            .map(|p| (p.key(), p.checks))
            .collect();
        for pr in &mut prs {
            if let Some(c) = known.get(&pr.key()) {
                pr.checks = *c;
            }
        }
        *self.snapshot.get_mut(kind) = prs;
        self.showing_cache = false;
        self.finish_step(kind);
    }

    pub fn set_checks(&mut self, kind: Kind, checks: HashMap<PrKey, Checks>) {
        for pr in self.snapshot.get_mut(kind) {
            if let Some(c) = checks.get(&pr.key()) {
                pr.checks = *c;
            }
        }
        self.finish_step(kind);
    }

    pub fn set_error(&mut self, kind: Kind, message: String) {
        self.error = Some(format!("{}: {message}", kind.label()));
        self.pending = self.pending.saturating_sub(1);
    }

    fn finish_step(&mut self, kind: Kind) {
        self.pending = self.pending.saturating_sub(1);
        if kind == self.tab {
            self.rebuild_rows();
        }
    }

    pub fn next_tab(&mut self) {
        self.tab = self.tab.next();
        self.rebuild_rows();
    }

    pub fn prev_tab(&mut self) {
        self.tab = self.tab.prev();
        self.rebuild_rows();
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            self.list_state.select(Some(0));
            return;
        }
        let max = self.filtered.len() as isize - 1;
        let next = (self.cursor() as isize + delta).clamp(0, max) as usize;
        self.list_state.select(Some(next));
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn pop_char(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn pop_word(&mut self) {
        let trimmed = self.query.trim_end();
        let cut = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
        self.query.truncate(cut);
        self.refilter();
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.refilter();
    }

    fn rebuild_rows(&mut self) {
        // Capture the selection before `rows` changes: `filtered` indexes the old rows.
        let previous = self.selected().map(|r| r.pr.key());
        self.rows = crate::stack::arrange(self.snapshot.get(self.tab));
        self.haystacks = self
            .rows
            .iter()
            .map(|r| {
                format!(
                    "{} #{} {} {}",
                    r.pr.repo, r.pr.number, r.pr.title, r.pr.author
                )
            })
            .collect();
        self.filtered.clear();
        self.refilter_keeping(previous);
    }

    fn refilter(&mut self) {
        let previous = self.selected().map(|r| r.pr.key());
        self.refilter_keeping(previous);
    }

    fn refilter_keeping(&mut self, previous: Option<PrKey>) {
        if self.query.is_empty() {
            self.filtered = (0..self.rows.len()).collect();
        } else {
            let pattern = Pattern::parse(&self.query, CaseMatching::Ignore, Normalization::Smart);
            let mut buf = Vec::new();
            let mut scored: Vec<(u32, usize)> = self
                .haystacks
                .iter()
                .enumerate()
                .filter_map(|(i, h)| {
                    pattern
                        .score(Utf32Str::new(h, &mut buf), &mut self.matcher)
                        .map(|s| (s, i))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            self.filtered = scored.into_iter().map(|(_, i)| i).collect();
        }
        let cursor = previous
            .and_then(|key| {
                self.filtered
                    .iter()
                    .position(|&i| self.rows[i].pr.key() == key)
            })
            .unwrap_or(0);
        self.list_state.select(Some(cursor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Review;

    fn pr(n: u64) -> Pr {
        Pr {
            repo: "o/r".into(),
            number: n,
            title: format!("pr {n}"),
            url: String::new(),
            author: String::new(),
            is_draft: false,
            base_ref: "main".into(),
            head_ref: format!("b{n}"),
            checks: Checks::None,
            review: Review::None,
        }
    }

    fn snapshot(mine: usize, review: usize) -> Snapshot {
        let mut s = Snapshot::default();
        *s.get_mut(Kind::Mine) = (1..=mine as u64).map(pr).collect();
        *s.get_mut(Kind::ReviewRequested) = (1..=review as u64).map(pr).collect();
        s
    }

    #[test]
    fn switching_to_a_shorter_list_resets_the_cursor() {
        let mut app = App::new(Some(snapshot(20, 1)));
        app.move_cursor(19);
        app.next_tab();
        assert_eq!(app.selected().unwrap().pr.number, 1);
    }

    #[test]
    fn fresh_shorter_list_keeps_selection_or_resets() {
        let mut app = App::new(Some(snapshot(20, 0)));
        app.move_cursor(19);
        app.start_fetch(1);
        app.set_list(Kind::Mine, vec![pr(3), pr(20)]);
        assert_eq!(app.selected().unwrap().pr.number, 20);
        app.set_list(Kind::Mine, vec![pr(3)]);
        assert_eq!(app.selected().unwrap().pr.number, 3);
    }

    #[test]
    fn fresh_list_keeps_cached_checks_until_checks_arrive() {
        let mut cached = snapshot(1, 0);
        cached.get_mut(Kind::Mine)[0].checks = Checks::Success;
        let mut app = App::new(Some(cached));
        app.start_fetch(2);
        app.set_list(Kind::Mine, vec![pr(1)]);
        assert_eq!(app.rows()[0].pr.checks, Checks::Success);
        app.set_checks(Kind::Mine, HashMap::from([(pr(1).key(), Checks::Failure)]));
        assert_eq!(app.rows()[0].pr.checks, Checks::Failure);
        assert_eq!(app.status(), Status::Fresh);
    }

    #[test]
    fn error_is_reported_after_all_steps_finish() {
        let mut app = App::new(None);
        app.start_fetch(2);
        app.set_error(Kind::Mine, "boom".into());
        assert!(matches!(app.status(), Status::Fetching { .. }));
        app.set_list(Kind::Assigned, vec![]);
        assert_eq!(app.status(), Status::Error("Mine: boom".into()));
    }
}
