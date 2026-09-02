use crate::app::{App, Status};
use crate::github::{Checks, Kind, Review};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use unicode_width::UnicodeWidthStr;

const ACCENT: Color = Color::Cyan;

fn accent() -> Style {
    Style::new().fg(ACCENT)
}

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [prompt, info, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(frame.area());

    frame.render_widget(prompt_line(app), prompt);
    frame.render_widget(info_line(app), info);
    draw_list(frame, app, list);
    frame.set_cursor_position((prompt.x + 2 + app.query.width() as u16, prompt.y));
}

fn prompt_line(app: &App) -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled("> ", accent()),
        Span::raw(app.query.clone()),
    ]))
}

fn info_line(app: &App) -> Paragraph<'static> {
    let mut spans = vec![Span::styled(
        format!("  {}/{}  ", app.filtered.len(), app.rows().len()),
        accent().add_modifier(Modifier::DIM),
    )];
    for (i, kind) in Kind::ALL.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", dim()));
        }
        let text = format!("{} {}", kind.label().to_lowercase(), app.count(kind));
        let style = if kind == app.tab {
            accent().add_modifier(Modifier::BOLD)
        } else {
            dim()
        };
        spans.push(Span::styled(text, style));
    }
    let status = match app.status() {
        Status::Fetching {
            showing_cache: true,
        } => Span::styled("   cached · fetching…", dim()),
        Status::Fetching {
            showing_cache: false,
        } => Span::styled("   fetching…", dim()),
        Status::Fresh => Span::raw(""),
        Status::Error(e) => Span::styled(format!("   error: {e}"), Style::new().fg(Color::Red)),
    };
    spans.push(status);
    match app.notice() {
        Some(Ok(msg)) => spans.push(Span::styled(format!("   {msg}"), accent())),
        Some(Err(e)) => spans.push(Span::styled(format!("   {e}"), Style::new().fg(Color::Red))),
        None => {}
    }
    Paragraph::new(Line::from(spans))
}

fn short_repo(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}

const RIGHT_MARGIN: usize = 2;

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let width = (area.width as usize).saturating_sub(RIGHT_MARGIN);
    // Fixed author width keeps the CI and review columns vertically aligned.
    let author_width = if app.tab == Kind::Mine {
        0
    } else {
        app.rows()
            .iter()
            .map(|r| r.pr.author.width())
            .max()
            .unwrap_or(0)
    };
    let cursor = app.cursor();

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(pos, &i)| {
            let row = &app.rows()[i];
            let pr = &row.pr;
            let current = pos == cursor;
            let pointer = if current { "▌ " } else { "  " };
            let indent = "  ".repeat(row.depth);
            let branch = if row.depth > 0 { "└ " } else { "" };
            let tree = format!("{indent}{branch}");
            let repo = format!("{}  ", short_repo(&pr.repo));
            let author = if author_width > 0 {
                format!(" {:<w$}", pr.author, w = author_width)
            } else {
                String::new()
            };
            let right = format!(
                " {} {}{author}",
                checks_mark(pr.checks),
                review_mark(&pr.review)
            );

            let used = pointer.width() + tree.width() + repo.width() + right.width();
            let title_width = width.saturating_sub(used);
            let full_title = if pr.is_draft {
                format!("[draft] {}", pr.title)
            } else {
                pr.title.clone()
            };
            let title = truncate(&full_title, title_width);
            let pad = " ".repeat(title_width.saturating_sub(title.width()));

            let title_style = if pr.is_draft { dim() } else { Style::new() };
            let mut line = Line::from(vec![
                Span::styled(pointer, accent()),
                Span::styled(tree, accent()),
                Span::raw(repo),
                Span::styled(title, title_style),
                Span::raw(pad),
                Span::styled(
                    format!(" {}", checks_mark(pr.checks)),
                    checks_style(pr.checks),
                ),
                Span::styled(
                    format!(" {}", review_mark(&pr.review)),
                    review_style(&pr.review),
                ),
                Span::styled(author, dim()),
            ]);
            if current {
                line = line.style(Style::new().add_modifier(Modifier::BOLD));
            }
            ListItem::new(line)
        })
        .collect();

    frame.render_stateful_widget(List::new(items), area, &mut app.list_state);
}

fn truncate(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let budget = max.saturating_sub(1);
    for c in s.chars() {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if out.width() + w > budget {
            break;
        }
        out.push(c);
    }
    if max > 0 {
        out.push('…');
    }
    out
}

