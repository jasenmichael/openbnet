# Roadmap

## Done

Data folder, editions, shared maps, dependency checks, the full command set, and install from source.

## Done: desktop GUI

A separate project, `wc2bne-gui/`, in Python + GTK 3, chosen as the lightest option that gives a real
window on most distros. It calls only the `wc2bne` command and its `--porcelain` output, so the command
line stays fully usable alone. It should cover:

- installing an edition and its dependencies, with approval
- choosing the edition; launching the game and opening its folder
- listing, adding, selecting, removing and resetting gateways
- settings (edition, window mode, UDP port)
- choosing the data folder

Built in [wc2bne-gui](../../wc2bne-gui/README.md), 2026-11-25. Not yet tried on a real desktop. Design: DECISIONS.md, 17.

## Later: install methods

In this order. The GUI does not have to wait for them.

1. `.deb`: declares Wine and other dependencies.
2. AppImage: bundles Wine; keeps the bubblewrap sandbox.
3. Flatpak: last, because its sandbox needs the most rework (see DECISIONS.md, 2).

Each sets its own default data folder; source checkouts use `data/`.

## Possible rewrite

Functions take arguments and print results, calculations are kept apart from side effects, and SPEC.md plus
the command-line tests define behavior. A rewrite in another language should pass the same tests.
