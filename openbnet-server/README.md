# openbnet-server

openbnet, a Battle.net-compatible server for Blizzard's classic games, written in Rust and based on
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

The program is called `openbnet`.

```sh
git clone https://github.com/jasenmichael/openbnet.git
cd openbnet/openbnet-server
cargo build --release
./target/release/openbnet setup      # who plays, and where; optional
./target/release/openbnet serve
```

Or with [Docker Compose](docs/DEPLOYMENT.md#docker-compose).

That is enough to play on this computer and your network: add its address as a gateway in the game. Everything the
server keeps is in `data/`, so that folder is also the backup:

| | |
|---|---|
| `openbnet.toml` | Settings, written with the defaults on the first start |
| `openbnet.db` | Accounts, results and friends |
| `files/` | What games download. May be empty |

Use another folder with `--data DIR` or `OPENBNET_DATA`.

Warcraft II needs no files: the server skips its version check, so the game runs no code from it. To check versions, or
for Diablo II and later StarCraft, [add the version-check files](docs/CONFIGURATION.md#version-checks).
openbnet contains none of Blizzard's files.

## Set it up

Make yourself an administrator. The server may be running:

```sh
./target/release/openbnet create-account YourName --role admin    # or: set-role YourName admin
```

The rest is optional.

| You want | Do |
|---|---|
| To play with others on your network | Use this computer's address, such as `192.168.1.10`, as the gateway instead of `127.0.0.1` |
| A VPS, such as a DigitalOcean droplet | [docs/VPS.md](docs/VPS.md), with or without Docker |
| Players over the internet at home, or a domain | [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) |
| Cloudflare | A tunnel carries only the web API, not games: [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#cloudflare) |
| Friends only, without opening ports | [Tailscale](docs/DEPLOYMENT.md#friends-over-tailscale) |
| A service or a container | [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md#as-a-service) |
| Other games than Warcraft II | `products` in [docs/CONFIGURATION.md](docs/CONFIGURATION.md#server) |
| To change how it behaves | [docs/CONFIGURATION.md](docs/CONFIGURATION.md), in `data/openbnet.toml` |
| A website or web chat | [docs/API.md](docs/API.md) |

Apply changed settings with `pkill -HUP -x openbnet`.

## License

GPL-3.0-or-later; see [LICENSE](LICENSE). Based on PvPGN; [NOTICE](NOTICE) says which parts, and credits its authors.

Unofficial, and not connected with Blizzard Entertainment. It contains none of Blizzard's files.
