# Settings

`data/openbnet.toml`, written with the defaults on the first start. Every setting is optional, and an unknown one is an
error, so typos don't pass silently. Paths are relative to the data folder.

To apply changes: `pkill -HUP -x openbnet`, `systemctl reload openbnet`, `docker kill -s HUP openbnet`, or the API. If the
new settings are invalid, the old ones stay and the log says why. **R** marks what needs a restart.

## [server]

| Setting | Default | |
|---|---|---|
| `name` | `"openbnet"` | Shown by the API and IRC |
| `listen` **R** | `"0.0.0.0:6112"` | Games, over TCP and UDP. IPv4. Games expect port 6112 |
| `max_connections` **R** | `2000` | Games and chat together. The process needs a few more open files than this |
| `max_connections_per_ip` | `16` | 0 means no limit |
| `login_timeout_secs` | `120` | For connections that haven't logged in |
| `ping_interval_secs` | `120` | Four unanswered pings end a connection |
| `shutdown_secs` | `5` | Warning players get before the server stops |
| `products` | `["W2BN"]` | Games allowed in: `W2BN`, `STAR`, `SEXP`, `SSHR`, `JSTR`, `DRTL`, `DSHR`, `D2DV`, `D2XP`. Empty means all |
| `strict_versions` | `false` | `true` refuses builds not in PvPGN's list or `[[build]]`. Needs the version-check file |

## [database] **R**

| Setting | Default | |
|---|---|---|
| `url` | `""` | Empty means SQLite in `data/openbnet.db`. Or `postgres://user:password@host/database` |

## [files] **R**

| Setting | Default | |
|---|---|---|
| `directory` | `"files"` | What games download. Files can be added while the server runs |
| `allow_unknown` | `false` | `true` serves MPQs with an unknown checksum. Games run the code inside them |

All optional:

