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

pub const GAME_ID: &str = "alien-destroyer";
const W: i32 = 26;
const H: i32 = 16;
const COLS: usize = 8;
const ROWS: usize = 3;

type Term = Terminal<CrosstermBackend<Stdout>>;

#[derive(Clone, Copy)]
struct Alien {
    x: i32,
    y: i32,
    alive: bool,
}

pub fn play(terminal: &mut Term, username: &str, db: &Db) -> Result<()> {
    let mut rng = rand::rng();
    let mut ship_x: i32 = W / 2;
    let mut bullets: Vec<(i32, i32)> = Vec::new();
    let mut bombs: Vec<(i32, i32)> = Vec::new();
    let mut aliens: Vec<Alien> = Vec::new();
    reset_wave(&mut aliens, 0);
    let mut fleet_dir: i32 = 1;
    let mut score: i64 = 0;
    let mut lives: i32 = 3;
    let mut wave: i32 = 1;
    let mut best = db.best(username, GAME_ID).unwrap_or(0);
    let mut over = false;
    let mut won_wave = false;
    let mut last = Instant::now();
    let mut last_fleet = Instant::now();
    let mut last_bomb = Instant::now();

    loop {
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Left | KeyCode::Char('a') => ship_x = (ship_x - 1).max(0),
                    KeyCode::Right | KeyCode::Char('d') => ship_x = (ship_x + 1).min(W - 1),
                    KeyCode::Char(' ') => {
                        if !over && bullets.len() < 4 {
                            bullets.push((ship_x, H - 2));
                        }
                    }
                    KeyCode::Enter if over => return Ok(()),
                    _ => {}
                }
            }
        }

        if !over && last.elapsed() >= Duration::from_millis(40) {
            last = Instant::now();
            for b in bullets.iter_mut() {
                b.1 -= 1;
            }
            bullets.retain(|b| b.1 >= 0);
            for b in bombs.iter_mut() {
                b.1 += 1;
            }
            bombs.retain(|b| b.1 < H);

            // bullets vs aliens
            for b in bullets.iter_mut() {
                for a in aliens.iter_mut().filter(|a| a.alive) {
                    if a.x == b.0 && a.y == b.1 {
                        a.alive = false;
                        b.1 = -99; // consume
                        score += 50;
                        best = best.max(score);
                        break;
                    }
                }
            }
            bullets.retain(|b| b.1 >= 0);

            // bombs vs ship
            if bombs.iter().any(|b| b.0 == ship_x && b.1 == H - 1) {
                lives -= 1;
                bombs.clear();
                if lives <= 0 {
                    over = true;
                    db.save_score(username, GAME_ID, score).ok();
                }
            }
        }

        // fleet movement
        let fleet_speed = Duration::from_millis(420u64.saturating_sub((wave as u64) * 30).max(120));
        if !over && last_fleet.elapsed() >= fleet_speed {
            last_fleet = Instant::now();
            let mut edge = false;
            for a in aliens.iter().filter(|a| a.alive) {
                if (a.x + fleet_dir) < 0 || (a.x + fleet_dir) >= W {
                    edge = true;
                    break;
                }
            }
            if edge {
                fleet_dir = -fleet_dir;
                for a in aliens.iter_mut().filter(|a| a.alive) {
                    a.y += 1;
                }
            } else {
                for a in aliens.iter_mut().filter(|a| a.alive) {
                    a.x += fleet_dir;
                }
            }
            if aliens.iter().filter(|a| a.alive).any(|a| a.y >= H - 1) {
                over = true;
                db.save_score(username, GAME_ID, score).ok();
            }
            if !aliens.iter().any(|a| a.alive) {
                won_wave = true;
            }
        }

        if won_wave {
            wave += 1;
            score += 200;
            best = best.max(score);
            reset_wave(&mut aliens, wave);
            won_wave = false;
        }

        // alien bombing
        if !over && last_bomb.elapsed() >= Duration::from_millis(700) {
            last_bomb = Instant::now();
            let alive: Vec<Alien> = aliens.iter().filter(|a| a.alive).copied().collect();
            if !alive.is_empty() {
                let pick = &alive[rng.random_range(0..alive.len())];
                bombs.push((pick.x, pick.y + 1));
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
            let title = if over {
                format!(" GAME OVER - wave {wave} - Enter to leave ")
            } else {
                format!(
                    " score {score}   best {best}   lives {lives}   wave {wave}   user {username} "
                )
            };
            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    " ALIEN DESTROYER ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(title),
            ]))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            let mut lines: Vec<Line> = Vec::new();
            for y in 0..H {
                let mut row = String::new();
                for x in 0..W {
                    let alien_here = aliens.iter().any(|a| a.alive && a.x == x && a.y == y);
                    let bullet_here = bullets.iter().any(|b| b.0 == x && b.1 == y);
                    let bomb_here = bombs.iter().any(|b| b.0 == x && b.1 == y);
                    if alien_here {
                        row.push_str("<>");
                    } else if bullet_here {
                        row.push_str("||");
                    } else if bomb_here {
                        row.push_str("**");
                    } else if y == H - 1 && x == ship_x {
                        row.push_str("/\\");
                    } else if y == H - 1 && (x == ship_x - 1 || x == ship_x + 1) {
                        row.push_str("--");
                    } else {
                        row.push_str(" .");
                    }
                }
                lines.push(Line::from(Span::styled(
                    row,
                    Style::default().fg(Color::Cyan),
                )));
            }
            f.render_widget(
                Paragraph::new(lines).alignment(Alignment::Center).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("kill every <>"),
                ),
                chunks[1],
            );
            f.render_widget(
                Paragraph::new("←/→ or A/D move · space fire · q quit")
                    .alignment(Alignment::Center),
                chunks[2],
            );
        })?;
    }
}

fn reset_wave(aliens: &mut Vec<Alien>, wave: i32) {
    aliens.clear();
    let x0 = 4 + (wave % 4);
    for r in 0..ROWS {
        for c in 0..COLS {
            aliens.push(Alien {
                x: x0 + (c as i32) * 2,
                y: 1 + (r as i32),
                alive: true,
            });
        }
    }
}
