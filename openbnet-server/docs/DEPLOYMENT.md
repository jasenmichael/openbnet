# Running a server for others

Start with the [README](../README.md). Settings are in [CONFIGURATION.md](CONFIGURATION.md). **For a VPS, follow
[VPS.md](VPS.md)**: step by step, with or without Docker.

## The quick way

```sh
openbnet setup
```

It asks who will play, at which address, and whether they may sign up. It then writes `data/openbnet.toml`, keeping your other
settings, and lists what to open and what players enter. Or without questions:

```sh
openbnet setup --for internet --address bnet.example.com --signups true
```

| `--for` | Address | What it sets |
|---|---|---|
| `local` | | Listens on this computer only |
| `lan` | This computer's, such as `192.168.1.10` | Listens everywhere |
| `vpn` | This computer's on the VPN | Listens everywhere |
| `internet` | A domain or public address | Listens everywhere; behind a router also `public_ip` and `local` |

It guesses your network as a /24, and turns a domain into the address it has now. If that changes, run it again.

## What has to reach what

| What | Port | Who connects |
|---|---|---|
| Login, chat, file downloads | TCP 6112 on the server | Every game |
| Finding players' game ports | UDP 6112 on the server | Every game |
| The match itself | UDP 6112 on the **player hosting it** | The other players, directly |
| IRC, if on | TCP 6697 | IRC clients |
| Web API, if published | TCP 443 on a proxy | Browsers |

Matches don't pass through openbnet. So the server must see each player's real address, and it can't help two players who
can't reach each other.

## Which address to give out

| Setup | Game's gateway | Settings |
|---|---|---|
| Only this computer | `127.0.0.1` | none |
| One network | The server's address there, such as `192.168.1.10` | none |
| A VPS, or any server with its own public address | Its address, or a domain | none |
| Friends, on a VPN | The server's VPN address | none |
| At home, for the internet | A domain or your public address; your own network's players use the inside address | `[network]` |

Don't use `127.0.0.1` when anyone else plays: the server would tell them to find your games at that address.

## With a domain

Games accept a name as the gateway, and openbnet doesn't need to know it. Make a DNS record that points straight at the
server:

| Type | Name | Content |
|---|---|---|
| A | `bnet.example.com` | The server's public IPv4 address |

- Games use IPv4, so an AAAA record alone isn't enough.
- Open TCP and UDP 6112 in the server's firewall, such as `sudo ufw allow 6112`, and in the hosting provider's.
- At home, forward TCP and UDP 6112 on the router to the server. If your address changes, use dynamic DNS. `public_ip` takes
  only an address, so update it too and reload.
- Players add it with `wc2bne gateway add bnet.example.com "My server"`, or any other gateway editor.

## Players inside the server's network

At home, players inside and outside need different addresses for a game hosted inside:

```toml
[network]
public_ip = "203.0.113.9"          # yours
local = ["192.168.1.0/24"]         # your network
```

- Players inside must connect to the inside address, such as `192.168.1.10`. Through the public name, the server may see
  your router instead of them. A DNS server in your network can give the name its inside address.
- Anyone inside who hosts for outside players needs their UDP game port forwarded to their computer. Two computers need two
  ports: `wc2bne config set port 6113`.
- The server's own computer can't use 6112 for a game, because the server has it.
- A forwarded port that differs from the game's own goes in `[[network.override]]`.

Players elsewhere who share an address, such as two in one house, also need different game ports.

## Cloudflare

| Part | Cloudflare Tunnel | Proxied DNS (orange cloud) | DNS only (grey cloud) |
|---|---|---|---|
| Games: TCP and UDP 6112 | No | No | **Yes** |
| IRC | No | No | Yes |
| Web API and WebSocket chat | **Yes** | Yes, with a proxy on 443 | Yes, with a proxy on 443 |

