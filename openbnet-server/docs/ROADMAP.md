# Roadmap

Each step is usable on its own.

1. **Done:** Warcraft II: accounts, chat, hosting, matches, results.
2. **Done:** bot, telnet, IRC (with TLS) and WebSocket chat; HTTP administration API; reloading settings; container
   image.
3. **Done, protocol-tested only:** StarCraft and Diablo I; ladders, friends, topics and advertisements.
4. **Done, protocol-tested only:** Diablo II open Battle.net.
5. Warcraft III (with a loader): SRP logins, matchmaking, clans.
6. Diablo II closed realms, with an external D2GS.

Before a public launch: an internet test with real games, days of running, a security review and packet fuzzing, CI and
releases, metrics, account deletion and a rule for how long data is kept.

Left out so far: mail, watches, custom icons, clans outside Warcraft III, translations, and some chat commands.

For hosting: a name instead of an address in `public_ip`, renewing IRC's certificate without a restart, and the PROXY protocol
are not there. Relaying matches is not planned.

Scripting: not planned, but the server's events should allow it.
