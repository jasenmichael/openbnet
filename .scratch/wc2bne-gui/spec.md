# Desktop GUI for wc2bne-linux (`wc2bne-gui`)

Status: resolved

Terms: the wc2bne-linux glossary. Design agreed with the owner: wc2bne-linux decision 17. Points marked † were not
discussed and were chosen while writing this; the owner can change them.

## Problem Statement

Playing Warcraft II: Battle.net Edition through wc2bne-linux means using a terminal: installing missing programs,
installing an edition, starting the game, and adding gateways by typing commands and numbers. Many players don't use a
terminal, and those who do still have to remember the commands. Problems such as a missing program, a changed game file
or an edition in use appear only as text and an exit code.

## Solution

A small desktop window that does everything the `wc2bne` command line does for a player. Its three tabs are **Play**
(install, choose an edition and mode, start the game), **Servers** (edit the gateway list) and **Settings** (data folder,
UDP port, required programs, uninstall). It can be added to the desktop menu. It only runs `wc2bne` commands, so both
can be used on the same data folder, and the command line stays fully usable alone.

## User Stories

### Starting it
1. As a player, I want to open a window instead of a terminal, so that I can play without learning commands.
2. As a player, I want to add it to my desktop menu with one option, so that I can start it like any other program.
3. As a player, I want to remove it from the menu, so that nothing is left behind.
4. As a player using a source checkout, I want it to find the launcher beside it, so that I don't have to configure anything.
5. As a player with `wc2bne` on my `PATH`, I want it to find that one, so that it works when installed elsewhere.
6. As a developer, I want to choose the launcher with an environment variable, so that I can test another copy.
7. As a player, I want a clear message if the launcher isn't found, so that I know what to install.
8. As a player without GTK, I want a message naming what's missing, so that I don't see a Python error.
9. As a player, I want the window to open straight away, so that it doesn't seem broken while it reads status.
10. As a player, I want it to follow my desktop's theme, including dark mode, so that it fits in.
11. As a player opening it twice, I want the existing window brought forward, so that I don't get two. †

### Installing an edition
12. As a new player, I want Play to become Install when the edition is absent, so that the next step is obvious.
13. As a new player, I want to see which step is running and how far the download is, so that I know it isn't stuck.
14. As a new player, I want the installer found or downloaded for me, so that I needn't look for it.
15. As a new player, I want Play to appear when installing finishes, so that I can start at once.
16. As a player, I want to cancel an install, so that I'm not forced to wait. †
17. As a player, I want a failed install explained and a way to try again, so that a lost connection isn't final.
18. As a player, I want a wrong checksum explained as "not run", so that I know nothing unsafe happened.
19. As a player, I want to choose the edition before installing, so that I only use disk space for what I need.
20. As a player, I want a warning before closing the window during an install, so that I don't stop it by accident. †

### Missing programs
21. As a new player, I want to see which needed programs are missing, so that I know why I can't play.
22. As a new player, I want to see the exact commands before they run, so that I know what changes on my PC.
23. As a new player, I want an Install button that asks for my password the usual way, so that I don't need a terminal.
24. As a player without `pkexec`, I want to copy the commands, so that I can run them myself.
25. As a player, I want to cancel the password prompt without harm, so that refusing is safe.
26. As a player on an unsupported distro, I want the missing programs named, so that I can find them myself.
27. As a player, I want the list checked again after installing, so that I can see whether it worked.
28. As a player, I want optional programs shown as optional, so that I don't think they block me.

### Playing
29. As a player, I want one large PLAY button, so that starting takes one click.
30. As a player, I want to choose Fullscreen or Window, so that the game fits how I play.
31. As a player, I want my edition and mode remembered, so that I set them once.
32. As a player with both editions, I want to switch between them, so that I can use PvPGN and Blizzard's server.
33. As a player, I want the window to stay open and say "Running", so that I know the game started.
34. As a player, I want changes disabled while that edition runs, so that I don't lose them.
35. As a player, I want to close the window without ending the game, so that I can't lose a match that way.
36. As a player, I want it to notice a game started from the terminal, so that it doesn't offer what would fail.
37. As a player, I want the window to be ready again when the game ends, so that I can play again.
38. As a player, I want an explanation if the game fails to start, so that I'm not left with nothing.
39. As a player whose game files changed, I want to be told and offered a reinstall, so that I understand why it won't start. †
40. As a player whose environment sets the edition or mode, I want the choices made in the window to apply to that start, so that the button does what it says. †

### Files
41. As a map collector, I want to open the shared maps, so that I can add maps.
42. As a player, I want to open the game folder, so that I can reach my saves and replays.
43. As a player, I want Open game folder unavailable when the edition is absent, so that it doesn't fail.

