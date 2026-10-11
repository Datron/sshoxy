use std::io::Stdout;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use crate::db::Db;

pub const GAME_ID: &str = "breakout";
const W: f64 = 30.0;
const H: f64 = 18.0;
const BRICK_COLS: usize = 10;
const BRICK_ROWS: usize = 4;
const PADDLE_W: f64 = 5.0;

type Term = Terminal<CrosstermBackend<Stdout>>;

pub fn play(terminal: &mut Term, username: &str, db: &Db) -> Result<()> {
    let mut paddle: f64 = W / 2.0;
    let mut bricks = [[true; BRICK_COLS]; BRICK_ROWS];
    let mut ball = (W / 2.0, H - 3.0);
    let mut vel = (0.45, -0.55);
    let mut stuck = true;
    let mut score: i64 = 0;
    let mut lives: i32 = 3;
    let mut best = db.best(username, GAME_ID).unwrap_or(0);
    let mut over = false;
    let mut last = Instant::now();

    loop {
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(k) = event::read()? {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Left | KeyCode::Char('a') => {
                        paddle = (paddle - 1.2).max(PADDLE_W / 2.0);
                        if stuck {
                            ball.0 = paddle;
                        }
                    }
                    KeyCode::Right | KeyCode::Char('d') => {
                        paddle = (paddle + 1.2).min(W - PADDLE_W / 2.0);
                        if stuck {
                            ball.0 = paddle;
                        }
                    }
                    KeyCode::Char(' ') => stuck = false,
                    KeyCode::Enter if over => return Ok(()),
                    _ => {}
                }
            }
        }

        if !over && !stuck && last.elapsed() >= Duration::from_millis(45) {
            last = Instant::now();
            ball.0 += vel.0;
            ball.1 += vel.1;

            if ball.0 <= 0.0 || ball.0 >= W - 1.0 {
                vel.0 = -vel.0;
                ball.0 = ball.0.clamp(0.0, W - 1.0);
            }
            if ball.1 <= 0.0 {
                vel.1 = -vel.1;
                ball.1 = 0.0;
            }
            // paddle bounce
            if vel.1 > 0.0
                && ball.1 >= H - 2.0
                && ball.1 <= H - 1.0
                && (ball.0 - paddle).abs() <= PADDLE_W / 2.0
            {
                vel.1 = -vel.1.abs();
                vel.0 = (ball.0 - paddle) * 0.18;
                ball.1 = H - 2.0;
            }
            // brick hit
            let bx = ball.0 as i32;
            let by = ball.1 as i32;
            let bw = W / (BRICK_COLS as f64);
            if by >= 1 && by < 1 + (BRICK_ROWS as i32) {
                let row = (by - 1) as usize;
                let col = ((bx as f64) / bw) as usize;
                if row < BRICK_ROWS && col < BRICK_COLS && bricks[row][col] {
                    bricks[row][col] = false;
                    vel.1 = -vel.1;
                    score += 25;
                    best = best.max(score);
                }
            }
            // lost ball
            if ball.1 >= H {
                lives -= 1;
                if lives <= 0 {
                    over = true;
                    db.save_score(username, GAME_ID, score).ok();
                } else {
                    stuck = true;
                    ball = (paddle, H - 3.0);
                    vel = (0.45, -0.55);
                }
            }
            if bricks.iter().all(|r| r.iter().all(|b| !b)) {
                score += 250;
                best = best.max(score);
                db.save_score(username, GAME_ID, score).ok();
                over = true;
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
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        " BREAKOUT ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        " score {score}   best {best}   lives {lives}   user {username} {}",
                        if over {
                            "CLEARED/GAME OVER - Enter to leave"
                        } else if stuck {
                            "SPACE to launch"
                        } else {
                            ""
                        }
                    )),
                ]))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL)),
                chunks[0],
            );

            let mut lines: Vec<Line> = Vec::new();
            let bw = W / (BRICK_COLS as f64);
            for y in 0..(H as i32) {
                let mut row = String::new();
                for x in 0..(W as i32) {
                    let mut ch = "  ";
                    // bricks
                    if y >= 1 && y < 1 + (BRICK_ROWS as i32) {
                        let col = ((x as f64) / bw) as usize;
                        let r = (y - 1) as usize;
                        if r < BRICK_ROWS && col < BRICK_COLS && bricks[r][col] {
                            ch = "##";
                        }
                    }
                    if (ball.0.round() as i32) == x && (ball.1.round() as i32) == y {
                        ch = "()";
                    }
                    if y == (H as i32) - 1 && ((x as f64) - paddle).abs() <= PADDLE_W / 2.0 {
                        ch = "==";
                    }
                    row.push_str(ch);
                }
                let color = if y < 1 + (BRICK_ROWS as i32) {
                    Color::Red
                } else {
                    Color::Blue
                };
                lines.push(Line::from(Span::styled(row, Style::default().fg(color))));
            }
            f.render_widget(
                Paragraph::new(lines).alignment(Alignment::Center).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("smash every ##"),
                ),
                chunks[1],
            );
            f.render_widget(
                Paragraph::new("←/→ or A/D move · space launch · q quit")
                    .alignment(Alignment::Center),
                chunks[2],
            );
        })?;
    }
}
