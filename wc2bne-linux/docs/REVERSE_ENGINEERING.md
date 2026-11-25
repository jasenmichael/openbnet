# Reverse-engineering notes

Findings from static analysis (REA + Ghidra) and runs against PvPGN. Addresses are virtual
addresses in `battle.snp` (PE32 i386, image base 0x19000000), hash below.

## Installer

`War2Combat46_en.exe`: Inno Setup 5.6.1, 52,232,067 bytes,
SHA-256 `2b53fabb11dafc811c2167719cf2ac29ae41bd38363289748631305847436ecd`.
It holds three builds sharing identical `storm.dll` and `battle.snp`:

| Component | Version | Used as |
|---|---|---|
| `gog` | 2.0.2.1 | `official` |
| `lc` | 2.0.2.0 + `LC.dll` plugins | `community` |
| `2020` | 2.0.2.0 | not used |

Installed files (SHA-256):

| File | Hash |
|---|---|
| `Warcraft II BNE.exe` (community) | `cb852be461cebb03efdfdeeb65f67e6d5a279d97a13b11d09b366cba2e701cd7` |
| `Warcraft II BNE.exe` (official) | `c3d83429aa0c2200ad3f6ac141da8ae7213a7b196d1f70d4d84a6e607bd504bd` |
| `storm.dll` | `3f5999aa3190bf5a53dda42e43ab8cbf46a36e805cc9bfd3498613dbd492fdbc` |
| `battle.snp` | `f8b8689ff9f61b5343e03182b72e2a303c4c1184b9a3236074829849be49af86` |

## Gateways

Registry key `HKCU\Software\Battle.net\Configuration`, type `REG_MULTI_SZ`:

```text
"2000"            list version
"01"              selected server, 2 digits, 1-based
host, GMT offset, name     repeated per server
```

| Address | Finding |
|---|---|
| `0x19002930` | Reads `Override Battle.net gateways` first, then falls back to `Battle.net gateways` |
| `0x19002700` | Picks between the override and normal value names |
| `0x19002970` | Handles the server-pushed list; writes only the normal value |

So the override survives a server push, and the normal list doesn't. A push has been seen in
practice: after online play, the `community` normal list differs from the installer's.

Installer defaults: `server.war2.ru`, `backup.war2.ru`, `server.war2.me`, `backup.war2.me`,
`connect-forever.classic.blizzard.com`.

## Ports

| Use | Port |
|---|---|
| Battle.net login and chat | TCP 6112 |
| Game data | UDP 6112 by default, from `Game Data Port` (DWORD); scans up to 100 ports (`0x19014240`) |

## Login and version check

1. Client sends protocol byte `0x01`.
2. Packets `SID 0x1E`, then `0x12`, then `0x06` (STARTVERSIONING) with platform `IX86`, product
   `W2BN`, version byte `0x4F`.
3. The server names an MPQ and an equation; the client downloads the MPQ and calls its
   `CheckRevision` (`0x19017100`) over the exe, `storm.dll` and `battle.snp`.

PvPGN's matching entries ([pvpgn-server](https://github.com/pvpgn/pvpgn-server),
`conf/versioncheck.json.in`, `W2BN`/`IX86`/`0x4f`):

| Version | Hash | Tag |
|---|---|---|
| 2.0.2.1 | `0xff0d4c4a` | `W2BN_202B` |
| 2.0.2.0 | `0xb52bad87` | `W2BN_202A` |

**Seen in practice.** After a login to a public server (45.33.94.234, 2026-11-24), three files not from
the installer were in the `community` game directory: `CheckRevisionD1.dll`, and `war2mod.w2p` with
its `war2mod.ini`. `war2mod.w2p` is a DLL exporting both `CheckRevision` and `w2p_init`, and it
imports `URLDownloadToFileA` and WinINet. So server-supplied code does persist in the prefix. Whether
it loads again on other servers is unknown. The exe, `storm.dll` and `battle.snp` were unchanged.

Stock PvPGN also sets `allow_bad_version = true` and `allow_unknown_version = true`
(`conf/bnetd.conf.in`). Servers that turn these off are untested.

## Wine behavior

- The client creates several top-level windows for Battle.net screens.
- A Wine virtual desktop drew a taskbar in our tests, even with `EnableShell=0`.
- `Managed=N` looks right, but a click on the background kills input.
- Killing the sandbox before `wineserver` exits loses registry writes.

## Toolchain (owner's machine)

REA is needed for analysis only, not to play. `rea-agents@6.10.1` is installed globally and runs
on Node 22.23.5 (Node 26 is unsupported). Ghidra 12.2, JDK 25 and checkpoints are in
`~/.local/share/rea-toolchain`. It is registered as the `rea` MCP server in Claude Code.
Decompiler output is not kept in this repository.