### Servers
44. As a player, I want to see the gateways of the chosen edition and which is selected, so that I know where I'll connect.
45. As a player, I want to add a gateway by address and name, so that I can join a friend's server.
46. As a player, I want to set its GMT offset, so that it matches what the game expects.
47. As a player, I want to select a gateway, so that the game uses it.
48. As a player, I want to remove a gateway, so that the list stays short.
49. As a player, I want to be stopped from removing the last one, so that the game keeps a valid list.
50. As a player, I want several changes saved at once, so that I only wait once.
51. As a player, I want to see that I have unsaved changes and be able to discard them, so that I know what's in effect.
52. As a player, I want a warning before losing unsaved changes, so that I don't lose work. †
53. As a player, I want a bad address or name explained before saving, so that I can fix it.
54. As a player, I want to return to the game's list, so that I can undo everything.
55. As a player, I want to be asked before that, so that I don't lose my list by accident. †
56. As a player, I want a reminder to add only gateways I trust, so that I know a server can run code in the sandbox. †
57. As a player without that edition, I want the tab to say it must be installed first, so that an empty list isn't confusing.
58. As a player, I want names in any language shown and saved correctly, so that nothing is damaged.

### Settings
59. As a player, I want to see where the data folder is, so that I can back it up.
60. As a player, I want to choose another data folder, so that I can keep it on another disk.
61. As a player, I want to be told that choosing one moves nothing, so that I'm not surprised by an empty one.
62. As a player, I want to return to the default data folder, so that I can undo that.
63. As a player with `WC2BNE_HOME` set, I want to see that it decides the folder, so that I know why I can't change it.
64. As a host, I want to set the UDP port, so that it matches my router.
65. As a host, I want an invalid port refused with the allowed range, so that I can correct it.
66. As a player, I want to check required programs at any time, so that I can diagnose problems.
67. As a player, I want to uninstall an edition after confirming what is deleted, so that I can free disk space.
68. As a player, I want to know that shared maps and the other edition stay, so that I'm not afraid to.
69. As a player with a broken settings file, I want it explained and repairable, so that the window isn't useless. †
70. As a player, I want to see the launcher's version and the license, so that I can report problems. †

### Accessibility and other people
71. As a keyboard user, I want every control reachable with the keyboard, so that I don't need a mouse.
72. As a screen-reader user, I want every control labeled, so that I know what it does.
73. As a player with large text, I want nothing cut off, so that I can read it.
74. As a terminal user, I want the GUI and the command line to show the same things, so that I can use either.
75. As a privacy-minded player, I want the GUI itself to make no network connections, so that only known downloads happen.
76. As a packager, I want it separate from the launcher, so that a package without a GUI is possible.
77. As a maintainer, I want it to rely only on the documented command line, so that the launcher can be rewritten.

## Implementation Decisions

**Project.** A separate project in the monorepo, Python 3 and GTK 3 through PyGObject, with no other runtime
dependency. Its program is `wc2bne-gui`, with `--add-to-menu`, `--remove-from-menu`, `--help` and `--version`.

**Modules.** Three, kept apart:
- *Launcher client*: the only part that runs commands. Finds the launcher, runs commands without blocking the window,
  turns `--porcelain` output and exit codes into plain values. Parsing is pure.
- *State*: pure functions from what the launcher reported, plus unsaved edits, to what the window shows and allows.
- *Window*: GTK widgets that show the state and report clicks. No parsing and no decisions.

**Contract.** Only documented `wc2bne` commands, porcelain formats and exit codes. No reading or writing of the data
folder, registry, settings or remembered path. Unknown porcelain names are ignored, so the launcher can add some.
Commands get an argument list, never a shell string.

**Finding the launcher.** `WC2BNE`, then the sibling `wc2bne-linux`, then `PATH`. A set but unusable `WC2BNE` is an
error, with no fallback. †

| In the window | Command |
|---|---|
| Open, refresh | `status --porcelain` |
| Edition or mode chosen | `config set edition` / `config set mode` |
| Install | `install --porcelain --edition E` |
| PLAY | `play --edition E` with `--fullscreen` or `--window` |
| Open game folder, Open maps | `open`, `open --maps` |
| Servers tab shown | `gateway list --porcelain` |
| Save servers | one `gateway set`, rows on standard input |
| Back to the game's list | `gateway reset` |
| Required programs | `doctor --porcelain` |
| Port, data folder | `config get` / `config set` |
| Uninstall | `uninstall --yes`, after the window's own confirmation |

Every command about an edition passes `--edition`.

**Edition choice.** The choice on the Play tab applies to all three tabs, which each name it. †

