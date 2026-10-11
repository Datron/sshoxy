//! sshoxy arcade — demo TUI app meant to run over ssh via sshoxy.
//!
//! Run locally:  `cargo run -p arcade`
//! Serve over ssh:
//!   `cargo run -p sshoxy -- -c 'cargo run -p arcade'`
//!   `ssh -p 2222 <user>@<host>`
//!
//! High scores are stored per-login-username in sqlite. sshoxy exports
//! the ssh login name as SSHOXY_USER.

mod db;
mod games;

use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

use db::{username, Db};

type Term = Terminal<CrosstermBackend<Stdout>>;

const MENU: [(&str, &str); 7] = [
    ("snake", "Snake — eat, grow, don't crash"),
    ("alien-destroyer", "Alien Destroyer — space invaders"),
    ("breakout", "Breakout — bounce the ball, smash bricks"),
    ("tetris", "Tetris — stack the falling blocks"),
    ("battleship", "Battleship — sink the AI fleet"),
    ("leaderboard", "Leaderboard — top pilots per game"),
    ("exit", "Exit arcade"),
];

const GAME_IDS: [&str; 5] = [
    "snake",
    "alien-destroyer",
    "breakout",
    "tetris",
    "battleship",
];

fn main() -> Result<()> {
    let user = username();
    let db = Db::open()?;
    let mut terminal = setup()?;
    let res = menu_loop(&mut terminal, &user, &db);
    restore()?;
    res
}

fn setup() -> Result<Term> {
    enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(out))?)
}

fn restore() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn menu_loop(terminal: &mut Term, user: &str, db: &Db) -> Result<()> {
    let mut state = ListState::default();
    state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ])
                .split(area);

            let banner = Paragraph::new(vec![
                Line::from(Span::styled(
                    "★ SSHOXY ARCADE ★",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::raw(format!(
                    "welcome, {user}! pick a game. scores save to your name."
                ))),
            ])
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(banner, chunks[0]);

            let items: Vec<ListItem> = MENU
                .iter()
                .map(|(id, desc)| {
                    let best = if *id == "leaderboard" || *id == "exit" {
                        String::new()
                    } else {
                        match db.best(user, id) {
                            Ok(b) => format!("  (your best: {b})"),
                            Err(_) => String::new(),
                        }
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!(" {desc}"), Style::default().fg(Color::White)),
                        Span::styled(best, Style::default().fg(Color::DarkGray)),
                    ]))
                })
                .collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("games"))
                .highlight_symbol("▶ ")
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                );
            f.render_stateful_widget(list, chunks[1], &mut state);

            f.render_widget(
                Paragraph::new("↑/↓ choose · Enter play · q quit")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL)),
                chunks[2],
            );
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = state.selected().unwrap_or(0);
                        state.select(Some(i.saturating_sub(1)));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = state.selected().unwrap_or(0);
                        state.select(Some((i + 1).min(MENU.len() - 1)));
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        let i = state.selected().unwrap_or(0);
                        let id = MENU.get(i).map(|m| m.0).unwrap_or("exit");
                        match id {
                            "exit" => break,
                            "leaderboard" => show_leaderboard(terminal, db)?,
                            "snake" => games::snake::play(terminal, user, db)?,
                            "alien-destroyer" => games::invaders::play(terminal, user, db)?,
                            "breakout" => games::breakout::play(terminal, user, db)?,
                            "tetris" => games::tetris::play(terminal, user, db)?,
                            "battleship" => games::battleship::play(terminal, user, db)?,
                            _ => {}
                        }
                        // re-open db handle state is cheap; keep same conn
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn show_leaderboard(terminal: &mut Term, db: &Db) -> Result<()> {
    let mut tab: usize = 0;
    loop {
        let game = GAME_IDS.get(tab).copied().unwrap_or(GAME_IDS[0]);
        let rows = db.top_for_game(game, 10).unwrap_or_default();

        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(2),
                ])
                .split(area);

            let mut tabs: Vec<Span> = Vec::new();
            for (i, g) in GAME_IDS.iter().enumerate() {
                let style = if i == tab {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                tabs.push(Span::styled(format!(" {g} "), style));
                tabs.push(Span::raw(" "));
            }
            f.render_widget(
                Paragraph::new(Line::from(tabs))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title("LEADERBOARD")),
                chunks[0],
            );

            let mut lines: Vec<Line> = Vec::new();
            if rows.is_empty() {
                lines.push(Line::from(Span::styled(
                    "no scores yet — go play!",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            for (rank, r) in rows.iter().enumerate() {
                let medal = match rank {
                    0 => "🥇",
                    1 => "🥈",
                    2 => "🥉",
                    _ => "  ",
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{medal} #{:>2} ", rank + 1),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!("{:<16}", r.username),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{:>7}", r.score), Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("   {}", r.played_at),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
            f.render_widget(
                Paragraph::new(lines).alignment(Alignment::Center).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("top pilots — {game}")),
                ),
                chunks[1],
            );
            f.render_widget(
                Paragraph::new("←/→ switch game · q/Esc back").alignment(Alignment::Center),
                chunks[2],
            );
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => break,
                    KeyCode::Left | KeyCode::Char('h') => {
                        tab = if tab == 0 {
                            GAME_IDS.len() - 1
                        } else {
                            tab - 1
                        };
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        tab = (tab + 1) % GAME_IDS.len();
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
