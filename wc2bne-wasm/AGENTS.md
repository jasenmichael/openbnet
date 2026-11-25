# AGENTS.md

Instructions for coding agents working on wc2bne-wasm. Paths are relative to this directory.
Read [docs/PLAN.md](docs/PLAN.md) first and [docs/RESEARCH.md](docs/RESEARCH.md) before changing engine or networking.
Terms are in [../wc2bne-linux/GLOSSARY.md](../wc2bne-linux/GLOSSARY.md).

## Rules

1. Run the original client unmodified. Never patch or replace the exe, `storm.dll` or `battle.snp`, and never
   recreate the game's code or art.
2. Never host, commit or upload game files, the installer, CD keys or account data. Players supply the installer;
   nothing unpacks unless its SHA-256 matches the one in `wc2bne-linux`.
3. Game data stays in the browser. The relay stores no game files or passwords and never logs payloads.
4. The relay must not be an open proxy: see "Relay rules" in the plan. Test each rule.
5. Test against a local PvPGN or openbnet on an isolated network, tracking off. Public servers only by the owner.
6. Don't use `../wc2bne-linux/data` in tests. Use a throwaway copy.
7. Work test-first. Say which browsers, editions and servers were really tested.
8. Update RESEARCH.md when a finding is confirmed or disproved.
