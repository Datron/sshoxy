use std::collections::VecDeque;
use std::io::Stdout;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use rand::Rng;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use crate::db::Db;

pub const GAME_ID: &str = "snake";
const W: i32 = 28;
const H: i32 = 18;

type Term = Terminal<CrosstermBackend<Stdout>>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Pt {
    x: i32,
    y: i32,
}

pub fn play(terminal: &mut Term, username: &str, db: &Db) -> Result<()> {
    let mut rng = rand::rng();
    let mut snake: VecDeque<Pt> = VecDeque::new();
    snake.push_back(Pt { x: 8, y: 9 });
    snake.push_back(Pt { x: 7, y: 9 });
    snake.push_back(Pt { x: 6, y: 9 });
    let mut dir = Pt { x: 1, y: 0 };
    let mut pending = dir;
    let mut food = Pt { x: 18, y: 9 };
    let mut score: i64 = 0;
    let mut best = db.best(username, GAME_ID).unwrap_or(0);
    let mut over = false;
    let mut paused = false;
    let mut last = Instant::now();

    loop {
        let speed = tick_for(score);
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('p') | KeyCode::Char(' ') => paused = !paused,
                    KeyCode::Up | KeyCode::Char('w') if dir.y == 0 => pending = Pt { x: 0, y: -1 },
                    KeyCode::Down | KeyCode::Char('s') if dir.y == 0 => pending = Pt { x: 0, y: 1 },
                    KeyCode::Left | KeyCode::Char('a') if dir.x == 0 => {
                        pending = Pt { x: -1, y: 0 }
                    }
                    KeyCode::Right | KeyCode::Char('d') if dir.x == 0 => {
                        pending = Pt { x: 1, y: 0 }
                    }
                    KeyCode::Enter if over => return Ok(()),
                    _ => {}
                }
            }
        }

        if !paused && !over && last.elapsed() >= speed {
            last = Instant::now();
            dir = pending;
            let head = *snake.front().unwrap_or(&Pt { x: 0, y: 0 });
            let next = Pt {
                x: head.x + dir.x,
                y: head.y + dir.y,
            };
            let hit_wall = next.x < 0 || next.y < 0 || next.x >= W || next.y >= H;
            let hit_self = snake.iter().any(|p| *p == next);
            if hit_wall || hit_self {
                over = true;
                db.save_score(username, GAME_ID, score).ok();
                best = best.max(score);
            } else {
                snake.push_front(next);
                if next == food {
                    score += 10;
                    best = best.max(score);
                    loop {
                        food = Pt {
                            x: rng.random_range(0..W),
                            y: rng.random_range(0..H),
                        };
                        if !snake.iter().any(|p| *p == food) {
                            break;
                        }
                    }
                } else {
                    snake.pop_back();
                }
            }
        }

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
            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    " SNAKE ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" score {score}   best {best}   user {username} ")),
                Span::styled(
                    if over {
                        "GAME OVER - Enter to leave"
                    } else if paused {
                        "PAUSED (p)"
                    } else {
                        ""
                    },
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ]))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let mut lines: Vec<Line> = Vec::new();
            for y in 0..H {
                let mut row = String::with_capacity((W as usize) * 2);
                for x in 0..W {
                    let p = Pt { x, y };
                    if Some(&p) == snake.front() {
                        row.push_str("██");
                    } else if snake.iter().any(|s| *s == p) {
                        row.push_str("[]");
                    } else if p == food {
                        row.push_str("()");
                    } else {
                        row.push_str(" .");
                    }
                }
                let style = if (y % 2) == 0 {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                lines.push(Line::from(Span::styled(row, style)));
            }
            let board = Paragraph::new(lines).alignment(Alignment::Center).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("feed () to grow"),
            );
            f.render_widget(board, chunks[1]);

            let help =
                Paragraph::new("arrows/WASD move · p pause · q quit").alignment(Alignment::Center);
            f.render_widget(help, chunks[2]);
        })?;
    }
}

fn tick_for(score: i64) -> Duration {
    let ms: u64 = 170u64.saturating_sub((score / 10) as u64 * 6).max(60);
    Duration::from_millis(ms)
}
