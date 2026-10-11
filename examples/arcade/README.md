# sshoxy arcade

Demo TUI app built for [sshoxy](../../README.md) — an SSH server that
runs one program per login. Five games, one leaderboard, zero mercy.

## Games

| # | Game | Keys |
|---|------|------|
| 1 | Snake | arrows/WASD move · `p` pause |
| 2 | Alien Destroyer | `←/→`/`A/D` move · `space` fire |
| 3 | Breakout | `←/→`/`A/D` paddle · `space` launch |
| 4 | Tetris | `←/→` move · `↓` soft · `↑` rotate · `space` hard drop |
| 5 | Battleship | arrows/WASD aim · `Enter` fire |
| 6 | Leaderboard | `←/→` switch game |
| 7 | Exit | — |

`q` / `Esc` quits any screen. Scores save on game over / win.

## Scores

SQLite leaderboard, one row per finished game:

- keyed by login username (`SSHOXY_USER`, falls back to `USER`, then `player`)
- file: OS data dir (`~/.local/share/sshoxy/arcade/leaderboard.db` on Linux)
- override: `SSHOXY_ARCADE_DB=/tmp/scores.db`

## Run locally

```sh
cargo run -p arcade
```

## Serve over SSH with sshoxy

```sh
# passwordless demo (any username works) on port 2222
cargo run -p sshoxy -- -c 'cargo run -p arcade' --no-auth
ssh -p 2222 player@localhost
```

With public-key auth (default), each SSH username gets its own
leaderboard identity automatically via `SSHOXY_USER`.
