# Specification: wc2bne-linux

The launcher's behavior, independent of its language. Terms: [GLOSSARY.md](../GLOSSARY.md).
What has been verified: [TESTING.md](TESTING.md).

## Goals

1. Install and run Warcraft II BNE on Linux from the War2Combat 4.6.1 installer.
2. Stay compatible with Battle.net online play and with PvPGN.
3. Contain the client, which runs code supplied by servers.
4. Keep everything it creates in one portable data folder.

Out of scope: a native engine port, patched or decompiled game code (the agent that built this declined to
recreate the game; the owner has been told), distributing game files, CD-key and no-CD tools.

## Data folder

```text
wc2bne.conf               settings
War2Combat46_en.exe       installer
Maps/                     shared maps
community/  official/     game folder of each installed edition
  Maps -> ../Maps
  .wine/                  its Wine environment, and install.log
.locks/                   what is using each edition
.checksums/               checksums of the checked files, per edition
```

Location: `WC2BNE_HOME` (may be relative) if set; else the path remembered in
`${XDG_CONFIG_HOME:-~/.config}/wc2bne/data-folder` by `config set data-folder`; else `data/` beside the launcher, if that exists or the
launcher's folder is writable; else `${XDG_DATA_HOME:-~/.local/share}/wc2bne`. Symlinks to the launcher are
followed. It needs a filesystem with symlinks (not FAT or exFAT).

## Settings

An option beats the environment, which beats `wc2bne.conf`, which beats the default.

| Name | Option | Environment | Values | Default |
|---|---|---|---|---|
| `edition` | `--edition` | `WC2BNE_EDITION` | `community`, `official` | `community` |
| `mode` | `--fullscreen`, `--window` | `WC2BNE_MODE` | `fullscreen`, `window` | `fullscreen` |
| `port` | | | 1024-65435 | 6112 |

`wc2bne.conf` holds `NAME=VALUE` lines; other lines are kept; it is never run as code. An invalid value is an
error that names its source. `port` is stored only in the edition's registry. `config set` can repair an
invalid file.

Other environment variables: `WC2BNE_HOME`, `WC2BNE_SETUP` (installer to use), `WC2BNE_OFFLINE` (non-empty: no
network), `WC2BNE_SHA256` (expected installer checksum, for tests), `WINEDEBUG` (default `-all`).

## Commands

`wc2bne COMMAND [OPTIONS] [ARGS]`. Options may come before or after the command. `help`, `help COMMAND`,
`--help`, `-h` and `--version` work everywhere.

| Command | Behavior |
|---|---|
| `install [--porcelain]` | Installs the edition |
| `uninstall [--yes]` | Deletes the game folder; asks first. Maps and the other edition stay |
| `play` | Checks the checked files, then runs the game |
| `open [--maps]` | Opens the game folder, or the shared maps, with `xdg-open` |
| `status [--porcelain]` | Data folder, settings, and `installed`, `changed` or `absent` per edition, and what is using it. Creates nothing |
| `doctor [--porcelain]` | Reports dependencies; offers to install missing ones |
| `gateway list [--porcelain]` | The list in effect |
| `gateway add HOST NAME [OFFSET]` | Appends and selects |
| `gateway select N`, `remove N` | By number, from 1. The last can't be removed |
| `gateway set` | Replaces the list with rows from stdin, in the porcelain format; exactly one selected |
| `gateway reset` | Drops the override list |
| `config list [--porcelain]`, `get NAME`, `set NAME VALUE` | Settings. Also `data-folder`: a path, or `default`; moves nothing |
| `exec -- CMD...` | Runs CMD in the sandbox, with network |

