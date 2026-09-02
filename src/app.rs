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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Repo,
    Title,
    Author,
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Scope::All => "all",
            Scope::Repo => "repo",
            Scope::Title => "title",
            Scope::Author => "author",
        }
    }

    fn next(self, show_author: bool) -> Self {
        match self {
            Scope::All => Scope::Repo,
            Scope::Repo => Scope::Title,
            Scope::Title if show_author => Scope::Author,
            Scope::Title | Scope::Author => Scope::All,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Fuzzy,
    Substring,
    Exact,
}

impl MatchMode {
    pub fn label(self) -> &'static str {
        match self {
            MatchMode::Fuzzy => "fuzzy",
            MatchMode::Substring => "substring",
            MatchMode::Exact => "exact",
        }
    }

    // Exact only makes sense against a single field, so `All` skips it.
    fn next(self, scope: Scope) -> Self {
        match self {
            MatchMode::Fuzzy => MatchMode::Substring,
            MatchMode::Substring if scope == Scope::All => MatchMode::Fuzzy,
            MatchMode::Substring => MatchMode::Exact,
            MatchMode::Exact => MatchMode::Fuzzy,
        }
    }

    fn allowed_in(self, scope: Scope) -> Self {
        if self == MatchMode::Exact && scope == Scope::All {
            MatchMode::Substring
        } else {
            self
        }
    }
}

/// Only what the list shows is searchable, so every hit can be highlighted.
struct Haystack {
    /// Fields joined by single spaces, for `Scope::All`.
    all: String,
    repo: String,
    title: String,
    author: Option<String>,
}

impl Haystack {
    fn new(pr: &Pr, show_author: bool) -> Self {
        let repo = pr.short_repo().to_string();
        let author = show_author.then(|| pr.author.clone());
        let mut all = format!("{repo} {}", pr.title);
        if let Some(a) = &author {
            all.push(' ');
            all.push_str(a);
        }
        Haystack {
            all,
            repo,
            title: pr.title.clone(),
            author,
        }
    }

    fn text(&self, scope: Scope) -> Option<&str> {
        match scope {
            Scope::All => Some(&self.all),
            Scope::Repo => Some(&self.repo),
            Scope::Title => Some(&self.title),
            Scope::Author => self.author.as_deref(),
        }
    }

    /// Split hit positions in `all` back into per-field positions.
    fn split(&self, hits: &[u32]) -> Highlight {
        let mut out = Highlight::default();
        let title_start = self.repo.chars().count() + 1;
        let author_start = title_start + self.title.chars().count() + 1;
        for &i in hits {
            let i = i as usize;
            if i < title_start - 1 {
                out.repo.push(i);
            } else if i >= title_start && i < author_start - 1 {
                out.title.push(i - title_start);
            } else if i >= author_start && self.author.is_some() {
                out.author.push(i - author_start);
            }
        }
        out
    }
}

/// How the query is matched, built once per refilter.
enum Needle {
    Fuzzy(Pattern),
    // Hand-rolled: nucleo 0.3's non-ASCII substring matcher skips the last start
    // position, so anything ending at the end of a field never matches.
    Substring(Vec<char>),
    Exact(Vec<char>),
}

