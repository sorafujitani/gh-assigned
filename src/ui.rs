use crate::app::{App, Status};
use crate::github::{Checks, Kind, Review};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const ACCENT: Color = Color::Cyan;

fn accent() -> Style {
    Style::new().fg(ACCENT)
}

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

// Fixed 256-color grey with a light foreground: ANSI DarkGray is theme-mapped and
// vanishes on dark themes, and a bare grey background swallows black text on light ones.
const HIT_BG: Color = Color::Indexed(238);
const HIT_FG: Color = Color::Indexed(255);

fn hit() -> Style {
    Style::new().bg(HIT_BG).fg(HIT_FG)
}

/// Split `text` into spans, giving the chars at `hits` (shifted by `offset`) the hit background.
fn highlighted(text: &str, hits: &[usize], offset: usize, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut run = String::new();
    let mut run_hit = false;
    for (i, c) in text.chars().enumerate() {
        let mut is_hit = i >= offset && hits.contains(&(i - offset));
        // Combining marks and variation selectors must stay with their base char.
        if unicode_width::UnicodeWidthChar::width(c) == Some(0) && !run.is_empty() {
            is_hit = run_hit;
        }
        if is_hit != run_hit && !run.is_empty() {
            spans.push(Span::styled(
                std::mem::take(&mut run),
                style_for(run_hit, base),
            ));
        }
        run_hit = is_hit;
        run.push(c);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, style_for(run_hit, base)));
    }
    spans
}

fn style_for(is_hit: bool, base: Style) -> Style {
    if is_hit { base.patch(hit()) } else { base }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let notice = notice_widget(app, frame.area().width);
    let notice_height = notice.as_ref().map_or(0, |(_, height)| *height);
    let [prompt, info, notice_area, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(notice_height),
        Constraint::Min(1),
    ])
    .areas(frame.area());

    frame.render_widget(prompt_line(app), prompt);
    if app.help {
        draw_help(frame, info.union(notice_area).union(list));
        return;
    }
    frame.render_widget(info_line(app), info);
    if let Some((notice, _)) = notice {
        frame.render_widget(notice, notice_area);
    }
    draw_list(frame, app, list);
    let cursor_x = mode_label(app).width() + PROMPT.width() + app.query.width();
    let cursor_x = u16::try_from(cursor_x).unwrap_or(u16::MAX);
    frame.set_cursor_position((prompt.x.saturating_add(cursor_x), prompt.y));
}

const PROMPT: &str = "> ";

/// Always shown so the default all·fuzzy is as visible as any other setting.
fn mode_label(app: &App) -> String {
    format!("{}·{} ", app.scope.label(), app.mode.label())
}

fn prompt_line(app: &App) -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled(mode_label(app), dim()),
        Span::styled(PROMPT, accent()),
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
    Paragraph::new(Line::from(spans))
}

fn notice_widget(app: &App, width: u16) -> Option<(Paragraph<'static>, u16)> {
    let (message, style) = match app.notice()? {
        Ok(message) => (message, accent()),
        Err(error) => (error, Style::new().fg(Color::Red)),
    };
    let lines = wrap_notice(&format!("   {message}"), width as usize);
    let height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let lines = lines
        .into_iter()
        .map(|line| Line::from(Span::styled(line, style)))
        .collect::<Vec<_>>();
    Some((Paragraph::new(lines), height))
}

fn wrap_notice(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    text.split('\n')
        .flat_map(|line| wrap_notice_line(line, width))
        .collect()
}

fn wrap_notice_line(mut remaining: &str, width: usize) -> Vec<String> {
    if remaining.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    while remaining.width() > width {
        let mut line_width = 0;
        let mut hard_break = None;
        let mut word_break = None;
        for (index, character) in remaining.char_indices() {
            let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
            if line_width + character_width > width {
                break;
            }
            line_width += character_width;
            hard_break = Some(index + character.len_utf8());
            if character.is_whitespace() {
                word_break = hard_break;
            }
        }
        let split_at = word_break.or(hard_break).unwrap_or_else(|| {
            remaining
                .chars()
                .next()
                .map_or(remaining.len(), char::len_utf8)
        });
        let (line, rest) = remaining.split_at(split_at);
        lines.push(line.to_owned());
        remaining = rest;
    }
    lines.push(remaining.to_owned());
    lines
}

