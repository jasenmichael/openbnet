# Research

2026-11-25. **Checked** = read in local source or files. **Reported** = project docs or a fetched page, not run by us.
Nothing has been run in a browser yet.

## The client

| Finding | Basis |
|---|---|
| All PE32 i386. The exe imports only ADVAPI32, GDI32, KERNEL32, SHELL32, Storm, USER32, VERSION, WSOCK32 | Checked (`objdump`) |
| The game folder brings its own ddraw, crtdll and wsock32; its wsock32 forwards to WS2_32 and IPHLPAPI | Checked |
| `LC.dll` also needs GDI+; `war2mod.w2p` (pushed by a server) needs WinINet and urlmon | Checked |
| Game folder is 45 MB without Wine; 36.9 MB of it is one MPQ. Installer is 52,232,067 bytes | Checked |
| Login runs server-supplied x86 code over unmodified files, so an x86 emulator is unavoidable | `wc2bne-linux` notes |
| `Warcraft II Config.exe` is .NET; its settings would need our own page instead | Checked |

## Engines

| | Boxedwine | v86 + small Linux + Wine | v86 + Windows 9x/2000 | CheerpX + Wine | Own Win32 layer |
|---|---|---|---|---|---|
| What | Wine on an x86/Linux emulator | Whole PC, Linux inside | Whole PC | Linux processes | Our own API subset |
| CPU in browser | No JIT; README says web is "still slow" | Compiles to Wasm at run time | Same | JIT | Would have to write |
| Network | WebSocket gateway merged 2026-10-09 (PR 164), TCP + UDP | Ethernet over WebSocket; its Wisp client is TCP only | Same | Tailscale only | Would have to write |
| Licence | GPL-2.0 | BSD, plus the guest's | Needs a Windows licence per player | No self-hosting without a commercial licence | Ours |
| Verdict | **Try first** | **Try second** | Rejected: licence | Rejected: licence, Tailscale | Last resort |

All engine rows are Reported. Why Boxedwine first: it is the same Wine approach as `wc2bne-linux`, with the smallest
guest. Its speed is the main risk. The game is 8-bit 640x480 software-rendered, so a slow CPU may be enough; only
measurement can tell. GPL-2.0 means publishing the source of the engine build we serve.

PR 164's review raised gateway problems (origin checks, payload limits, path validation); whether they were fixed wasn't
visible. Whether its UDP accepts packets from arbitrary peers is **unknown**, and hosting needs that.

## Networking

Browsers have no raw TCP or UDP, so **a static site alone cannot reach these servers. One relay is required.**
It carries packets only; the game still runs in the tab.

| Finding | Basis |
|---|---|
| Login and chat: TCP 6112. Match data: UDP between players, port from `Game Data Port` | `wc2bne-linux` notes |
| PvPGN and openbnet take a player's UDP address from the source of their UDP packet | Checked: `handle_udp.cpp`, `openbnet/src/udp.rs` |
| A client can also state its port (0x45), or its IP and port (0x1B), over TCP; both servers believe it | Checked: `handle_bnet.cpp`, `openbnet/src/bncs.rs` |
| Which of those W2BN sends isn't known | Open |
| Stock PvPGN: `max_conns_per_IP = 0` (off). Other operators may limit it | Checked |
| Wisp has no bind or listen, so it can't receive from unknown peers | Reported (protocol v1.2) |
| `.vendor/pvpgn-websockify` forwards one fixed TCP target; no UDP, so chat but no matches | Checked |
| openbnet's `/ws` is JSON chat, not the game protocol | Checked: `docs/API.md` |

So each session needs its **own public UDP port, the same number inside and outside**, and TCP and UDP must leave from the
same public IP. If the guest is also told that IP is its own, 0x1B, 0x45 and the UDP source all agree, and
players on ordinary PCs can join a browser player's game. Blizzard's and war2.ru's behavior is untested.

## Files

Neither installer host sent `Access-Control-Allow-Origin` to a request with a foreign `Origin`, so a page can't
download it directly (Checked 2026-11-25). The player downloads it and picks the file.