fn idx(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

fn fold(c: char) -> char {
    // One char per char keeps hit indices aligned with the haystack.
    c.to_lowercase().next().unwrap_or(c)
}

impl Needle {
    fn new(query: &str, mode: MatchMode) -> Self {
        let folded = || query.chars().map(fold).collect();
        match mode {
            MatchMode::Fuzzy => Needle::Fuzzy(Pattern::parse(
                query,
                CaseMatching::Ignore,
                Normalization::Smart,
            )),
            // Substring and exact take the query verbatim, spaces included.
            MatchMode::Substring => Needle::Substring(folded()),
            MatchMode::Exact => Needle::Exact(folded()),
        }
    }

    /// Fills `hits` with char positions and returns a score (higher sorts first).
    fn indices(&self, hay: &[char], matcher: &mut Matcher, hits: &mut Vec<u32>) -> Option<u32> {
        match self {
            Needle::Fuzzy(p) => p.indices(Utf32Str::Unicode(hay), matcher, hits),
            Needle::Substring(needle) => {
                let start = hay
                    .windows(needle.len())
                    .position(|w| w.iter().map(|&c| fold(c)).eq(needle.iter().copied()))?;
                hits.extend((start..start + needle.len()).map(idx));
                // Earlier matches rank higher, as in fzf.
                Some(u32::MAX - idx(start))
            }
            Needle::Exact(needle) => {
                if !hay.iter().map(|&c| fold(c)).eq(needle.iter().copied()) {
                    return None;
                }
                hits.extend((0..hay.len()).map(idx));
                Some(0)
            }
        }
    }
}

/// Matched char positions within each displayed field.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Highlight {
    pub repo: Vec<usize>,
    pub title: Vec<usize>,
    pub author: Vec<usize>,
}

pub struct App {
    snapshot: Snapshot,
    rows: Vec<crate::stack::Row>,
    haystacks: Vec<Haystack>,
    pub filtered: Vec<usize>,
    /// Parallel to `filtered`: char indices into the scoped field(s) of the row.
    hits: Vec<Vec<u32>>,
    pub tab: Kind,
    pub query: String,
    pub scope: Scope,
    pub mode: MatchMode,
    pub list_state: ListState,
    pub help: bool,
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
            hits: Vec::new(),
            tab: Kind::Mine,
            query: String::new(),
            scope: Scope::All,
            mode: MatchMode::Fuzzy,
            list_state: ListState::default(),
            help: false,
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

    /// Highlight for the `pos`-th visible row.
    pub fn highlight(&self, pos: usize) -> Highlight {
        let mut out = Highlight::default();
        let (Some(&row), Some(hits)) = (self.filtered.get(pos), self.hits.get(pos)) else {
            return out;
        };
        let positions = hits.iter().map(|&i| i as usize).collect::<Vec<_>>();
        match self.scope {
            Scope::All => return self.haystacks[row].split(hits),
            Scope::Repo => out.repo = positions,
            Scope::Title => out.title = positions,
            Scope::Author => out.author = positions,
        }
        out
    }

    fn shows_author(&self) -> bool {
        self.tab != Kind::Mine
    }

    pub fn next_scope(&mut self) {
        self.scope = self.scope.next(self.shows_author());
        self.mode = self.mode.allowed_in(self.scope);
        self.refilter();
    }

