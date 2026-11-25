# AGENTS.md

Monorepo root. Each project is self-contained in its own directory, with its own `AGENTS.md`,
README, docs and tests. **Read the project's AGENTS.md before working in it.**

| Project | Agent instructions |
|---|---|
| `wc2bne-linux/` | [wc2bne-linux/AGENTS.md](wc2bne-linux/AGENTS.md) |
| `openbnet/` | [openbnet/AGENTS.md](openbnet/AGENTS.md) |
| `wc2bne-wasm/` | [wc2bne-wasm/AGENTS.md](wc2bne-wasm/AGENTS.md) |

- Keep everything specific to one project inside its directory. Only monorepo-wide files belong here.
- `.vendor/` is git-ignored, shared third-party material. Don't edit or commit it.
- Other top-level directories are placeholders; leave them alone until the owner defines them.
- Never commit game files, the installer, Wine environments, server data or keys. Licensed GPL-3.0-or-later.

## Agent skills

### Issue tracker

Local markdown files under `.scratch/`. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default labels. See `docs/agents/triage-labels.md`.

### Domain docs

Multi-context: `GLOSSARY-MAP.md` and one glossary per project. See `docs/agents/domain.md`.