fn checks_mark(c: Checks) -> &'static str {
    match c {
        Checks::Success => "✓",
        Checks::Failure => "✗",
        Checks::Pending => "●",
        Checks::None => " ",
    }
}

fn checks_style(c: Checks) -> Style {
    match c {
        Checks::Success => accent(),
        Checks::Failure => Style::new().fg(Color::Red),
        Checks::Pending => Style::new(),
        Checks::None => dim(),
    }
}

fn review_mark(r: &Review) -> &'static str {
    match r {
        Review::Approved => "approved",
        Review::ChangesRequested => "changes ",
        Review::Pending => "review  ",
        Review::None => "        ",
    }
}

fn review_style(r: &Review) -> Style {
    match r {
        Review::Approved => accent(),
        Review::ChangesRequested => Style::new().fg(Color::Red),
        Review::Pending => Style::new(),
        Review::None => dim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::{Pr, Snapshot};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn pr(n: u64, base: &str, head: &str, title: &str) -> Pr {
        Pr {
            repo: "o/r".into(),
            number: n,
            title: title.into(),
            url: format!("https://github.com/o/r/pull/{n}"),
            author: "me".into(),
            is_draft: n == 2,
            base_ref: base.into(),
            head_ref: head.into(),
            checks: Checks::Success,
            review: Review::Approved,
        }
    }

    fn render(app: &mut App, width: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, 12)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn stacked() -> Snapshot {
        let mut snapshot = Snapshot::default();
        *snapshot.get_mut(Kind::Mine) = vec![
            pr(1, "main", "a", "first"),
            pr(2, "a", "b", "second"),
            pr(3, "main", "c", "unrelated"),
        ];
        snapshot
    }

    #[test]
    fn renders_stack_and_filter() {
        let mut app = App::new(Some(stacked()));
        let out = render(&mut app, 80);
        assert!(out.contains("3/3"));
        assert!(out.contains("▌ r  first"));
        assert!(out.contains("  └ r  [draft] second"));
        assert!(!out.contains(" me"), "author is hidden on Mine");

        for c in "unrel".chars() {
            app.push_char(c);
        }
        let out = render(&mut app, 80);
        assert!(out.contains("1/3"));
        assert!(out.contains("r  unrelated"));
        assert!(!out.contains("first"));
        assert_eq!(app.selected().unwrap().pr.number, 3);
    }

    #[test]
    fn author_column_is_padded_so_status_columns_align() {
        let mut snapshot = Snapshot::default();
        let mut long = pr(2, "main", "b", "second");
        long.author = "someone-long".into();
        *snapshot.get_mut(Kind::ReviewRequested) = vec![pr(1, "main", "a", "first"), long];
        let mut app = App::new(Some(snapshot));
        app.next_tab();
        let out = render(&mut app, 80);
        let cols: Vec<usize> = out
            .lines()
            .filter_map(|l| l.chars().position(|c| c == '✓'))
            .collect();
        assert_eq!(cols.len(), 2, "{out}");
        assert_eq!(cols[0], cols[1], "{out}");
        assert!(out.contains("approved me          "), "{out}");
    }

    #[test]
    fn shows_cache_marker_while_first_fetch_runs() {
        let mut app = App::new(Some(stacked()));
        app.start_fetch(6);
        assert!(render(&mut app, 80).contains("cached · fetching…"));
        app.set_list(Kind::Mine, vec![pr(1, "main", "a", "first")]);
        assert!(render(&mut app, 80).contains(" fetching…"));
        assert!(!render(&mut app, 80).contains("cached"));
    }

    #[test]
    fn narrow_terminal_does_not_panic() {
        let mut snapshot = Snapshot::default();
        *snapshot.get_mut(Kind::Mine) =
            vec![pr(1, "main", "a", "a very long title that will not fit")];
        let mut app = App::new(Some(snapshot));
        render(&mut app, 20);
    }

    #[test]
    fn wide_and_draft_titles_keep_the_right_columns() {
        let mut snapshot = Snapshot::default();
        *snapshot.get_mut(Kind::Mine) = vec![
            pr(
                1,
                "main",
                "a",
                "日本語のタイトルでカラムがずれるかどうかの確認です",
            ),
            pr(
                2,
                "main",
                "b",
                "a draft with a fairly long title that needs truncating here",
            ),
        ];
        let mut app = App::new(Some(snapshot));
        let out = render(&mut app, 80);
        for line in out.lines().skip(2).take(2) {
            assert!(line.ends_with("✓ approved  "), "{line:?}");
        }
    }
}
