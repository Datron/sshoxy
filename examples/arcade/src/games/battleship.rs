use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use rand::{seq::SliceRandom, Rng};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use crate::db::Db;

pub const GAME_ID: &str = "battleship";
const N: usize = 8;
const SHIPS: [usize; 4] = [4, 3, 3, 2];

type Term = Terminal<CrosstermBackend<Stdout>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Empty,
    Ship,
    Hit,
    Miss,
}

struct Board {
    cells: [[Cell; N]; N],
    ships_alive: usize,
}

impl Board {
    fn new_random(rng: &mut impl Rng) -> Self {
        let mut b = Self {
            cells: [[Cell::Empty; N]; N],
            ships_alive: SHIPS.iter().sum(),
        };
        for len in SHIPS {
            'place: loop {
                let horiz: bool = rng.random();
                let x: usize = rng.random_range(0..N);
                let y: usize = rng.random_range(0..N);
                let mut pts = Vec::new();
                for i in 0..len {
                    let (cx, cy) = if horiz { (x + i, y) } else { (x, y + i) };
                    if cx >= N || cy >= N || b.cells[cy][cx] != Cell::Empty {
                        continue 'place;
                    }
                    pts.push((cx, cy));
                }
                // no-adjacency rule
                for (cx, cy) in &pts {
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let nx = (*cx as i32) + dx;
                            let ny = (*cy as i32) + dy;
                            if nx >= 0
                                && ny >= 0
                                && (nx as usize) < N
                                && (ny as usize) < N
                                && b.cells[ny as usize][nx as usize] == Cell::Ship
                            {
                                continue 'place;
                            }
                        }
                    }
                }
                for (cx, cy) in pts {
                    b.cells[cy][cx] = Cell::Ship;
                }
                break;
            }
        }
        b
    }

    /// Returns true if it was a hit.
    fn fire(&mut self, x: usize, y: usize) -> Option<bool> {
        match self.cells[y][x] {
            Cell::Ship => {
                self.cells[y][x] = Cell::Hit;
                self.ships_alive -= 1;
                Some(true)
            }
            Cell::Empty => {
                self.cells[y][x] = Cell::Miss;
                Some(false)
            }
            _ => None,
        }
    }
}

pub fn play(terminal: &mut Term, username: &str, db: &Db) -> Result<()> {
    let mut rng = rand::rng();
    let mut enemy = Board::new_random(&mut rng);
    let mut player = Board::new_random(&mut rng);
    let mut cursor = (0usize, 0usize);
    let mut ai_targets: Vec<(usize, usize)> =
        (0..N).flat_map(|y| (0..N).map(move |x| (x, y))).collect();
    ai_targets.shuffle(&mut rng);
    let mut shots: i64 = 0;
    let mut hits: i64 = 0;
    let mut best = db.best(username, GAME_ID).unwrap_or(0);
    let mut status = String::from("sink every ship! Enter fires.");
    let mut over: Option<bool> = None; // Some(true) = won

    loop {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Enter if over.is_some() => return Ok(()),
                    _ if over.is_some() => {}
                    KeyCode::Up | KeyCode::Char('w') => cursor.1 = cursor.1.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('s') => cursor.1 = (cursor.1 + 1).min(N - 1),
                    KeyCode::Left | KeyCode::Char('a') => cursor.0 = cursor.0.saturating_sub(1),
                    KeyCode::Right | KeyCode::Char('d') => cursor.0 = (cursor.0 + 1).min(N - 1),
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        match enemy.fire(cursor.0, cursor.1) {
                            Some(true) => {
                                shots += 1;
                                hits += 1;
                                status = format!(
                                    "HIT at {},{}! AI thinking…",
                                    cursor.0 + 1,
                                    cursor.1 + 1
                                );
                            }
                            Some(false) => {
                                shots += 1;
                                status = format!(
                                    "miss at {},{}. AI thinking…",
                                    cursor.0 + 1,
                                    cursor.1 + 1
                                );
                            }
                            None => {
                                status = String::from("already fired there.");
                                continue;
                            }
                        }
                        if enemy.ships_alive == 0 {
                            let score = score_for(shots, true);
                            best = best.max(score);
                            db.save_score(username, GAME_ID, score).ok();
                            status =
                                format!("YOU WIN in {shots} shots! score {score}. Enter to leave.");
                            over = Some(true);
                            continue;
                        }
                        // AI shot
                        let mut ai_msg = String::from("AI missed.");
                        while let Some((ax, ay)) = ai_targets.pop() {
                            match player.fire(ax, ay) {
                                Some(true) => {
                                    ai_msg = format!("AI HIT your ship at {},{}!", ax + 1, ay + 1);
                                    break;
                                }
                                Some(false) => {
                                    ai_msg = String::from("AI missed.");
                                    break;
                                }
                                None => continue,
                            }
                        }
                        if player.ships_alive == 0 {
                            let score = score_for(shots, false);
                            best = best.max(score);
                            db.save_score(username, GAME_ID, score).ok();
                            status = format!(
                                "DEFEAT. AI sank your fleet. score {score}. Enter to leave."
                            );
                            over = Some(false);
                        } else if over.is_none() {
                            status = format!(
                                "{} {}",
                                if hits > 0 {
                                    "keep going."
                                } else {
                                    "keep hunting."
                                },
                                ai_msg
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
                .split(area);
            let score_now = score_for(shots.max(1), over == Some(true));
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" BATTLESHIP ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    Span::raw(format!(
                        " hits {hits}   shots {shots}   score {score_now}   best {best}   user {username}"
                    )),
                ]))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL)),
                chunks[0],
            );

            let mid = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            let enemy_lines = board_lines(&enemy, Some(cursor), false);
            let player_lines = board_lines(&player, None, true);
            f.render_widget(
                Paragraph::new(enemy_lines).alignment(Alignment::Center).block(
                    Block::default().borders(Borders::ALL).title("ENEMY (fire here)"),
                ),
                mid[0],
            );
            f.render_widget(
                Paragraph::new(player_lines).alignment(Alignment::Center).block(
                    Block::default().borders(Borders::ALL).title("YOUR FLEET"),
                ),
                mid[1],
            );

            f.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        status.clone(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::raw("arrows/WASD aim · Enter fire · q quit")),
                ])
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL)),
                chunks[2],
            );
        })?;
    }
}

fn score_for(shots: i64, won: bool) -> i64 {
    let base = if won { 1500 } else { 300 };
    (base - shots * 15).max(10)
}

fn board_lines(b: &Board, cursor: Option<(usize, usize)>, reveal: bool) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut head = String::from("  ");
    for x in 0..N {
        head.push_str(&format!("{} ", x + 1));
    }
    lines.push(Line::from(Span::styled(
        head,
        Style::default().fg(Color::DarkGray),
    )));
    for y in 0..N {
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(
            format!("{} ", (b'A' + (y as u8)) as char),
            Style::default().fg(Color::DarkGray),
        ));
        for x in 0..N {
            let is_cur = cursor == Some((x, y));
            let (txt, color) = match b.cells[y][x] {
                Cell::Hit => ("XX", Color::Red),
                Cell::Miss => ("··", Color::DarkGray),
                Cell::Ship if reveal => ("SS", Color::Green),
                Cell::Ship => ("~~", Color::Blue),
                Cell::Empty => ("~~", Color::Blue),
            };
            let mut style = Style::default().fg(color);
            if is_cur {
                style = style
                    .bg(Color::White)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(txt.to_string(), style));
        }
        lines.push(Line::from(spans));
    }
    lines
}
