# wc2bne-wasm

Warcraft II: Battle.net Edition in a browser tab: the original, unmodified Windows client under an x86
emulator compiled to WebAssembly. Each player's game runs, and is saved, in their own browser.

**Status: planning. Nothing runs yet.** See [docs/PLAN.md](docs/PLAN.md) and [docs/RESEARCH.md](docs/RESEARCH.md).

The site contains no game files. Players supply the War2Combat 4.6.1 installer, which is checked by SHA-256
and unpacked in their browser. Warcraft II belongs to Blizzard Entertainment; this project is unofficial.
