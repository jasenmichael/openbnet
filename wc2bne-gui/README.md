# wc2bne-gui

A desktop window for [wc2bne-linux](../wc2bne-linux/README.md): install and play Warcraft II: Battle.net Edition without
a terminal. It only runs `wc2bne` commands, so you can use both.

```sh
./wc2bne-gui
```

- **Play:** install an edition, choose it and the display mode, play, open the game folder or maps, and install
  missing programs.
- **Servers:** add, remove and select gateways, saved together, or go back to the game's list.
- **Settings:** data folder, UDP port, required programs, uninstall, and repairing a broken settings file.

To add it to your desktop's menu: `./wc2bne-gui --add-to-menu` (and `--remove-from-menu`). Run that again if you move
the folder.

## Requirements

Python 3 with GTK 3: `python3-gi gir1.2-gtk-3.0` (Debian, Ubuntu), `python3-gobject gtk3` (Fedora) or
`python-gobject gtk3` (Arch). Tested only on Pop!_OS 24.04.

It uses the `wc2bne` named by the `WC2BNE` environment variable, else the one in `wc2bne-linux` beside this folder, else
the one on your `PATH`. Installing missing programs asks for your password through `pkexec`, after showing the commands.

Closing the window leaves the game running. Closing it during an install asks first.

## Development

See [AGENTS.md](AGENTS.md). Licensed GPL-3.0-or-later; no Blizzard artwork or game files.