**Playing.** `play` runs in its own session, not as a child that dies with the window. Its output goes to a file, not
a pipe, so closing the window can't break the game. The window learns that a game runs from `status`
(`EDITION_in_use`), which also covers games started elsewhere and a window reopened during a game. It checks about
every two seconds while visible. †

**Changes while in use.** Everything that would exit 6 is disabled for that edition, with the reason shown. Exit 6 is
still handled, because the state can change between checking and clicking.

**Installing.** Progress follows `step=` and `percent=`; steps without a percentage show activity. Cancel interrupts
the launcher, and exit 130 shows as "Cancelled". †

**Missing programs.** `doctor`'s `install=` lines are shown exactly and run exactly, in order, stopping at the first
failure, with one `pkexec` request. Nothing runs before the click. Afterwards `doctor` decides success, not the
commands' exit codes. With no `pkexec`, or no `install=` lines, Copy or the names remain. The GUI itself refuses to
run as root. †

**Servers.** Edits are kept in the window until Save. The same rules as the launcher are checked while typing, for
quick feedback only; the launcher's answer is final. A failed Save keeps the edits.

**Errors.** Each exit code has a plain explanation and a next step: 1 failed (with the launcher's message); 2 invalid
setting; 3 offer Install; 4 show missing programs; 5 checksum or changed files; 6 in use. The launcher's message can
always be seen in full. If `status` fails because of a bad setting, the Settings tab still works for repairing it.

**Looks.** System theme, its own simple icon, no Blizzard artwork, fonts or sounds. One fixed, small window that still
grows for large text. Interface text is English; user-supplied text is shown as plain text, never as markup.

**Menu entry.** Written to the user's applications and icon folders under `XDG_DATA_HOME`, with an absolute path to the
program. Running it again updates it; removing deletes only what it wrote.

**Docs.** The project gets its own README, AGENTS.md and testing notes, and joins the root indexes and glossary map. If
the GUI needs something the command line lacks, it is added to the command line first, with its own tests and spec.

**Order.** (1) Play tab, with installing and missing programs; (2) Servers tab; (3) Settings tab and menu entry. The
owner tries it on a real desktop after each.

## Testing Decisions

**One seam, agreed with the owner:** the real window, on a hidden display, run against a stand-in `wc2bne`. Tests find
controls by their accessible names, operate them, and check what is shown and which commands ran, with their arguments
and standard input. The only other observable is the menu entry's files, in a temporary home.

- A good test states something a player can see or cause. It doesn't call the modules, count refreshes or check widget
  classes. Rewriting the window with another toolkit should change the test driver, not what the tests say.
- Expected commands and outputs are literals from the launcher's specification, not values computed like the code does.
- The stand-in is scripted per test: porcelain output, exit codes, slow commands, progress. `pkexec` and file opening
  are stand-ins too. No real launcher, Wine, network, data folder or password prompt. The home and `PATH` are temporary.
- Waiting is for a condition with a time limit, never a fixed sleep.
- Python's `unittest` and Xvfb; no new dependency. Xvfb is for development only.
- Test-first, one behavior at a time. Every exit code has at least one test, as does each row of the command table.
- **Drift:** the stand-in could disagree with the launcher. Its formats follow the launcher's specification, which the
  launcher's own tests cover. A change there must be made in both.
- **Prior art:** wc2bne-linux's bats tests, which drive the command line with stand-ins for everything outside it.
- **Not covered:** looks, real Wayland and COSMIC behavior, the real `pkexec` prompt, real installs and the game.
  Each step ends with a short check by the owner.

## Out of Scope

- Anything the command line can't do; changes to the game, the sandbox or the launcher's rules.
- Starting or managing a game server; accounts, chat, friends, ladders or a server browser.
- Several data folders at once, or moving files between them.
- A map manager or editor; CD-key and no-CD tools.
- Packages (`.deb`, AppImage, Flatpak), updates, translations, GTK 4, tray icon, other systems.
- The release work: remote installer, crash-safe installs, disk-space check, signing, CI.

## Further Notes

- The larger AppImage that Python and GTK cause is accepted; one without the GUI may be offered.
- Reads take about 0.02 s, so checking often is cheap. One write starts Wine and takes about 2 s, so saving needs a busy
  state.
- `WC2BNE_HOME`, `WC2BNE_EDITION`, `WC2BNE_MODE` and `WC2BNE_OFFLINE` are inherited by the launcher unchanged.
- A game folder with changed files counts as installed, so reinstalling it means uninstalling first, which deletes saves
  in it. The window must say so.
- Developed on Pop!_OS 24.04 with GTK 3.24. Other distros are untested.
