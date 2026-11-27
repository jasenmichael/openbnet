# Running openbnet on a VPS

A VPS has its own public address, so players connect straight to it: no router, no port forwarding, no VPN. These steps are
for a fresh Ubuntu 24.04 or Debian 12 server, such as a DigitalOcean droplet. Pick one way: [without Docker](#without-docker)
or [with Docker](#with-docker).

## Before you start

1. **Create the VPS.** Ubuntu 24.04; the smallest size, 1 CPU and 1 GB of memory, is enough. Building peaks at about 700 MB, so
   with 1 GB [add swap](#with-1-gb-of-memory) first. Running it needs little.
2. **Note its public IPv4 address**, such as `203.0.113.9`.
3. **Optional: a name.** Make an A record, such as `bnet.example.com`, pointing at that address. On Cloudflare, set it to
   **DNS only** (grey cloud); the orange cloud and tunnels can't carry games.
4. **Log in:** `ssh root@203.0.113.9`. Below, use your own address or name wherever `bnet.example.com` appears.

## Without Docker

**1. Install the tools and build.** About 3 minutes with 2 CPUs, longer with 1.

```sh
sudo apt update && sudo apt install -y build-essential git curl
curl -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
git clone https://github.com/jasenmichael/openbnet.git
cd openbnet/openbnet-server
cargo build --release
sudo install target/release/openbnet /usr/local/bin/
```

**2. Give it a user and a folder.**

```sh
sudo useradd --system --home /var/lib/openbnet openbnet
sudo install -d -o openbnet -g openbnet /var/lib/openbnet
```

**3. Write its settings.** `--signups false` if you'd rather make every account yourself.

```sh
sudo -u openbnet openbnet --data /var/lib/openbnet setup --for internet --address bnet.example.com --signups true
```

**4. Run it as a service** that starts with the server and restarts if it stops:

```sh
sudo cp deploy/openbnet.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now openbnet
```

**5. Open the port**, then [finish](#finish).

```sh
sudo ufw allow OpenSSH
sudo ufw allow 6112
sudo ufw enable
```

| To | Run |
|---|---|
| Make an account | `sudo -u openbnet openbnet --data /var/lib/openbnet create-account NAME` |
| See the log | `journalctl -u openbnet -f` |
| Apply changed settings | `sudo systemctl reload openbnet`. Settings are in `/var/lib/openbnet/openbnet.toml` |
| Restart | `sudo systemctl restart openbnet` |
| Update | `cd ~/openbnet && git pull && cd openbnet-server && cargo build --release && sudo install target/release/openbnet /usr/local/bin/ && sudo systemctl restart openbnet` |
| Back up | `sudo systemctl stop openbnet && sudo tar czf openbnet-backup.tgz -C /var/lib openbnet && sudo systemctl start openbnet` |

## With Docker

**1. Install Docker** with its official script, and get openbnet:

```sh
curl -fsSL https://get.docker.com | sudo sh
git clone https://github.com/jasenmichael/openbnet.git
cd openbnet/openbnet-server
```

**2. Build it.** About 3 minutes with 2 CPUs, longer with 1. The container runs as your user, so it can write to `data/`:

```sh
mkdir -p data
printf 'OPENBNET_UID=%s\nOPENBNET_GID=%s\n' "$(id -u)" "$(id -g)" > .env
sudo docker compose build
```

**3. Write its settings and start it.**

```sh
sudo docker compose run --rm openbnet setup --for internet --address bnet.example.com --signups true
sudo docker compose up -d
```

**4. Open the port**, then [finish](#finish). The container uses the server's own network, so the firewall applies to it as usual.

```sh
sudo ufw allow OpenSSH
sudo ufw allow 6112
sudo ufw enable
```

| To | Run, in `openbnet/openbnet-server` |
|---|---|
| Make an account | `sudo docker compose exec openbnet openbnet create-account NAME` |
| See the log | `sudo docker compose logs -f` |
| Apply changed settings | `sudo docker compose kill -s HUP openbnet`. Settings are in `data/openbnet.toml` |
| Restart | `sudo docker compose restart` |
| Update | `git pull && sudo docker compose up -d --build` |
| Back up | `sudo docker compose stop && tar czf openbnet-backup.tgz data && sudo docker compose start` |

It starts again after a reboot by itself.

## Finish

1. **Make yourself an administrator**, with "Make an account" above plus `--role admin`. It asks for a password.
2. **Check it from your own computer:** `nc -vz bnet.example.com 6112` should say it connected.
3. **Players add the gateway** in their game: `bnet.example.com`, or the address. With wc2bne:
   `wc2bne gateway add bnet.example.com "My server"`.
4. Players who **host** games need UDP 6112 open on their own router. Joining needs nothing.

If your provider has its own firewall, such as DigitalOcean's Cloud Firewalls, allow TCP and UDP 6112 there too.

## With 1 GB of memory

Building can run out of memory. Add 2 GB of swap first:

```sh
sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
```

## More

Everything else, such as a web chat page behind a proxy, IRC and backups while running, is in [DEPLOYMENT.md](DEPLOYMENT.md).
Settings are in [CONFIGURATION.md](CONFIGURATION.md). If something doesn't work, see
[When something doesn't work](DEPLOYMENT.md#when-something-doesnt-work).
