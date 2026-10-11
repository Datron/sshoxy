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

pub const GAME_ID: &str = "tetris";
const W: usize = 10;
const H: usize = 20;

type Term = Terminal<CrosstermBackend<Stdout>>;

const SHAPES: [[[(i32, i32); 4]; 4]; 7] = [
    // I
    [
        [(0, 1), (1, 1), (2, 1), (3, 1)],
        [(2, 0), (2, 1), (2, 2), (2, 3)],
        [(0, 2), (1, 2), (2, 2), (3, 2)],
        [(1, 0), (1, 1), (1, 2), (1, 3)],
    ],
    // O
    [
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (2, 1)],
    ],
    // T
    [
        [(1, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (1, 2)],
        [(1, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // S
    [
        [(1, 0), (2, 0), (0, 1), (1, 1)],
        [(1, 0), (1, 1), (2, 1), (2, 2)],
        [(1, 1), (2, 1), (0, 2), (1, 2)],
        [(0, 0), (0, 1), (1, 1), (1, 2)],
    ],
    // Z
    [
        [(0, 0), (1, 0), (1, 1), (2, 1)],
        [(2, 0), (1, 1), (2, 1), (1, 2)],
        [(0, 1), (1, 1), (1, 2), (2, 2)],
        [(1, 0), (0, 1), (1, 1), (0, 2)],
    ],
    // J
    [
        [(0, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (2, 0), (1, 1), (1, 2)],
        [(0, 1), (1, 1), (2, 1), (2, 2)],
        [(1, 0), (1, 1), (0, 2), (1, 2)],
    ],
    // L
    [
        [(2, 0), (0, 1), (1, 1), (2, 1)],
        [(1, 0), (1, 1), (1, 2), (2, 2)],
        [(0, 1), (1, 1), (2, 1), (0, 2)],
        [(0, 0), (1, 0), (1, 1), (1, 2)],
    ],
];

struct Piece {
    kind: usize,
    rot: usize,
    x: i32,
    y: i32,
}

impl Piece {
    fn cells(&self) -> [(i32, i32); 4] {
        let base = SHAPES[self.kind][self.rot];
        std::array::from_fn(|i| (base[i].0 + self.x, base[i].1 + self.y))
    }
}

pub fn play(terminal: &mut Term, username: &str, db: &Db) -> Result<()> {
    let mut rng = rand::rng();
    let mut board = [[0u8; W]; H];
    let mut cur = fresh(&mut rng);
    let mut score: i64 = 0;
    let mut lines_done: i64 = 0;
    let mut best = db.best(username, GAME_ID).unwrap_or(0);
    let mut over = false;
    let mut last = Instant::now();

    loop {
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Left | KeyCode::Char('a') => {
                        if !over {
                            cur.x -= 1;
                            if collides(&board, &cur) {
                                cur.x += 1;
                            }
                        }
                    }
                    KeyCode::Right | KeyCode::Char('d') => {
                        if !over {
                            cur.x += 1;
                            if collides(&board, &cur) {
                                cur.x -= 1;
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('s') => {
                        if !over {
                            cur.y += 1;
                            if collides(&board, &cur) {
                                cur.y -= 1;
                            } else {
                                score += 1;
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('x') => {
                        if !over {
                            let old = cur.rot;
                            cur.rot = (cur.rot + 1) % 4;
                            if collides(&board, &cur) {
                                cur.rot = old;
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        if !over {
                            while !collides(
                                &board,
                                &Piece {
                                    y: cur.y + 1,
                                    ..cur_clone(&cur)
                                },
                            ) {
                                cur.y += 1;
                                score += 2;
                            }
                        }
                    }
                    KeyCode::Enter if over => return Ok(()),
                    _ => {}
                }
            }
        }

        let level = (lines_done / 10) + 1;
        let gravity = Duration::from_millis(700u64.saturating_sub((level as u64) * 55).max(80));
        if !over && last.elapsed() >= gravity {
            last = Instant::now();
            cur.y += 1;
            if collides(&board, &cur) {
                cur.y -= 1;
                lock(&mut board, &cur);
                let cleared = clear_lines(&mut board);
                if cleared > 0 {
                    lines_done += cleared as i64;
                    let pts = match cleared {
                        1 => 100,
                        2 => 300,
                        3 => 500,
                        _ => 800,
                    } * level;
                    score += pts;
                    best = best.max(score);
                }
                cur = fresh(&mut rng);
                if collides(&board, &cur) {
                    over = true;
                    db.save_score(username, GAME_ID, score).ok();
                }
            }
        }

        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)])
                .split(area);
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" TETRIS ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(format!(
                        " score {score}   best {best}   lines {lines_done}   lvl {level}   user {username}{}",
                        if over { " GAME OVER - Enter to leave" } else { "" }
                    )),
                ]))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL)),
                chunks[0],
            );

            let mut cells = board;
            if !over {
                for (cx, cy) in cur.cells() {
                    if cy >= 0 && (cy as usize) < H && cx >= 0 && (cx as usize) < W {
                        cells[cy as usize][cx as usize] = (cur.kind + 1) as u8;
                    }
                }
            }
            let colors = [
                Color::Black,
                Color::Cyan,
                Color::Yellow,
                Color::Magenta,
                Color::Green,
                Color::Red,
                Color::Blue,
                Color::LightYellow,
            ];
            let mut lines: Vec<Line> = Vec::new();
            for row in cells {
                let mut spans: Vec<Span> = Vec::new();
                for v in row {
                    let c = colors[(v as usize).min(colors.len() - 1)];
                    if v == 0 {
                        spans.push(Span::styled(" .", Style::default().fg(Color::DarkGray)));
                    } else {
                        spans.push(Span::styled("[]", Style::default().fg(c).add_modifier(Modifier::BOLD)));
                    }
                }
                lines.push(Line::from(spans));
            }
            f.render_widget(
                Paragraph::new(lines).alignment(Alignment::Center).block(
                    Block::default().borders(Borders::ALL).title("stack 'em"),
                ),
                chunks[1],
            );
            f.render_widget(
                Paragraph::new("←/→ move · ↓ soft · ↑ rotate · space hard drop · q quit")
                    .alignment(Alignment::Center),
                chunks[2],
            );
        })?;
    }
}