Cloudflare's tunnel needs `cloudflared` on the visitor's computer for anything but web traffic and has no UDP, and its
ordinary proxy doesn't cover 6112
([protocols](https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/routing-to-tunnel/protocols/),
[ports](https://developers.cloudflare.com/fundamentals/reference/network-ports/)). Spectrum can proxy such ports, on
Enterprise plans. It isn't enough here anyway: openbnet would see Cloudflare's addresses instead of the players', and players
couldn't reach each other's games. The same goes for other relays, such as frp or an HAProxy in front of 6112.

So use two names:

| Name | Record | For |
|---|---|---|
| `bnet.example.com` | A, **DNS only** | Games |
| `chat.example.com` | The tunnel's CNAME, proxied | Web API |

The game's name shows your address. To keep a home address private, run the server on a VPS, or put all players on a VPN such
as Tailscale or WireGuard and give them the server's VPN address; they can then also reach each other's games.

## Friends over Tailscale

No ports, no domain, and nobody else can reach the server. Everyone installs [Tailscale](https://tailscale.com) and joins your
tailnet, or you share the server's computer with theirs. Players hosting games must be reachable by the others too, so for
matches everyone should be in one tailnet.

```sh
openbnet setup --for vpn --address "$(tailscale ip -4)"
```

Not tested with the game yet.

## The Cloudflare tunnel

In `data/openbnet.toml`, keeping `listen` on `127.0.0.1`:

```toml
[http]
behind_proxy = true
```

`~/.cloudflared/config.yml`:

```yaml
tunnel: YOUR-TUNNEL-ID
credentials-file: /home/you/.cloudflared/YOUR-TUNNEL-ID.json
ingress:
  - hostname: chat.example.com
    path: ^/api/admin
    service: http_status:404
  - hostname: chat.example.com
    service: http://127.0.0.1:6180
  - service: http_status:404
```

The first rule keeps administration off the internet. For a tunnel managed in the dashboard, add a public hostname
with service `http://127.0.0.1:6180`, and above it one with path `^/api/admin` and service `http_status:404`. Browsers connect to
`wss://chat.example.com/ws`. If `cloudflared` runs in a container, `127.0.0.1` is that container: run it with host networking.

`behind_proxy` makes the limits on wrong passwords and sign-ups count each visitor, whose address Cloudflare adds last to
`X-Forwarded-For`. Without it, all visitors share one limit.

## Web API with your own proxy

Caddy, which gets a certificate itself:

```caddyfile
bnet.example.com {
	@admin path /api/admin/*
	respond @admin 404
	reverse_proxy 127.0.0.1:6180
}
```

Set `behind_proxy = true`. If Cloudflare's orange cloud is also in front of Caddy, set Caddy's `trusted_proxies` so that the
visitor's address survives. Other proxies must support WebSocket and put the visitor last in `X-Forwarded-For`.

## IRC with TLS

```toml
[irc]
tls_listen = "0.0.0.0:6697"
certificate = "fullchain.pem"
key = "privkey.pem"
```

Copy the certificate for your domain into the data folder, readable by the server's user. Restart after each renewal.

## As a service

```sh
sudo install target/release/openbnet /usr/local/bin/
sudo useradd --system --home /var/lib/openbnet openbnet
sudo install -d -o openbnet -g openbnet /var/lib/openbnet/files
```

`/etc/systemd/system/openbnet.service`, also in [`deploy/openbnet.service`](../deploy/openbnet.service):

```ini
[Unit]
Description=openbnet
Wants=network-online.target
After=network-online.target

[Service]
ExecStart=/usr/local/bin/openbnet --data /var/lib/openbnet serve
ExecReload=/bin/kill -HUP $MAINPID
User=openbnet
Group=openbnet
StateDirectory=openbnet
Restart=on-failure
LimitNOFILE=8192
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now openbnet
sudo -u openbnet openbnet --data /var/lib/openbnet create-account YourName --role admin
journalctl -u openbnet -f
```

To bring an existing folder along, stop the old server first, copy `data/` there, and
`sudo chown -R openbnet: /var/lib/openbnet`.

## As a container

### Docker Compose

```sh
mkdir -p data                 # yours, or the server can't write to it
docker compose build
docker compose run --rm openbnet setup
docker compose up -d
docker compose exec openbnet openbnet create-account YourName --role admin
```

| To | Run |
|---|---|
| Follow the log | `docker compose logs -f openbnet` |
| Apply settings | `docker compose kill -s HUP openbnet` |
| Update | `docker compose up -d --build` |
| Add the web API's tunnel | `docker compose --profile tunnel up -d`, with `TUNNEL_TOKEN` |
| Join a tailnet | `docker compose --profile tailscale up -d`, with `TS_AUTHKEY`. Not if this computer already runs Tailscale |

Put `TUNNEL_TOKEN`, `TS_AUTHKEY`, and `OPENBNET_UID` and `OPENBNET_GID` if yours aren't 1000, in a git-ignored `.env`. The tunnel's
rules, including the one blocking `/api/admin`, are then set in Cloudflare's dashboard.

It uses host networking, so Linux only. The container has a read-only system, no privileges, a health check and limited logs.

### Docker alone

```sh
docker build -t openbnet .
docker run -d --name openbnet --restart unless-stopped --network host \
  --user "$(id -u):$(id -g)" -v "$PWD/data:/data" openbnet
docker exec -it openbnet openbnet create-account YourName --role admin
```

Host networking (Linux) lets the server see players' real addresses, and the API stays on `127.0.0.1`. With published ports
instead (`-p 6112:6112/tcp -p 6112:6112/udp`), players on the same computer appear as Docker's address, and the API needs
`listen = "0.0.0.0:6180"` with `-p 127.0.0.1:6180:6180`. `--user` must own the data folder.

## Backups and updates

Stop the server and copy the data folder. Or, while it runs: `sqlite3 data/openbnet.db ".backup copy.db"`. Don't copy
`openbnet.db` alone while it runs; recent changes are in `openbnet.db-wal`. With PostgreSQL, use `pg_dump`.

To update, back up, replace the program and restart. The database is updated at startup and can't go back.

## Stopping and watching

Stopping (`SIGTERM`, Ctrl+C) warns players for `shutdown_secs`, then stores results already reported. Matches go on between the
players, but their results are lost. `openbnet health` and `GET /api/health` tell whether it works; the second also asks the
database. There are no metrics beyond `GET /api/status` and the log.

## Before opening it to the internet

Defaults already allow only Warcraft II, 16 connections and 5 sign-ups an hour per address, and leave bots, IRC and
administration over HTTP off. An error in one connection ends only that connection.

- Appoint moderators (`set-role NAME moderator`) and learn `/kick`, `/mute`, `/lock` and `/ipban`.
- Make yourself an administrator, then decide on `allow_new`.
- Older games log in with an unsalted hash, unencrypted. Tell players not to reuse a password, and keep backups private.
- Keep `http.listen` on `127.0.0.1`, `/api/admin` blocked, and `admin_token` empty unless you use it.
- Leave `allow_unknown` off, and leave the version-check files out unless you need them. Leave bots and plain IRC off unless you need them.

## When something doesn't work

| Seen | Likely cause |
|---|---|
| "unable to properly identify your application version" | The game needs a [version-check file](CONFIGURATION.md#version-checks) that is missing or damaged, or `strict_versions` refused the build. For Warcraft II, remove the file and turn strict mode off |
| Some players of one household can't connect | `max_connections_per_ip` |
| Can't connect at all | TCP 6112 closed, a proxied DNS record, or a tunnel. Try `nc -vz bnet.example.com 6112` |
| A plug beside a name | That player's UDP doesn't work |
| A game is listed but can't be joined | The host's UDP port isn't open, or the host connected through `127.0.0.1` or a relay |
| Works inside the network only | The router doesn't forward, or the provider shares one address among customers (CGNAT): use a VPS |
| Others inside can't join a host inside | The host connected through the public name; use the inside address |
| "attempt to write a readonly database" | The data folder belongs to another user |
| Everyone on the web is refused at once | `behind_proxy` is off behind a proxy |
| A setting has no effect | It needs a restart, or the log shows the file was refused |
