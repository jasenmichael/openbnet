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
| `max_connections` **R** | `2000` | All protocols together. The process needs a few more open files than this |
| `max_connections_per_ip` | `0` | 0 means no limit |
| `login_timeout_secs` | `120` | For connections that haven't logged in |
| `ping_interval_secs` | `120` | Four unanswered pings end a connection |
| `products` | `[]` | Games allowed in: `W2BN`, `STAR`, `SEXP`, `SSHR`, `JSTR`, `DRTL`, `DSHR`, `D2DV`, `D2XP`. Empty means all |
| `strict_versions` | `false` | `true` refuses unknown and modified builds, including the War2Combat community build |

## [database] **R**

| Setting | Default | |
|---|---|---|
| `url` | `""` | Empty means SQLite in `data/openbnet.db`. Or `postgres://user:password@host/database` |

## [files] **R**

| Setting | Default | |
|---|---|---|
| `directory` | `"files"` | What games download. Files can be added while the server runs |
| `allow_unknown` | `false` | `true` serves MPQs with an unknown checksum. Games run the code inside them |

Useful files: `IX86ver1.mpq` and `ver-IX86-1.mpq` (version checks), `icons.bni`, `termsofservice.txt`, `newaccount.txt`,
banners.

## [network]

For games hosted on the server's own network. See [DEPLOYMENT.md](DEPLOYMENT.md#players-inside-the-servers-network).

| Setting | Default | |
|---|---|---|
| `public_ip` | unset | The address outside players use for hosts inside. An IPv4 address, not a name |
| `local` | `[]` | The networks inside, such as `["192.168.0.0/24"]` |
| `[[network.override]]` `host`, `public` | none | A host inside with a forwarded port of its own: `host = "192.168.0.50"`, `public = "203.0.113.9:6118"` |

## [accounts]

| Setting | Default | |
|---|---|---|
| `allow_new` | `true` | Let players sign up |
| `symbols` | `"-_[]"` | Allowed in names besides letters and digits |
| `max_failures` | `10` | Wrong passwords per address in `window_secs`; then even the right one is refused. 0 means no limit |
| `max_signups` | `20` | New accounts per address in `window_secs`. 0 means no limit |
| `window_secs` | `300` | |

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
| `behind_proxy` | `false` | Believe the last address in `X-Forwarded-For`. Only if nobody can reach `listen` except the proxy |
| `admin_token` | `""` | Password of the administration API. Empty turns it off. Use a long random one: `openssl rand -hex 32` |

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

## Command line

```sh
openbnet [--data DIR] serve
openbnet [--data DIR] create-account NAME [--role player|moderator|admin]    # asks for the password
openbnet [--data DIR] set-role NAME ROLE                                     # from the next login
```

`OPENBNET_DATA` also sets the folder, and `RUST_LOG=openbnet=debug` shows more. Both account commands work while the server
runs. Run them as the server's user, or it may lose write access to its database.