fn cur_clone(p: &Piece) -> Piece {
    Piece {
        kind: p.kind,
        rot: p.rot,
        x: p.x,
        y: p.y,
    }
}

fn fresh(rng: &mut impl Rng) -> Piece {
    Piece {
        kind: rng.random_range(0..7),
        rot: 0,
        x: 3,
        y: 0,
    }
}

fn collides(board: &[[u8; W]; H], p: &Piece) -> bool {
    for (cx, cy) in p.cells() {
        if cx < 0 || cx >= (W as i32) || cy >= (H as i32) {
            return true;
        }
        if cy >= 0 && board[cy as usize][cx as usize] != 0 {
            return true;
        }
    }
    false
}

fn lock(board: &mut [[u8; W]; H], p: &Piece) {
    for (cx, cy) in p.cells() {
        if cy >= 0 && (cy as usize) < H && cx >= 0 && (cx as usize) < W {
            board[cy as usize][cx as usize] = (p.kind + 1) as u8;
        }
    }
}

fn clear_lines(board: &mut [[u8; W]; H]) -> usize {
    let mut kept: Vec<[u8; W]> = Vec::new();
    let mut cleared = 0;
    for row in board.iter() {
        if row.iter().all(|v| *v != 0) {
            cleared += 1;
        } else {
            kept.push(*row);
        }
    }
    while kept.len() < H {
        kept.insert(0, [0; W]);
    }
    for (i, row) in kept.iter().enumerate() {
        board[i] = *row;
    }
    cleared
}