const HELP: &str = include_str!("../HELP.txt");

/// The `Keys` section of HELP.txt, so the overlay and `--help` never drift apart.
fn help_keys() -> impl Iterator<Item = &'static str> {
    HELP.lines()
        .skip_while(|l| *l != "Keys")
        .skip(1)
        .take_while(|l| !l.is_empty())
}

/// Rows the help overlay needs below the prompt.
pub fn help_rows() -> usize {
    help_keys().count() + 1
}

/// Falls back to side-by-side columns when the viewport is shorter than the key list.
fn draw_help(frame: &mut Frame, area: Rect) {
    let keys: Vec<Line> = help_keys().map(Line::raw).collect();
    let rows = (area.height as usize).max(1);
    let widest = help_keys().map(UnicodeWidthStr::width).max().unwrap_or(0);
    let col_width = u16::try_from(widest).unwrap_or(u16::MAX).saturating_add(3);
    let columns_that_fit = (area.width / col_width.max(1)).max(1) as usize;
    let shown = (rows.saturating_sub(1) + rows * (columns_that_fit - 1)).min(keys.len());
    let hint = if shown < keys.len() {
        "   esc to close · terminal too small, some keys hidden"
    } else {
        "   esc to close"
    };
    let mut lines = vec![Line::from(vec![
        Span::styled("  keys", accent().add_modifier(Modifier::BOLD)),
        Span::styled(hint, dim()),
    ])];
    lines.extend(keys);
    let mut x = area.x;
    for column in lines.chunks(rows) {
        if x >= area.right() {
            break;
        }
        let width = col_width.min(area.right() - x);
        let col = Rect::new(x, area.y, width, area.height);
        frame.render_widget(Paragraph::new(column.to_vec()), col);
        x += col_width;
    }
}

const RIGHT_MARGIN: usize = 2;
const DRAFT_PREFIX: &str = "[draft] ";
/// Separates the review column from the author column.
const AUTHOR_GAP: &str = " ";

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
            let hl = app.highlight(pos);
            let repo = format!("{}  ", pr.short_repo());
            // Padded by display width, not char count, so wide names keep the column.
            let author = if author_width > 0 {
                let pad = " ".repeat(author_width.saturating_sub(pr.author.width()));
                format!("{AUTHOR_GAP}{}{pad}", pr.author)
            } else {
                String::new()
            };
            let checks = format!(" {}", checks_mark(pr.checks));
            let review = format!(" {}", review_mark(pr.review));

            let used = pointer.width()
                + tree.width()
                + repo.width()
                + checks.width()
                + review.width()
                + author.width();
            let title_width = width.saturating_sub(used);
            let full_title = if pr.is_draft {
                format!("{DRAFT_PREFIX}{}", pr.title)
            } else {
                pr.title.clone()
            };
            let title = truncate(&full_title, title_width);
            let pad = " ".repeat(title_width.saturating_sub(title.width()));

            let title_style = if pr.is_draft { dim() } else { Style::new() };
            let mut spans = vec![
                Span::styled(pointer, accent()),
                Span::styled(tree, accent()),
            ];
            spans.extend(highlighted(&repo, &hl.repo, 0, Style::new()));
            // Hits past the truncation point would land on the ellipsis.
            let kept = title
                .chars()
                .count()
                .saturating_sub(usize::from(title != full_title));
            let title_hits: Vec<usize> = hl
                .title
                .iter()
                .map(|&i| i + DRAFT_PREFIX.chars().count() * usize::from(pr.is_draft))
                .filter(|&i| i < kept)
                .collect();
            spans.extend(highlighted(&title, &title_hits, 0, title_style));
            spans.push(Span::raw(pad));
            spans.push(Span::styled(checks, checks_style(pr.checks)));
            spans.push(Span::styled(review, review_style(pr.review)));
            spans.extend(highlighted(
                &author,
                &hl.author,
                AUTHOR_GAP.chars().count(),
                dim(),
            ));
            let mut line = Line::from(spans);
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

