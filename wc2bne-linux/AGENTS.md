# AGENTS.md

Instructions for coding agents working on wc2bne-linux. Paths are relative to this directory. Humans: see [README.md](README.md).

## What this is

A Linux launcher for the **unmodified** Windows build of Warcraft II: Battle.net Edition
(War2Combat 4.6.1). It must stay compatible with Battle.net and PvPGN. It is Bash: `wc2bne` plus
`lib/`, using Wine + bubblewrap + nested Xwayland.

The owner's original request was "reverse engineer this for Linux", using
[REA](https://github.com/morluto/rea). The reverse engineering was used to configure the original
client correctly. There is no decompiled, patched or native game code here.

## Read first

| File | When |
|---|---|
| [GLOSSARY.md](GLOSSARY.md) | Always; use these terms |
| [docs/SPEC.md](docs/SPEC.md) | Before changing behavior: commands, settings, exit codes, security rules |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Before replacing an approach; lists what already failed |
| [docs/REVERSE_ENGINEERING.md](docs/REVERSE_ENGINEERING.md) | Before touching gateways, ports, versions or networking |
| [docs/TESTING.md](docs/TESTING.md) | Before claiming something works |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Before starting something new |

## Layout

| Path | Responsibility |
|---|---|
| `wc2bne` | Loads `lib/` and calls `main` |
| `lib/cli.sh` | Arguments, environment, commands |
| `lib/help.sh` | Help text |
| `lib/config.sh` | Data folder and settings |
| `lib/editions.sh` | Editions, paths, shared maps, file check |
| `lib/installer.sh` | Finding, downloading, checking, running the installer |
| `lib/gateways.sh`, `lib/registry.sh` | Gateway lists; registry access |
| `lib/sandbox.sh` | The sandbox |
| `lib/display.sh` | Nested display; running the game |
| `lib/deps.sh` | Dependencies |
| `lib/lock.sh` | One user at a time per edition |
| `test/*.bats`, `test/stubs/`, `test/helper.bash` | Tests and stand-ins |
| `data/` | **The owner's real data folder.** Git-ignored. Don't test in it; set `WC2BNE_HOME` |

`../.vendor/` is shared third-party material. Leave it alone.

## Commands

No build step. bats is a development-only dependency.

```sh
bash -n wc2bne lib/*.sh                     # syntax check
bats test                                   # or: npx bats test; offline, takes seconds
WC2BNE_HOME=/some/scratch/dir ./wc2bne install                # real throwaway install (~1.4 GB)
WC2BNE_HOME=/some/scratch/dir WC2BNE_OFFLINE=1 ./wc2bne play --window
```

## Code conventions

- Functions take arguments and print results. Those marked "pure" touch no files, processes or environment.
- Keep calculations apart from side effects. Only `cli.sh` reads `WC2BNE_` variables, except `WC2BNE_OFFLINE` in `box`.
- Each function has a comment: arguments, `->` output, side effects.
- Behavior must stay describable without Bash, so it could be rewritten in another language.

## Rules

1. **Never modify, patch or replace** `Warcraft II BNE.exe`, `storm.dll` or `battle.snp`. The
   server's version check covers them. Expected hashes are in docs/REVERSE_ENGINEERING.md.
2. **Never run the client outside the sandbox** or widen it (home directory, host X socket on
   Wayland, D-Bus, `Z:` drive) without the owner's agreement. It executes server-supplied code.
3. **Never commit or upload** game files, the installer, Wine prefixes, CD keys, account data or
   decompiler output. A prefix registry holds per-client Battle.net identifiers.
4. Don't run `War2noCDloader.exe`, `War2Ppatcher4.exe` or the key changer.
5. Don't connect test clients to public servers. Use a local PvPGN on an internal-only network with
   tracking off. Ask before building or running containers.
6. Keep `wineserver -w` at the end of `box`; without it registry changes are lost.
7. Keep `set -euo pipefail`; no new runtime dependency unless needed. Never run `sudo` without a yes.
8. Work test-first: a failing test through the command line, then the code. Agree new test boundaries
   with the owner. Tests must never use the real network, Wine,
   sandbox or data folder.
9. Never run an installer whose checksum doesn't match. `WC2BNE_SHA256` exists for tests.
10. Report what was tested and what wasn't. A headless result doesn't prove it works on a real desktop.

## Known problems

- In `--window`, if the compositor tiles or resizes the window, the game stays 640x480, top-left.
- bubblewrap sees only `/usr` and `/etc`, so Wine installed in `/opt` (WineHQ packages) won't work.

## Keeping docs current

Update SPEC.md when behavior changes, TESTING.md when something is verified or breaks, and append
to DECISIONS.md when an approach is chosen or rejected.
