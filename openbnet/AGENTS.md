# AGENTS.md

Instructions for coding agents working on openbnet. Paths are relative to this directory.

A Rust server that real Blizzard games connect to, based on PvPGN (`../.vendor/pvpgn-server`, read-only).
No PHP, Lua or Westwood protocols. Terms are in [GLOSSARY.md](GLOSSARY.md), decisions in [docs/adr](docs/adr),
what comes next in [docs/ROADMAP.md](docs/ROADMAP.md). For people running a server:
[docs/CONFIGURATION.md](docs/CONFIGURATION.md) and [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).

## Layout

| Path | What |
|---|---|
| `src/server.rs` | Ports, accepting connections, storing finished games |
| `src/net.rs`, `src/auth.rs` | What all chat connections share; signing up and logging in |
| `src/bncs.rs` | The games' binary protocol |
| `src/text.rs`, `src/irc.rs`, `src/http.rs` | Bot and telnet, IRC, HTTP and WebSocket ([docs/API.md](docs/API.md)) |
| `src/hub.rs` | Sessions, channels and games, behind one lock |
| `src/command.rs`, `src/ladder.rs` | Chat commands; ratings |
| `src/files.rs` | BNFTP and which files may be served |
| `src/udp.rs` | Where players' games can be reached |
| `src/store.rs`, `migrations/` | Database |
| `src/xsha1.rs`, `src/versions.rs`, `src/wire.rs` | Password hash, version checks, packet fields |
| `assets/` | Built into the program: default settings and PvPGN's version list |
| `data/` | Git-ignored. The owner's running server lives here: don't change it or test in it; use `--data` |
| `tests/` | Simulated clients, comparison with PvPGN, load |

## Commands

```sh
cargo test                                  # about 25 s
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The comparison with PvPGN and the load tests are in [docs/TESTING.md](docs/TESTING.md).

## Rules

1. Match what PvPGN sends, unless an ADR says otherwise. When PvPGN and a real game disagree, the game wins.
2. Work test-first, with simulated clients over sockets. Change `tests/reference.rs` when adding packets.
3. Never hold the hub's lock across an `await`, and don't do slow work under it. Never cancel a request half-way (in a
   `select!` arm or a timeout): an abandoned query costs the database connection.
4. Everything a client sends is untrusted: names, addresses, results and counts included. No `unsafe`, and no
   panics on input.
5. Text from one protocol must not add lines or packets to another: use `net::one_line`.
6. A password hash is enough to log in with. Never log one, or send one to a client.
7. Migrations must run on SQLite and PostgreSQL. Don't edit one that has been released; add another.
8. Never commit Blizzard's files.
9. Run PvPGN and real games only on an isolated network, with PvPGN's tracking off and public gateways removed.
   Use a throwaway `WC2BNE_HOME`, and stop processes by the PIDs you started.
10. A new or changed setting goes in `assets/openbnet.toml` and docs/CONFIGURATION.md; a test checks that it is listed.
11. Say which games were tested for real. Simulated clients prove only the protocol.