| Exit code | Meaning |
|---|---|
| 0 | Success (`exec` and `play` pass on the program's own code) |
| 1 | Failed |
| 2 | Usage error or invalid setting |
| 3 | The edition isn't installed |
| 4 | A needed program is missing |
| 5 | Wrong checksum: installer, download, or a changed game file |
| 6 | The edition is in use |
| 130 | Interrupted |

`--porcelain` output is stable. `status` and `config list`: `NAME=VALUE` lines. `gateway list`: number, selected
(1 or 0), host, GMT offset and name, separated by tabs. `status` includes `EDITION_in_use=no` or the command using it.
`install`: `step=download|prepare|install|finish|done`, and `percent=N`. `doctor`: `NAME=ok|missing|optional`, then
`install=COMMAND` lines to run as root, which ask nothing.

### One user at a time

`install`, `uninstall`, `play`, `exec`, gateway changes and `config set port` hold the edition until they finish; anything
else that needs it exits 6. Reading never waits and doesn't start Wine: it uses the saved registry file, which may be a
little old while the game runs. A killed launcher leaves nothing locked.

### Installing

Refused if already installed or with no display. Installer, in order:

1. `WC2BNE_SETUP`. Missing is exit 1, wrong checksum exit 5; nothing else is tried.
2. `War2Combat46_en.exe` in the data folder, if its checksum matches.
3. Download over HTTPS to the data folder: `ftp.war2.ru`, then `war2combat46en.netlify.app`. A failure or wrong
   checksum moves on to the next. A mismatched kept file becomes `.bad`, only after a good download.

No installer runs without a matching SHA-256. It runs silently, offline, in the sandbox, into `C:\war2`. Then the
edition's maps are merged into the shared maps without overwriting and replaced by a relative link, and `z:` is removed.
`official` also gets an override list with only Blizzard's server. Last, the checked files' checksums are recorded: until
then the edition isn't installed, so a stopped install can be run again. Deleting an edition's folder also uninstalls it.

| | `community` | `official` |
|---|---|---|
| Installer component | `program\lc` + plugins | `program\gog` |
| Version | 2.0.2.0 | 2.0.2.1 |

### Gateways

Any change copies the list in effect into the override list. Removing a gateway keeps the same one selected; removing
the selected one selects the first. HOST is `[A-Za-z0-9.-]+`, OFFSET an integer; NAME is non-empty, without `|`, `\`,
tabs or line breaks.

### Dependencies

`bwrap`, `wine`, 32-bit Wine libraries (`wine32`); `curl` to download; `Xwayland` to play on Wayland; `xdg-open`
for `open`; `xrandr` is optional. When something is missing: name it, print the commands for `apt`, `dnf`, `pacman`
or `zypper`, and on a terminal ask `[y/N]`. Nothing is installed without a yes. Otherwise exit 4.

Wine environments use Wine's default architecture; `WINEARCH=win32` is refused by new WoW64 builds of Wine.

## Display

The client opens its Battle.net screens as separate top-level windows, so on Wayland:

1. Start a private rootful Xwayland with no window manager. Window: `640x480 -decorate`. Fullscreen: `-fullscreen`,
   480 high, width shaped like the screen (854 on 1920x1080), at least 640.
2. Rewrite `war2_ddraw.ini` on every `play`: `Windowed=Yes`, `ShowWindowFrame=No`, centered `PosX`, `PosY=0`.
3. Run the game there; stop Xwayland on exit.

Without Wayland, or if Xwayland won't start, the game gets a 640x480 Wine desktop.

## Security

**Threat:** at login the client downloads an MPQ from the server, extracts a DLL and calls its `CheckRevision`. Any
server the player joins can run code as the client process.

The sandbox must:

- use `--unshare-all --clearenv --new-session --die-with-parent`, sharing only the network, and only for `play` and
  `exec`;
- mount `/usr`, `/etc` and `/sys` read-only, with fresh `/tmp`, `/run`, `/var` and home;
- bind read-write only the edition's game folder, its Wine environment, and the shared maps; hide `.wine` inside the
  game folder; give the installer read-only;
- not reach the other edition, `wc2bne.conf`, `.checksums` or the user's home;
- expose only the X socket in use, the PulseAudio socket, `/dev/dri` and `/dev/snd`; on Wayland, `play` exposes the
  nested display, not the desktop's;
- wait for `wineserver` to exit, so the registry is written.

**Not protected:** server code can reach the LAN and localhost, use the audio socket (including recording, if the sound
server allows), read `/etc` files the user can, and change its own edition and the shared maps. In the Wine desktop
fallback it shares the desktop's X server; `exec` and `install` use whatever `DISPLAY` is. The file check catches
accidental changes, not server code changing files while the game runs. The game folder is visible, so starting its
`.exe` from a file manager runs it outside this sandbox.
