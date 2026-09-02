mod app;
mod cache;
mod github;
mod stack;
mod ui;

use anyhow::{Result, bail};
use app::App;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, size};
use github::{Checks, Kind, PrKey};
use ratatui::prelude::CrosstermBackend;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::collections::HashMap;
use std::io::{self, IsTerminal};
use std::process::{Command, ExitCode};
use std::sync::mpsc;
use std::time::Duration;

enum Action {
    Continue,
    Reload,
    Quit,
    Open(String),
}

fn main() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => {}
        ["-h" | "--help"] => {
            print!("{}", include_str!("../HELP.txt"));
            return Ok(ExitCode::SUCCESS);
        }
        ["--json"] => {
            let snapshot = github::fetch_all()?;
            cache::store(&snapshot)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
            return Ok(ExitCode::SUCCESS);
        }
        _ => bail!(
            "unknown arguments: {}\nSee `gh assigned --help`.",
            args.join(" ")
        ),
    }

    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("gh assigned needs an interactive terminal; use --json for scripts");
    }

    let mut app = App::new(cache::load());
    let (tx, rx) = mpsc::channel();
    start_fetch(&mut app, &tx);

    // Drawn inline on stderr, fzf-style, so the shell scrollback stays visible.
    let mut terminal = enter_tui(app.rows().len())?;
    let result = run(&mut terminal, &mut app, &tx, &rx);
    leave_tui(terminal)?;

    match result? {
        Action::Open(url) => {
            open_in_browser(&url);
            Ok(ExitCode::SUCCESS)
        }
        // Same as fzf: cancelling exits with a distinct code.
        _ => Ok(ExitCode::from(130)),
    }
}

type Tui = Terminal<CrosstermBackend<io::Stderr>>;

const MIN_HEIGHT: u16 = 8;

/// Like fzf's `--height 40%`, capped by the visible list plus the two header rows.
fn viewport_height(rows: usize) -> u16 {
    let term_rows = size().map(|(_, h)| h).unwrap_or(24);
    let wanted = (rows as u16).saturating_add(2).max(MIN_HEIGHT);
    wanted
        .min((term_rows * 2 / 5).max(MIN_HEIGHT))
        .min(term_rows)
}

/// The help overlay needs more rows than the list usually gets.
fn help_viewport_height() -> u16 {
    let term_rows = size().map(|(_, h)| h).unwrap_or(24);
    (ui::help_rows() as u16).saturating_add(1).min(term_rows)
}

fn inline_terminal(height: u16) -> Result<Tui> {
    let options = TerminalOptions {
        viewport: Viewport::Inline(height),
    };
    Ok(Terminal::with_options(
        CrosstermBackend::new(io::stderr()),
        options,
    )?)
}

fn enter_tui(rows: usize) -> Result<Tui> {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        default_hook(info);
    }));
    enable_raw_mode()?;
    inline_terminal(viewport_height(rows))
}

/// An inline viewport cannot grow in place, so swap in a new one at the same spot.
fn resize_viewport(terminal: &mut Tui, height: u16) -> Result<()> {
    let _ = terminal.clear();
    *terminal = inline_terminal(height)?;
    Ok(())
}

