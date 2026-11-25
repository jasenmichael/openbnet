# Decisions

Append new entries; mark old ones superseded instead of deleting them.

## 1. Run the original client under Wine

Reverse engineering showed the client's own networking already works with both Battle.net and
PvPGN, and the version check needs unmodified files.
**Consequence:** the host needs 32-bit Wine, and there is no native binary.

## 2. Sandbox with bubblewrap

The client executes a server-supplied DLL at login. Wine alone doesn't protect the user's files,
as it maps the whole filesystem to `Z:`. Bubblewrap is required; there is no unsandboxed mode.
**Consequence:** Flatpak isn't a drop-in replacement. Its sandbox would be coarser here: whole-app
network access, the host Wayland socket exposed, and one data directory for both profiles.

## 3. Two profiles with separate prefixes

_Superseded in part by 11 and 12: now editions, with shared maps and `official` optional._

PvPGN communities use 2.0.2.0 and the installer's plugins; the GoG build, 2.0.2.1, is for Blizzard's
server. Separate prefixes keep settings and keys apart.
**Consequence:** about 650 MB each, mostly Wine's `windows` directory.

## 4. Use the gateway override value

A server can replace `Battle.net gateways` but not `Override Battle.net gateways`.
**Consequence:** once set, the override replaces the normal list until `gateway reset`. Changes start from a copy of
the list in effect, so adding one server doesn't hide the others.

## 5. Nested Xwayland with no window manager

The owner saw several windows, no cursor, and a windowed "fullscreen" on COSMIC, because the
compositor manages each Battle.net window separately.

Rejected:
- Wine virtual desktop: always drew a taskbar (kept only as the non-Wayland fallback).
- Wine `Managed=N`: a background click kills input.
- Testing without a window manager: hid the problem.

## 6. Wait for wineserver before leaving the sandbox

Leaving bubblewrap kills `wineserver` and lost registry writes, so `box` ends with
`wineserver -w`.

## 7. Packaging (open)

_Superseded by 15: the order is source, .deb, AppImage, Flatpak._

Current form is a script using the host's Wine. AppImage was recommended for sharing because it
keeps the existing sandbox: estimated 70-80 MB, measured from a stripped 32-bit Wine 9.0 whose
stripped copy wasn't run. Flatpak suits a public release better but needs rework. Not decided by
the owner; nothing built.

## 8. Download the installer, pinned by checksum

`install` uses `WC2BNE_SETUP`, then `.vendor`, then a download from two community hosts. Neither host
is trusted: one pinned SHA-256 decides. _Superseded in part by 11: kept in the data folder, not `.vendor`._ Downloads are kept in `.vendor` so a second profile reuses them.
**Consequence:** a new War2Combat release needs a new pinned hash. If the script is copied elsewhere,
`.vendor` is created beside its parent directory.

## 9. bats, testing through `install`

Chosen over plain Bash for per-test isolation and reporting. Development only. The owner agreed the
boundary: the `install` command, with stand-ins for `bwrap`, `wine` and `curl`.

## 10. Monorepo

Everything for this launcher lives in `wc2bne-linux/`; the root holds only an index. `.vendor/` stays at
the root because other projects will share it.

## 11. One portable data folder, with a familiar game folder

Everything lives in one movable folder, in `data/` when run from source. The game folder is visible like on Windows, with
Wine hidden in `.wine/`. The sandbox mounts it at `C:\war2`, because a symlink from inside `.wine` would make a
directory loop. Settings travel with the folder. Only the folder's
location may be remembered in `~/.config` (added for the GUI).
**Consequence:** a visible `.exe` can be started outside the
sandbox by mistake.

## 12. Editions, not profiles; shared maps only

They are game builds, not accounts, so the name changed. `official` is optional: the installer's readme says to use the
GoG build only for Blizzard's server, and in a separate folder. Maps are shared as the owner asked; registries aren't,
because a community server's code could otherwise reach the Blizzard settings and CD key.

## 13. Wine's default architecture

Pure 32-bit environments are reported refused by Arch's Wine since about 10.9 and deprecated in Wine 11.
**Consequence:** 1.3 GB per edition on Wine 9.0 instead of about 570 MB, and old environments can't be converted.

## 14. File check from recorded checksums

`play` compares the checked files with checksums recorded at install, kept outside the sandbox. Chosen over a hash
table per edition as simpler; the installer itself is pinned.

## 15. Functional Bash in small files; commands as the contract

Split for readability and a possible rewrite. The command line, exit codes and `--porcelain` formats are the interface,
tested only from outside. Install methods come in order: source, `.deb`, AppImage, Flatpak.

## 16. A lock per edition; reading without Wine

Two Wine servers on one registry overwrite each other: a gateway added during a game was lost. So one process at a time
may use an edition. Reading parses Wine's `user.reg`, because starting Wine took over 4 seconds per list.

## 17. GUI design

`wc2bne-gui/`: one GTK 3 window with Play, Servers and Settings tabs, using only the command line. Missing programs are
installed with `pkexec` after showing the commands. It stays open while the game runs, uses the system theme and no
Blizzard artwork, and is tested as a real window against a stand-in `wc2bne`, driven through the accessibility interface.

## 18. Recorded checksums mark an edition as installed

They are written last, so a cancelled or failed install doesn't count as installed and can be run again.
