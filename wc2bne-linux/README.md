# wc2bne-linux

Play **Warcraft II: Battle.net Edition** on Linux, online, against
[PvPGN](https://github.com/pvpgn/pvpgn-server) servers or Blizzard's classic Battle.net.

The game is the original Windows program from the War2Combat 4.6.1 installer, unmodified.
`wc2bne` is a Bash launcher that installs it, runs it under Wine inside a
[bubblewrap](https://github.com/containers/bubblewrap) sandbox, and shows it in one window.
It is not a native port, and no game files are included here.

## Requirements

Wine with 32-bit support and bubblewrap, plus Xwayland on Wayland desktops. `wc2bne doctor` checks and offers to
install what is missing (`apt`, `dnf`, `pacman` or `zypper`), asking first. Developed on Pop!_OS 24.04 with Wine 9.0;
other distros are untested.

## Install from source

```sh
git clone <this repository> wc2bne
cd wc2bne/wc2bne-linux
./wc2bne doctor
./wc2bne install            # downloads the installer (50 MB) if needed
./wc2bne play
```

To run it as `wc2bne` from anywhere, link it into a directory on your `PATH`:

```sh
mkdir -p ~/.local/bin
ln -s "$PWD/wc2bne" ~/.local/bin/wc2bne
```

If `wc2bne` isn't found after that, add `~/.local/bin` to your `PATH` (on many distros, log out and in).
To uninstall, remove the link and this folder. Packages (`.deb`, AppImage, Flatpak) are on the
[roadmap](docs/ROADMAP.md).

### Where the installer comes from

`WC2BNE_SETUP` if set; else a copy in the data folder; else a download from
<https://ftp.war2.ru/war2/War2Combat46_en.exe> or, failing that,
<https://war2combat46en.netlify.app/War2Combat46_en.exe>. Its SHA-256 must match, or it won't be run.

## Using it

For a window instead of commands, see [wc2bne-gui](../wc2bne-gui/README.md).

```sh
wc2bne play                     # fullscreen
wc2bne play --window            # 640x480 window
wc2bne open                     # open the game folder, to add maps
wc2bne status
wc2bne gateway list
wc2bne gateway add my.server.example "My server"
wc2bne gateway select 2
wc2bne gateway reset            # back to the game's own list
wc2bne config set mode window
wc2bne help
```

### Editions

| Edition | Game version | For |
|---|---|---|
| `community` (default) | 2.0.2.0 (2.02a) + plugins | PvPGN servers |
| `official` | 2.0.2.1 (2.02b, GoG) | Blizzard's server, with your own CD key |

Most people only need `community`. Add the other with `wc2bne install --edition official`, and use `--edition official`
or `wc2bne config set edition official`. You choose the server inside the game.

### Your files

Everything is in `data/`, or wherever `WC2BNE_HOME` points. You can move or copy it as a whole; it must be on a
filesystem with symlinks (not FAT or exFAT).

```text
data/
  wc2bne.conf               settings
  Maps/                     your maps, shared by both editions
  community/                the game folder
    .wine/                  Wine's files, about 1.3 GB
```

Maps and other files are yours to edit. Servers check `Warcraft II BNE.exe`, `storm.dll` and `battle.snp`, so `play`
refuses to start if those have changed. **Start the game with `wc2bne play`, not by double-clicking the `.exe`**, which
would run it without the sandbox.

## Status

On a local PvPGN server: login with both editions, and with `community` a two-player match, chat, and downloading the
host's map. It works on the owner's COSMIC desktop. Not confirmed: Blizzard's server, internet hosting, and other
distros. See [docs/TESTING.md](docs/TESTING.md).

## Security

A Battle.net server sends the client code to run at login. The sandbox hides your home directory,
but still has network access, so only join servers you trust.
Don't run the game outside it. Details: [docs/SPEC.md](docs/SPEC.md#security).

## Development

Code is in `lib/`, documented in each file. See [AGENTS.md](AGENTS.md), [docs/](docs) and [GLOSSARY.md](GLOSSARY.md).

Tests need [bats](https://github.com/bats-core/bats-core); playing doesn't.

```sh
bats test        # or: npx bats test
```

## Legal

Warcraft II and its files belong to Blizzard Entertainment. This project is unofficial and
contains none of them; the installer is downloaded from third-party Warcraft II community hosts. 
This project's own code is GPL-3.0-or-later; see [LICENSE](LICENSE).
