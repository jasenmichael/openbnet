# Plan

## Can it be done?

Probably. Two things decide it, and M0 tests both first: whether an engine is fast enough, and
whether UDP hosting works through a relay. Evidence is in [RESEARCH.md](RESEARCH.md).

| Wanted | Answer |
|---|---|
| One site, game runs in each player's browser | Yes |
| Sessions persist | Yes, in browser storage, with export and import as backup |
| PvPGN, openbnet, war2.ru | Expected, through the relay |
| Blizzard's server | Expected with `official` and the player's own key; untested even on Linux |
| Purely static hosting | Offline play only. Online needs the relay, which can run on the same site |
| Every function | Not promised: map editor (separate program), .NET config tool (replaced by our page) |

## Design

```text
Browser tab                                              Server (one machine)
  page: install, gateways, settings, backup                static files (COOP/COEP headers)
  worker: engine + Wine + original client                  relay
  OPFS: game folder, registry, maps  --- WSS / WebTransport ---  |-- TCP 6112 --> Battle.net server
                                                                 |-- UDP, own port <--> other players
```

- **Engine:** Boxedwine, behind a small interface (`start`, `stop`, `mount`, `net`, `video`, `audio`, `input`)
  so v86 can replace it.
- **Storage:** base Wine image is read-only and cached; only differences, the game folder, registry and `Maps` are saved.
  OPFS, `navigator.storage.persist()`, writes flushed while playing and on exit. Export/import as one file, because
  browsers may evict data and private windows lose it. Export holds keys and client IDs: warn before saving it.
- **Editions:** `community` and `official` have separate registries and shared maps, as in `wc2bne-linux`.
- **One tab per profile** (Web Locks). A second profile allows two players in one browser.
- **Gateways:** the page edits the override list, so any server can be added.
- **Threads** need SharedArrayBuffer, so the site must be HTTPS with COOP/COEP. Emulation runs in a worker so a
  hidden tab doesn't stall a lockstep match; check this per browser.

## Networking

Own protocol, small, over WSS first; WebTransport datagrams for UDP later to avoid TCP stalls. Operations:
open (get public IP and UDP port), resolve, TCP connect/data/close, UDP send/receive, close.

1. On open, the relay reserves a public UDP port for the session and the page sets `Game Data Port` to it.
2. The guest's network card carries the relay's public IP, and guest port = public port.
3. TCP and UDP leave from that same IP. Two sessions on one relay are delivered to each other inside it.
4. A dropped connection can resume briefly with a secret token and keeps its port.

### Relay rules

- TCP only to port 6112 by default; a host is resolved once, and that address is used. UDP ports 1024-65535 only.
- UDP: packets go only to the session's server and to peers; new peers per minute, packets and bytes are limited.
  Unknown senders are accepted only on a session that logged in, with a size limit.
- Private, loopback, link-local, multicast and the relay's own addresses are blocked, after resolving. A self-hoster
  can allow named ones, for a LAN openbnet.
- Check `Origin`, limit sessions per address and total, time out idle ones. No payload logs.
- WinINet traffic (80/443) is blocked unless the owner decides otherwise; this affects `war2mod.w2p`.

**Costs to accept:** the relay's operator can see unencrypted Battle.net traffic; all its players share an IP, so one ban
hits everyone; it adds delay; its operator handles abuse reports. Ask war2.ru's operators before launch.
Anyone can run their own relay, and openbnet could include one later.

## Security

The server-supplied DLL runs inside the emulator with no access to the player's files, and the relay rules limit its network.
It can still read that edition's registry and key, so editions stay separate. Strict CSP; nothing third-party on the page.

## Milestones

Each has a pass/fail test; stop and reconsider on failure.

| | Work | Done when |
|---|---|---|
| **M0a** | Build Boxedwine for web; mount a throwaway game folder; measure. If too slow, the same with v86 | Main menu and a skirmish at normal speed with sound on the owner's PC in Chrome and Firefox; frame time and CPU recorded |
| **M0b** | Relay prototype; browser-like test client, then the real client under Wine through it | Local PvPGN shows the relay's IP and the right UDP port, and an outside client joins a hosted match |
| **M1** | Page: pick installer, verify, unpack in the browser, play offline | Fresh browser to skirmish; no game file on the server |
| **M2** | Storage, profiles, backup | Saves, maps, settings and registry survive restart; export restores in another browser |
| **M3** | Relay in the engine; gateways | Login, chat, map download, hosting and joining against local PvPGN and openbnet, browser-to-browser and browser-to-PC |
| **M4** | Relay hardening, load test, deployment | Every relay rule tested; security review |
| **M5** | Owner tests on war2.ru, then Blizzard | Results recorded per server |
| **M6** | WebTransport, fullscreen and input polish, page for settings, browser matrix | Phones and Safari reported, not promised |

Unpacking: run the installer in the engine silently, as `wc2bne-linux` does, or use `innoextract` in Wasm. Choose in M1.

## Proposed layout

```text
engine/   build scripts, our patches to the engine     web/    page, worker, storage, network client
relay/    Rust, like openbnet                           test/   relay, storage and browser tests
```

## Decisions for the owner

1. Relay: run a public one, or self-host only? Recommended: public only after M4, with war2.ru's agreement.
2. Is asking players to pick the installer themselves fine? Recommended, as it avoids hosting Blizzard's files.
3. GPL-2.0 for the engine build (required by Boxedwine), and a licence for our own code?
4. Is about 60 MB of engine and Wine download acceptable? The figure is a guess until M0a.
5. Start M0a and M0b? M0b needs a local PvPGN container, which the project rules say to ask about.
