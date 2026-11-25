# Testing

Last updated 2026-11-25.

## Automated

`cargo test`: unit tests, and over real sockets `tests/warcraft2.rs` and `tests/community.rs` (simulated
games: all of those below, ladders, friends, topics, banners) and `tests/chat.rs` (bots, telnet, IRC with and without
TLS, WebSocket, HTTP).

### Comparison with PvPGN

`tests/reference.rs` runs one script against PvPGN and openbnet and compares every reply. Times, session tokens,
latency and the host's address are left out, as are the differences listed below. Last run 2026-11-25 against
PvPGN 1.99.7.2.1-PRO (commit 9cd173f): identical.

Start PvPGN without network access, tracking, custom icons or topics (image: see `../wc2bne-linux/docs/TESTING.md`):

```sh
docker network create --internal --subnet 172.31.78.0/24 openbnet-ref
docker run -d --rm --name openbnet-ref --network openbnet-ref --ip 172.31.78.2 --entrypoint sh wc2bne-pvpgn-test -c '
  cd /opt/pvpgn/etc/pvpgn &&
  sed -i -e "s/^track *=.*/track = 0/" -e "s/^motdfile.*/motdfile = none.txt/" -e "s/^quota *=.*/quota = no/" bnetd.conf &&
  sed -i "s/^custom_icons.*/custom_icons = false/" icons.conf &&
  sed -i "s/botlogin\"=\"false/botlogin\"=\"true/" bnetd_default_user.plain &&
  : > bnissue.txt && : > topics.conf && rm -f i18n/*/bnmotd.txt i18n/bnmotd.txt
  exec /opt/pvpgn/sbin/bnetd -f'
OPENBNET_REF=172.31.78.2:6112 OPENBNET_FILES=../.vendor/pvpgn-server/files \
  cargo test --test reference -- --ignored
```

Use a fresh container for each run; the script creates accounts. Set `OPENBNET_TRANSCRIPTS` to a directory to get
both transcripts.

Known differences:

- openbnet doesn't ask for an e-mail address (`0x59`).
- A player without working UDP keeps the plug icon; PvPGN drops it on joining a channel.
- PvPGN sends bots freed memory for some replies, such as `/whoami`; openbnet sends the text.
- PvPGN repeats user flags that haven't changed.
- Ladder lists: PvPGN's are always empty, because of an inverted check. openbnet fills them in.
- Ladder games against six others: PvPGN's formula gives wrong chances; see `src/ladder.rs`. The rest match its own
  functions to 11 digits.
- Not implemented: custom icons, mail, watches.
- Deliberate: ADR 4, and players see no IP addresses.

### Load

```sh
ulimit -n 20000
cargo test --release --test load -- --ignored --nocapture --test-threads 1
```

Clients and server share one 16-thread machine, with SQLite on disk. Every player logs in, joins a room of 40 and
talks; all lines must arrive.

| Players | All logged in after | Slowest reply | Result |
|---|---|---|---|
| 800 | 0.43 s | 15 ms | Pass |
| 2000 | 0.79 s | 41 ms | Pass |

These found two problems, both fixed: a listen queue of 128 overflowed, and a full disk sync per write was too slow.

### PostgreSQL

`OPENBNET_POSTGRES=postgres://... cargo test --test postgres -- --ignored`, with an empty database. Passed on
2026-11-25 with PostgreSQL 18.7.

### Deployment

2026-11-25, as written in [DEPLOYMENT.md](DEPLOYMENT.md): Caddy 2.11.9 in front (API and WebSocket work, `/api/admin` blocked),
`cloudflared` 2026.10.7's own check of the tunnel rules, `systemd-analyze verify` on the unit, and the container with host
networking and `--user`. Not run: a live tunnel, a certificate, the installed service, published container ports, games in a
container.

## Real games

Two `community` clients from `wc2bne-linux` (2.0.2.0) under Xvfb, throwaway data folders, different UDP ports,
openbnet on a local bridge address. 2026-11-25:

| Step | Result |
|---|---|
| Download and run the version check | Pass |
| Create accounts, log in, join the channel | Pass |
| Channel talk and whispers | Pass |
| Host, list, join, cancel, host again | Pass |
| Melee match over a minute, with in-game chat | Pass |
| Quit as a draw; result and both reports stored; `/stats` shows it | Pass |
| Back in the channel afterwards | Pass |

The community build reports checksum `0x270dfc2b`, not PvPGN's `0xb52bad87` for 2.0.2.0. So
`strict_versions = true` would refuse it.

## Not tested

- The real clients again since milestones 2 to 4 changed the shared code. The comparison with PvPGN was run again
- The `official` Warcraft II build, any other game, a match with a winner and loser, and a ladder match
- The internet, NAT and `[network]` settings with real games
- Real IRC clients, bots and browsers
- Terms of service text in the real game (added after the run above)
