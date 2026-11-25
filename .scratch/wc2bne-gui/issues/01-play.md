# 01: Open the window and play

**What to build:** A window whose Play tab shows the chosen edition and mode, starts the game, says "Running" while it runs, and opens the game folder and shared maps. Covers spec stories 1, 4-11, 29-38, 40-43, 71-77.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Finds the launcher through `WC2BNE`, the sibling project, then `PATH`; explains when there is none, or no GTK
- [x] Edition and mode come from `status`, are saved when chosen, and apply to PLAY
- [x] PLAY runs `play` for the chosen edition and mode; "Running" shows, also for a game started elsewhere
- [x] Closing the window leaves the game running; the window is ready again when the game ends
- [x] Every failure shows a plain explanation and the launcher's own message
- [x] Open game folder and Open maps work, and are unavailable when the edition is absent
- [x] A second start brings the first window forward; refuses to run as root