| File | For |
|---|---|
| `ver-IX86-1.mpq`, `IX86ver1.mpq` | [Version checks](#version-checks) |
| `icons.bni` | Chat icons. Games have their own |
| `termsofservice.txt`, `newaccount.txt` | Shown when signing up. Without them, short built-in texts are sent |
| Banners | `[[ad]]` |

## Version checks

Off for Warcraft II until its file is in `data/files/`. The game then downloads the file, runs the program in it, and reports
a checksum of itself.

| `ver-IX86-1.mpq` | `strict_versions` | Warcraft II |
|---|---|---|
| Missing | `false` | Not checked. The game runs nothing from the server |
| Present | `false` | Checked and logged, but every build is let in. Known builds only see games of the same version |
| Present | `true` | Only builds in PvPGN's list and `[[build]]` are let in. War2Combat 4.6.1 is listed |
| Missing | `true` | Can't log in |

**It doesn't stop cheating.** The game reports its own checksum, and a modified one can report the right answer. Strict
mode keeps out old builds and casual modifications, and players with harmless ones.

### Getting the files

They appear to be Blizzard's, so openbnet doesn't include them. PvPGN's repository has them. Download only those, pinned to
one commit, and check them:

```sh
cd data/files
base=https://raw.githubusercontent.com/pvpgn/pvpgn-server/9cd173f4e02ba3d9f8f15a67ca308b5eb78723e4/files
curl -fLO "$base/ver-IX86-1.mpq"
echo "b5deb6ffeb88de8f58942b67e75b0f28aaa6703947cc968e1a0a21d378d18fef  ver-IX86-1.mpq" | sha256sum -c
```

`sha256sum` must say `OK`; if not, delete the file. That one is enough for Warcraft II. Diablo II and later
StarCraft use one or the other, depending on the version, and can't log in without theirs. For the second:

```sh
curl -fLO "$base/IX86ver1.mpq"
echo "c4cb275b310b1b268e7da64000fefbc8c3feaafb25e8830cd4c8955034f69027  IX86ver1.mpq" | sha256sum -c
```

With PvPGN's source at hand, copy them from its `files/` instead. For a service the folder is `/var/lib/openbnet/files`, and the
files must be readable by its user. Nothing needs restarting.

To see what games report, run the server with `RUST_LOG=openbnet=debug` and look for `version reported`. To require it, set
`strict_versions = true` under `[server]` and reload. To turn the check off again, delete the files and set it back.

## [network]

For games hosted on the server's own network. See [DEPLOYMENT.md](DEPLOYMENT.md#players-inside-the-servers-network).

| Setting | Default | |
|---|---|---|
| `public_ip` | unset | The address outside players use for hosts inside. An IPv4 address, not a name |
| `local` | `[]` | The networks inside, such as `["192.168.1.0/24"]` |
| `[[network.override]]` `host`, `public` | none | A host inside with a forwarded port of its own: `host = "192.168.1.50"`, `public = "203.0.113.9:6118"` |

## [accounts]

| Setting | Default | |
|---|---|---|
| `allow_new` | `true` | Let players sign up |
| `symbols` | `"-_[]"` | Allowed in names besides letters and digits |
| `max_failures` | `10` | Wrong passwords per address in `window_secs`; then even the right one is refused. 0 means no limit |
| `window_secs` | `300` | |
| `max_signups` | `5` | New accounts per address in `signup_window_secs`. 0 means no limit |
| `signup_window_secs` | `3600` | |

## [chat]

| Setting | Default | |
|---|---|---|
| `welcome` | `[]` | Lines shown on entering the first channel |
| `flood_lines`, `flood_secs` | `5`, `5` | Lines allowed in that time. Long lines count as several |
| `flood_disconnect_lines` | `10` | Lines in that time that end the connection |
| `max_line` | `200` | Bytes |

## [games]

| Setting | Default | |
|---|---|---|
| `hide_private`, `hide_started` | `true` | Leave them out of game lists |
| `disconnect_is_loss` | `false` | For every game. Ladder hosts can choose it for theirs |

## [bots]

Bot and telnet chat on the game port. Passwords travel unencrypted.

| Setting | Default | |
|---|---|---|
| `enabled` | `false` | |
| `accounts` | `[]` | Who may use it. `["*"]` means all |

## [irc] **R**

| Setting | Default | |
|---|---|---|
| `listen` | unset | Such as `"0.0.0.0:6667"`. Passwords travel unencrypted |
| `tls_listen` | unset | Such as `"0.0.0.0:6697"` |
| `certificate`, `key` | unset | PEM files for TLS. Read at startup, so restart after renewing them |

## [http]

The API in [API.md](API.md).

| Setting | Default | |
|---|---|---|
| `listen` **R** | `"127.0.0.1:6180"` | No TLS. Keep it local and put a proxy in front |
| `max_connections` **R** | `256` | Besides those for games. A request may take 30 seconds |
| `behind_proxy` | `false` | Believe the last address in `X-Forwarded-For`. Only if nobody can reach `listen` except the proxy. Without it, everyone behind a proxy shares one address's limits |
| `admin_token` | `""` | Password of the administration API. Empty turns it off. Use a long random one: `openssl rand -hex 32` |

## [[build]]

Game builds that `strict_versions` accepts besides PvPGN's list. Listing any replaces the default, which is the War2Combat
4.6.1 community build. Each has `product`, `version` and `checksum` as the log shows them (`RUST_LOG=openbnet=debug`, "version
reported"), and optionally `tag`: builds with the same tag see each other's games.

```toml
[[build]]
product = "W2BN"
version = "2.0.2.0"
checksum = 0x270dfc2b
tag = "W2BN_202A"
```

## [[ad]]

One per banner: `file` (pcx, smk, mng or png, in the files directory), `url`, and optionally `product`.

## [[channel]]

One per permanent channel. Listing any replaces the built-in list. New ones open on reload; changes to open ones need a
restart.

| Setting | Default | |
|---|---|---|
| `name` | | |
| `alias` | unset | What games ask for. Numbered copies open as it fills |
| `products` | `[]` | Games it is offered to. Empty means all. Anyone can still join by name |
| `max` | no limit | People |
| `moderated`, `staff_only` | `false` | Only operators may talk; only moderators may enter |
| `topic` | `""` | Until someone sets one with `/topic` |

Games join `War2BNE`, `Starcraft`, `Brood War`, `Diablo Retail` and `Diablo II`; keep those as aliases.

## Bans

Bans are kept in the database, not here. Moderators use `/ipban <user or address> [hours] [why]`, `/ipunban` and `/ipbans`, or
see [API.md](API.md#administration). A name bans the address that player last used; with IPv6, its /64. Nobody can ban the server's
own computer, themselves, or a moderator online. Anyone with another address can come back, so lock accounts too.

## Command line

```sh
openbnet [--data DIR] setup [--for local|lan|vpn|internet] [--address ADDRESS] [--signups true|false]
openbnet [--data DIR] serve
openbnet [--data DIR] health                                                 # fails if a running server doesn't answer
openbnet [--data DIR] create-account NAME [--role player|moderator|admin]    # asks for the password
openbnet [--data DIR] set-role NAME ROLE                                     # from the next login
```

`OPENBNET_DATA` also sets the folder, and `RUST_LOG=openbnet=debug` shows more. Both account commands work while the server
runs. Run them as the server's user, or it may lose write access to its database.
