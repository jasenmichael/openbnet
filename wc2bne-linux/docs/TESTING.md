# Testing

Last updated 2026-11-25.

## Automated

`bats test`: 103 tests through the command line, with stand-ins for the sandbox, Wine, network, Xwayland, xrandr,
xdg-open, sudo and package managers. `PATH` holds only those and basic tools. The real `sha256sum` runs. They don't
cover real Wine, isolation, displays or downloads.

## Verified for real

On Pop!_OS 24.04, Wine 9.0, in a throwaway data folder.

| Area | Result |
|---|---|
| Install of both editions; three file hashes match the pinned ones | Pass |
| Gateway add, select, remove, reset; port | Pass |
| Sandbox sees only `box` in `/home`; `.wine` is empty inside the game folder | Pass |
| Data folder moved to a path with a space; both editions reach the main menu | Pass |
| Map added while the game runs appears in its list | Pass |
| Login to local PvPGN 1.99.7.2.1-PRO: `community` (W2BN_202A), `official` (W2BN_202B) | Pass |
| Two `community` clients: host, join, map download into `Maps/Download`, match over a minute, in-game chat | Pass |
| Real game running: gateway change and second `play` refused; `set`, odd characters and `uninstall` work | Pass |
| On the owner's COSMIC desktop | Owner reports "it works", 2026-11-25; window, cursor, fullscreen, sound and Exit not itemized |
| In-game Exit during a match; launcher exits 0 | Pass |
| Nested display in headless Weston, main menu, both modes, cursor visible in the nested display | Pass |
| Real download from each URL, with the earlier single-file script | Pass |
| Login to a public server, owner's machine, earlier launcher, 2026-11-24 | Pass (`BnetLog.txt`) |

The PvPGN runs used the Wine desktop fallback under Xvfb.

## Not verified

- A match in the nested display; the map editor; sound
- Blizzard's server (only a DNS lookup); internet hosting and NAT; strict-version PvPGN servers
- Any other distro, Wine 10+ / new WoW64, or X11-only desktop
- Copying the data folder to another PC
- openSUSE package names. Debian 13, Arch and Fedora names were looked up on the distros' sites; no package was
  installed through `doctor` outside the tests

## Checks for a change

1. `bash -n wc2bne lib/*.sh` and `bats test`.
2. If it touches the sandbox, installer, registry or display: a real throwaway install, `exec -- ls /home`, and
   gateway changes, as above.
3. Play on a real desktop in both modes. Look for extra windows, the cursor, and input after clicking the background.

## Local PvPGN

Ask the owner before building or running containers. Image, built from
[pvpgn-server](https://github.com/pvpgn/pvpgn-server) source in `src/`:

```dockerfile
FROM debian:12-slim AS b
RUN apt-get update && apt-get install -y --no-install-recommends build-essential cmake zlib1g-dev liblua5.1-0-dev ca-certificates
COPY src /src
RUN cmake -S /src -B /b -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/opt/pvpgn -DWITH_LUA=ON && cmake --build /b -j12 && cmake --install /b
FROM debian:12-slim
RUN apt-get update && apt-get install -y --no-install-recommends zlib1g liblua5.1-0 libstdc++6
COPY --from=b /opt/pvpgn /opt/pvpgn
CMD ["/opt/pvpgn/sbin/bnetd","-f"]
```

- Run it on a network created with `--internal` (172.31.77.0/24 was used), publishing no ports.
- Set `track = 0` in `bnetd.conf`; stock PvPGN announces itself to public trackers.
- `gateway add` its IP and remove every other gateway. Use a separate `WC2BNE_HOME` per client.
- The image may still exist locally as `wc2bne-pvpgn-test`.