fn review_mark(r: Review) -> &'static str {
    match r {
        Review::Approved => "approved",
        Review::ChangesRequested => "changes ",
        Review::Pending => "review  ",
        Review::None => "        ",
    }
}

fn review_style(r: Review) -> Style {
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
    use ratatui::buffer::Buffer;

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

    fn render_buffer(app: &mut App, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn text(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render(app: &mut App, width: u16) -> String {
        text(&render_buffer(app, width, 12))
    }

    /// The symbols on `row` drawn with the hit background.
    fn marked(buf: &Buffer, row: u16) -> String {
        (0..buf.area.width)
            .filter(|&x| buf[(x, row)].bg == HIT_BG)
            .map(|x| buf[(x, row)].symbol().to_string())
            .collect()
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
        assert!(out.contains("all·fuzzy > unrel"), "{out}");
        assert!(out.contains("1/3"));
        assert!(out.contains("r  unrelated"));
        assert!(!out.contains("first"));
        assert_eq!(app.selected().unwrap().pr.number, 3);
        app.next_scope();
        assert!(render(&mut app, 80).contains("repo·fuzzy > unrel"));
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
    fn notice_wraps_below_status_on_a_narrow_terminal() {
        let mut app = App::new(Some(stacked()));
        let url = "https://github.com/o/r/pull/8";
        app.notify(Err(anyhow::anyhow!(
            "could not open {url}: try `gh auth login`, then check GH_BROWSER/BROWSER"
        )));
        let out = render(&mut app, 40);
        let lines: Vec<&str> = out.lines().collect();

        assert!(!lines[1].contains("could not open"), "{out}");
        assert!(lines.iter().skip(2).any(|line| line.contains(url)), "{out}");
        assert!(
            lines.iter().skip(2).any(|line| line.contains("auth login")),
            "{out}"
        );
    }

    #[test]
    fn matched_chars_get_a_background() {
        let mut app = App::new(Some(stacked()));
        for c in "unrel".chars() {
            app.push_char(c);
        }
        assert_eq!(marked(&render_buffer(&mut app, 80, 12), 2), "unrel");
    }

    #[test]
    fn help_overlay_lists_keys_and_hides_the_list() {
        let mut app = App::new(Some(stacked()));
        app.help = true;
        let out = text(&render_buffer(&mut app, 80, 20));
        assert!(out.contains("esc to close"), "{out}");
        assert!(!out.contains("some keys hidden"), "{out}");
        assert!(out.contains("esc / ctrl-c"), "{out}");
        assert!(!out.contains("first"), "{out}");

        // Too short and too narrow for columns: say so instead of silently truncating.
        let out = render(&mut app, 80);
        assert!(out.contains("some keys hidden"), "{out}");
    }

    #[test]
    fn narrow_terminal_does_not_panic() {
        let mut snapshot = Snapshot::default();
        *snapshot.get_mut(Kind::Mine) =
            vec![pr(1, "main", "a", "a very long title that will not fit")];
        let mut long = pr(2, "main", "b", "second");
        long.author = "dependabot[bot]".into();
        long.repo = "org/my-service".into();
        *snapshot.get_mut(Kind::ReviewRequested) = vec![long];
        let mut app = App::new(Some(snapshot));
        for width in [40, 20, 10, 1] {
            render(&mut app, width);
        }
        app.next_tab();
        for width in [40, 20, 10, 1] {
            render(&mut app, width);
        }
    }

    #[test]
    fn highlight_stays_on_the_chars_after_an_emoji() {
        let mut snapshot = Snapshot::default();
        *snapshot.get_mut(Kind::Mine) = vec![pr(1, "main", "a", "❤\u{fe0f} fix login")];
        let mut app = App::new(Some(snapshot));
        for c in "login".chars() {
            app.push_char(c);
        }
        assert_eq!(marked(&render_buffer(&mut app, 80, 12), 2), "login");
    }

    #[test]
    fn short_help_viewport_uses_columns() {
        let mut app = App::new(Some(stacked()));
        app.help = true;
        let out = text(&render_buffer(&mut app, 160, 8));
        assert!(out.contains("esc / ctrl-c"), "{out}");
        assert!(out.contains("ctrl-r"), "{out}");
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
