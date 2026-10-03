# sshoxy

Cargo workspace for the `sshoxy` crates, versioned with [Cocogitto](https://github.com/cocogitto/cocogitto) (`cog`).

## Layout

```text
.
├── Cargo.toml            # virtual workspace, members = ["crates/*"]
├── cog.toml              # conventional-commit versioning / monorepo config
├── rust-toolchain.toml   # pinned toolchain 1.97.0
├── crates/
│   └── sshoxy/           # initial binary crate (from `cargo init --name sshoxy`)
└── README.md
```

## Prerequisites

- Rust toolchain `1.97.0` (auto-installed via `rustup` from `rust-toolchain.toml`)
- `cog` for versioning: `cargo install --locked cocogitto`
- `cargo-set-version` for monorepo bump hooks: `cargo install cargo-edit`

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all

# conventional commits
cog commit feat "add awesome feature"
cog check
cog changelog

# versioning (monorepo, Cargo resolver)
cog bump --auto
cog bump --package sshoxy --auto
```

## Adding a crate

```sh
cargo new crates/<name> --name <name>
# then register in cog.toml:
# [monorepo.packages.<name>]
# path = "crates/<name>"
```

Workspace inherits `version`, `edition`, `rust-version`, `license` from `[workspace.package]`.
Keep `cog.toml` package paths in sync with `crates/*`.

## License

MIT, see [LICENSE-MIT](LICENSE-MIT).