fn leave_tui(mut terminal: Tui) -> Result<()> {
    // Erase the inline UI so the shell prompt returns where it was.
    let _ = terminal.clear();
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

enum Msg {
    List(Kind, Result<Vec<github::Pr>>),
    Checks(Kind, Result<HashMap<PrKey, Checks>>),
}

struct Tagged {
    generation: u64,
    msg: Msg,
}

/// Lists and CI state are independent queries, so all six run at once.
fn start_fetch(app: &mut App, tx: &mpsc::Sender<Tagged>) {
    let generation = app.start_fetch(Kind::ALL.len() * 2);
    for kind in Kind::ALL {
        let tx_list = tx.clone();
        std::thread::spawn(move || {
            let msg = Msg::List(kind, github::fetch_list(kind));
            let _ = tx_list.send(Tagged { generation, msg });
        });
        let tx_checks = tx.clone();
        std::thread::spawn(move || {
            let msg = Msg::Checks(kind, github::fetch_checks(kind));
            let _ = tx_checks.send(Tagged { generation, msg });
        });
    }
}

fn run(
    terminal: &mut Tui,
    app: &mut App,
    tx: &mpsc::Sender<Tagged>,
    rx: &mpsc::Receiver<Tagged>,
) -> Result<Action> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let mut changed = false;
        while let Ok(Tagged { generation, msg }) = rx.try_recv() {
            if !app.accepts(generation) {
                continue;
            }
            match msg {
                Msg::List(kind, Ok(prs)) => {
                    app.set_list(kind, prs);
                    changed = true;
                }
                Msg::Checks(kind, Ok(checks)) => {
                    app.set_checks(kind, checks);
                    changed = true;
                }
                Msg::List(kind, Err(e)) | Msg::Checks(kind, Err(e)) => {
                    app.set_error(kind, e.to_string())
                }
            }
        }
        if changed {
            let _ = cache::store(app.snapshot());
        }

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        // Windows reports key releases too; only presses count.
        if key.kind != KeyEventKind::Press {
            continue;
        }
        app.clear_notice();
        let help_was_open = app.help;
        let action = handle_key(app, key);
        if app.help != help_was_open {
            let height = if app.help {
                help_viewport_height()
            } else {
                viewport_height(app.rows().len())
            };
            resize_viewport(terminal, height)?;
        }
        match action {
            Action::Continue => {}
            Action::Reload => {
                if !app.is_fetching() {
                    start_fetch(app, tx);
                }
            }
            Action::Quit => return Ok(Action::Quit),
            Action::Open(url) => return Ok(Action::Open(url)),
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    if app.help {
        app.help = false;
        return Action::Continue;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let plain = !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
    match key.code {
        KeyCode::F(1) => {
            app.help = true;
            Action::Continue
        }
        // `?` only opens help on an empty prompt so it can still be typed mid-query.
        // cmd-? is accepted too, though most terminals never forward it.
        KeyCode::Char('?')
            if app.query.is_empty() || key.modifiers.contains(KeyModifiers::SUPER) =>
        {
            app.help = true;
            Action::Continue
        }
        KeyCode::Esc => Action::Quit,
        KeyCode::Char('c' | 'q') if ctrl => Action::Quit,
        KeyCode::Enter => match app.selected() {
            Some(row) => Action::Open(row.pr.url.clone()),
            None => Action::Continue,
        },
        // Open without leaving, for going through several PRs.
        KeyCode::Char('O') if plain => {
            if let Some(row) = app.selected() {
                open_in_browser(&row.pr.url);
            }
            Action::Continue
        }
        KeyCode::Char('Y') if plain => {
            if let Some(row) = app.selected() {
                let url = row.pr.url.clone();
                app.notify(copy_to_clipboard(&url).map(|()| format!("copied {url}")));
            }
            Action::Continue
        }
        KeyCode::Char('N') if plain => {
            if let Some(row) = app.selected() {
                let number = row.pr.number.to_string();
                app.notify(copy_to_clipboard(&number).map(|()| format!("copied #{number}")));
            }
            Action::Continue
        }
        KeyCode::Tab | KeyCode::Right => {
            app.next_tab();
            Action::Continue
        }
        KeyCode::BackTab | KeyCode::Left => {
            app.prev_tab();
            Action::Continue
        }
        KeyCode::Down => {
            app.move_cursor(1);
            Action::Continue
        }
        KeyCode::Char('n' | 'j') if ctrl => {
            app.move_cursor(1);
            Action::Continue
        }
        KeyCode::Up => {
            app.move_cursor(-1);
            Action::Continue
        }
        KeyCode::Char('p' | 'k') if ctrl => {
            app.move_cursor(-1);
            Action::Continue
        }
        KeyCode::PageDown => {
            app.move_cursor(10);
            Action::Continue
        }
        KeyCode::PageUp => {
            app.move_cursor(-10);
            Action::Continue
        }
        KeyCode::Backspace => {
            app.pop_char();
            Action::Continue
        }
        KeyCode::Char('h') if ctrl => {
            app.pop_char();
            Action::Continue
        }
        KeyCode::Char('w') if ctrl => {
            app.pop_word();
            Action::Continue
        }
        KeyCode::Char('u') if ctrl => {
            app.clear_query();
            Action::Continue
        }
        KeyCode::Char('r') if ctrl => Action::Reload,
        KeyCode::Char('f') if ctrl => {
            app.next_scope();
            Action::Continue
        }
        KeyCode::Char('t') if ctrl => {
            app.next_mode();
            Action::Continue
        }
        KeyCode::Char(c) if plain => {
            app.push_char(c);
            Action::Continue
        }
        _ => Action::Continue,
    }
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    let candidates: &[(&str, &[&str])] = match std::env::consts::OS {
        "macos" => &[("pbcopy", &[])],
        "windows" => &[("clip", &[])],
        _ => &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ],
    };
    for (program, args) in candidates {
        let Ok(mut child) = Command::new(program)
            .args(*args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        if child.wait()?.success() {
            return Ok(());
        }
    }
    bail!("no clipboard command found")
}

fn open_in_browser(url: &str) {
    let (program, args): (&str, Vec<&str>) = match std::env::consts::OS {
        "macos" => ("open", vec![url]),
        "windows" => ("cmd", vec!["/C", "start", "", url]),
        _ => ("xdg-open", vec![url]),
    };
    let _ = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