    pub fn next_mode(&mut self) {
        self.mode = self.mode.next(self.scope);
        self.refilter();
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

    pub fn set_checks(&mut self, kind: Kind, checks: &HashMap<PrKey, Checks>) {
        for pr in self.snapshot.get_mut(kind) {
            if let Some(c) = checks.get(&pr.key()) {
                pr.checks = *c;
            }
        }
        self.finish_step(kind);
    }

    pub fn set_error(&mut self, kind: Kind, message: &str) {
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
        let last = self.filtered.len() - 1;
        let next = self.cursor().saturating_add_signed(delta).min(last);
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
        let cut = trimmed.rfind(' ').map_or(0, |i| i + 1);
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
        let show_author = self.shows_author();
        if self.scope == Scope::Author && !show_author {
            self.scope = Scope::All;
            self.mode = self.mode.allowed_in(self.scope);
        }
        self.haystacks = self
            .rows
            .iter()
            .map(|r| Haystack::new(&r.pr, show_author))
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
            self.hits = vec![Vec::new(); self.rows.len()];
        } else {
            let needle = Needle::new(&self.query, self.mode);
            let mut buf = Vec::new();
            let mut scored: Vec<(u32, usize, Vec<u32>)> = self
                .haystacks
                .iter()
                .enumerate()
                .filter_map(|(i, h)| {
                    let mut hits = Vec::new();
                    // `Utf32Str::new` indexes by grapheme (or byte for ASCII); the
                    // highlighter counts chars, so feed nucleo chars explicitly.
                    buf.clear();
                    buf.extend(h.text(self.scope)?.chars());
                    let score = needle.indices(&buf, &mut self.matcher, &mut hits)?;
                    hits.sort_unstable();
                    hits.dedup();
                    Some((score, i, hits))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            self.filtered = scored.iter().map(|(_, i, _)| *i).collect();
            self.hits = scored.into_iter().map(|(_, _, hits)| hits).collect();
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
        app.set_checks(Kind::Mine, &HashMap::from([(pr(1).key(), Checks::Failure)]));
        assert_eq!(app.rows()[0].pr.checks, Checks::Failure);
        assert_eq!(app.status(), Status::Fresh);
    }

    #[test]
    fn error_is_reported_after_all_steps_finish() {
        let mut app = App::new(None);
        app.start_fetch(2);
        app.set_error(Kind::Mine, "boom");
        assert!(matches!(app.status(), Status::Fetching { .. }));
        app.set_list(Kind::Assigned, vec![]);
        assert_eq!(app.status(), Status::Error("Mine: boom".into()));
    }

    #[test]
    fn hidden_owner_and_author_do_not_match_on_mine() {
        let mut snapshot = Snapshot::default();
        let mut hidden = pr(1);
        hidden.repo = "linter-org/r".into();
        hidden.author = "linter".into();
        hidden.title = "unrelated".into();
        *snapshot.get_mut(Kind::Mine) = vec![hidden];
        let mut app = App::new(Some(snapshot));
        for c in "lint".chars() {
            app.push_char(c);
        }
        assert!(app.filtered.is_empty());
    }

    #[test]
    fn pr_number_is_not_searchable() {
        let mut snapshot = Snapshot::default();
        let mut p = pr(42);
        p.title = "no digits".into();
        *snapshot.get_mut(Kind::Mine) = vec![p];
        let mut app = App::new(Some(snapshot));
        app.push_char('4');
        assert!(app.filtered.is_empty());
    }

    #[test]
    fn scope_limits_matching_to_one_field() {
        let mut snapshot = Snapshot::default();
        let mut p = pr(1);
        p.repo = "o/docs".into();
        p.title = "api".into();
        *snapshot.get_mut(Kind::Mine) = vec![p];
        let mut app = App::new(Some(snapshot));
        for c in "docs".chars() {
            app.push_char(c);
        }
        assert_eq!(app.filtered, vec![0]);
        app.next_scope();
        assert_eq!(app.scope, Scope::Repo);
        assert_eq!(app.filtered, vec![0]);
        assert_eq!(app.highlight(0).repo, vec![0, 1, 2, 3]);
        app.next_scope();
        assert_eq!(app.scope, Scope::Title);
        assert!(app.filtered.is_empty());
        // Mine hides the author, so the cycle skips it.
        app.next_scope();
        assert_eq!(app.scope, Scope::All);
    }

    #[test]
    fn substring_and_exact_modes() {
        let mut snapshot = Snapshot::default();
        let mut a = pr(1);
        a.title = "toridori docs".into();
        let mut b = pr(2);
        b.title = "t o r i d o r i".into();
        *snapshot.get_mut(Kind::Mine) = vec![a, b];
        let mut app = App::new(Some(snapshot));
        for c in "toridori".chars() {
            app.push_char(c);
        }
        assert_eq!(app.filtered.len(), 2);
        app.next_mode();
        assert_eq!(app.mode, MatchMode::Substring);
        assert_eq!(app.filtered, vec![0]);
        // Exact is skipped while searching all fields.
        app.next_mode();
        assert_eq!(app.mode, MatchMode::Fuzzy);
        app.next_scope();
        app.next_scope();
        assert_eq!(app.scope, Scope::Title);
        app.next_mode();
        app.next_mode();
        assert_eq!(app.mode, MatchMode::Exact);
        assert!(app.filtered.is_empty());
        for c in " docs".chars() {
            app.push_char(c);
        }
        assert_eq!(app.filtered, vec![0]);
        // Leaving a single field drops exact back to substring.
        app.next_scope();
        assert_eq!(app.scope, Scope::All);
        assert_eq!(app.mode, MatchMode::Substring);
    }

    fn substring_hits(title: &str, author: &str, scope_steps: usize, query: &str) -> Highlight {
        let mut snapshot = Snapshot::default();
        let mut p = pr(1);
        p.repo = "o/gh-assigned".into();
        p.title = title.into();
        p.author = author.into();
        *snapshot.get_mut(Kind::ReviewRequested) = vec![p];
        let mut app = App::new(Some(snapshot));
        app.next_tab();
        app.next_mode();
        assert_eq!(app.mode, MatchMode::Substring);
        for _ in 0..scope_steps {
            app.next_scope();
        }
        for c in query.chars() {
            app.push_char(c);
        }
        assert_eq!(app.filtered, vec![0], "{query:?} in scope {:?}", app.scope);
        app.highlight(0)
    }

    #[test]
    fn substring_matches_up_to_the_end_of_a_field() {
        assert_eq!(
            substring_hits("toridori docs", "octocat", 0, "docs").title,
            vec![9, 10, 11, 12]
        );
        assert_eq!(
            substring_hits("toridori docs", "octocat", 0, "cat").author,
            vec![4, 5, 6]
        );
        assert_eq!(
            substring_hits("toridori docs", "octocat", 1, "assigned").repo,
            (3..11).collect::<Vec<_>>()
        );
        assert_eq!(
            substring_hits("日本語のタイトル", "octocat", 2, "タイトル").title,
            vec![4, 5, 6, 7]
        );
        assert_eq!(
            substring_hits("Mixed Case", "octocat", 2, "CASE").title,
            vec![6, 7, 8, 9]
        );
    }

    #[test]
    fn substring_ranks_earlier_matches_first() {
        let mut snapshot = Snapshot::default();
        let mut late = pr(1);
        late.title = "fix the docs".into();
        let mut early = pr(2);
        early.title = "docs fix".into();
        *snapshot.get_mut(Kind::Mine) = vec![late, early];
        let mut app = App::new(Some(snapshot));
        app.next_mode();
        for c in "docs".chars() {
            app.push_char(c);
        }
        assert_eq!(app.filtered, vec![1, 0]);
    }

    #[test]
    fn hits_are_char_indices_even_with_graphemes_and_ascii() {
        let mut snapshot = Snapshot::default();
        let mut emoji = pr(1);
        emoji.title = "❤\u{fe0f} fix login".into();
        let mut ascii = pr(2);
        ascii.title = "plain login".into();
        ascii.author = "ali".into();
        *snapshot.get_mut(Kind::ReviewRequested) = vec![emoji, ascii];
        let mut app = App::new(Some(snapshot));
        app.next_tab();
        for c in "login".chars() {
            app.push_char(c);
        }
        let by_row: Vec<(u64, Vec<usize>)> = (0..app.filtered.len())
            .map(|pos| {
                (
                    app.rows()[app.filtered[pos]].pr.number,
                    app.highlight(pos).title,
                )
            })
            .collect();
        assert!(by_row.contains(&(1, vec![7, 8, 9, 10, 11])), "{by_row:?}");
        assert!(by_row.contains(&(2, vec![6, 7, 8, 9, 10])), "{by_row:?}");
    }

    #[test]
    fn highlight_maps_hits_onto_repo_title_and_author() {
        let mut snapshot = Snapshot::default();
        let mut p = pr(1);
        p.repo = "o/rb".into();
        p.title = "日本 x".into();
        p.author = "ann".into();
        *snapshot.get_mut(Kind::ReviewRequested) = vec![p];
        let mut app = App::new(Some(snapshot));
        app.next_tab();
        for c in "b本n".chars() {
            app.push_char(c);
        }
        assert_eq!(app.filtered, vec![0]);
        let h = app.highlight(0);
        assert_eq!(h.repo, vec![1]);
        assert_eq!(h.title, vec![1]);
        assert_eq!(h.author, vec![1]);
    }
}
