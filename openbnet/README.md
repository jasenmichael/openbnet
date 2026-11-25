# openbnet

A Battle.net-compatible server for Blizzard's classic games, written in Rust and based on
[PvPGN](https://github.com/pvpgn/pvpgn-server). Unmodified games can create accounts, log in, chat, and host
and join games.

## Status

| Game | State |
|---|---|
| Warcraft II: Battle.net Edition | Works: two real 2.02a clients played a match. Ladders are protocol-tested only |
| StarCraft, Brood War, Diablo I | Protocol-tested only: log in, chat, games, ladders |
| Diablo II (open Battle.net) | Protocol-tested only: log in, chat, games |
| Diablo II (closed realms), Warcraft III | Planned |
| Bot, telnet, IRC and WebSocket chat | Works; tested with simulated clients |

Bot and telnet chat and IRC are off until you turn them on. The [HTTP and WebSocket API](docs/API.md) listens on this
computer only.

Details: [docs/TESTING.md](docs/TESTING.md) and [docs/ROADMAP.md](docs/ROADMAP.md).

## Run it

```sh
cargo build --release
mkdir -p data/files
cp ../.vendor/pvpgn-server/files/{IX86ver1.mpq,ver-IX86-1.mpq,icons.bni} data/files/   # or from your PvPGN
./target/release/openbnet serve
```

That is enough to play on this computer and your network: add its address as a gateway in the game. Everything the
server keeps is in `data/`, so that folder is also the backup:

| | |
|---|---|
| `openbnet.toml` | Settings, written with the defaults on the first start |
| `openbnet.db` | Accounts, results and friends |
| `files/` | What games download |

Use another folder with `--data DIR` or `OPENBNET_DATA`.

The version-check files aren't openbnet's, so you copy them in. Without them games say "Battle.net was unable to properly
identify your application version", and the server warns at startup. No restart is needed after copying. Games run the
code inside MPQ files, so ones with an unknown checksum aren't served unless you set `allow_unknown`.

## Set it up

Make yourself an administrator. The server may be running:

```sh
./target/release/openbnet create-account YourName --role admin    # or: set-role YourName admin
```

The rest is optional.

| You want | Do |
|---|---|
| To play with others on your network | Use this computer's address, such as `192.168.0.110`, as the gateway instead of `127.0.0.1` |
| Players over the internet, or a domain | [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) |
| Cloudflare | A tunnel carries only the web API, not games: [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#cloudflare) |
| A service or a container | [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#as-a-service) |
| To change how it behaves | [docs/CONFIGURATION.md](docs/CONFIGURATION.md), in `data/openbnet.toml` |
| A website or web chat | [docs/API.md](docs/API.md) |

Apply changed settings with `pkill -HUP -x openbnet`.

## License

GPL-3.0-or-later; see [LICENSE](LICENSE). openbnet is ported from PvPGN, © Mark Baysinger, Ross Combs, Rob
Crittenden, Marco Ziech, Dizzy and the other authors in
[PvPGN's CREDITS](https://github.com/pvpgn/pvpgn-server/blob/master/CREDITS), used under the GPL version 2 or later.
`assets/versions.json` is PvPGN's `versioncheck.json`.

Unofficial, and not connected with Blizzard Entertainment. It contains none of Blizzard's files.
