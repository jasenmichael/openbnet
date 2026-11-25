# AGENTS.md

Instructions for coding agents working on wc2bne-gui. Paths are relative to this directory. Terms:
[wc2bne-linux's glossary](../wc2bne-linux/GLOSSARY.md). Spec and tickets: `../.scratch/wc2bne-gui/`.

| Path | Responsibility |
|---|---|
| `wc2bne-gui` | Starts it; refuses root; explains missing GTK |
| `wc2bne_gui/launcher.py` | Finds the launcher; the only part that runs commands; parses their output |
| `wc2bne_gui/state.py` | Pure: what the launcher reported -> what the window shows and allows |
| `wc2bne_gui/window.py` | The window and its Play tab |
| `wc2bne_gui/servers.py`, `wc2bne_gui/settings.py` | The other tabs |
| `wc2bne_gui/menu.py`, `wc2bne_gui/icon.svg` | Desktop menu entry; the project's own icon |
| `test/harness.py`, `test/stand-in` | Private bus and display, window driver; stand-in for `wc2bne` and `pkexec` |

```sh
python3 -m test                                            # all, about 50 s
python3 -m test test.test_play.Play.test_NAME              # one
```

Tests need Xvfb (`xvfb-run`), `dbus-run-session` and `gir1.2-atspi-2.0`; using the program doesn't. They drive the real window through the
accessibility interface, as a screen reader would, and check what is shown and which commands ran.

## Rules

1. Use only documented `wc2bne` commands, `--porcelain` output and exit codes
   ([SPEC.md](../wc2bne-linux/docs/SPEC.md)). Never touch the data folder. If something is missing, add it to the command line first.
2. Never block the window: no waiting in the main thread and no `dialog.run()`, which hangs screen readers (and the tests).
3. The game must survive the window closing: own session, output to files, never pipes.
4. Run as root exactly the commands that are shown, only after a click.
5. Test-first, through the window; wait for conditions, not for time. Tests never use the real launcher, data folder,
   network or desktop.
6. No Blizzard artwork. No new runtime dependency.

## Verified for real

2026-11-25, under Xvfb, with real Wine in a throwaway data folder whose path has a space: install, cancel partway (nothing left
running, edition absent), install again, Play in a window to the main menu, close the window (game keeps running),
reopen (shows Running), game ends (ready); add, remove and select gateways with a non-ASCII name and save; UDP port;
uninstall (maps kept). The menu entry passes `desktop-file-validate`. **Not verified:** a real desktop, the real `pkexec` prompt, a real download.
