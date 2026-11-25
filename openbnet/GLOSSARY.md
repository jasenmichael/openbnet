# openbnet

A Battle.net-compatible server for Blizzard's classic games, based on PvPGN.

## Language

**Game client**:
An original, unmodified Blizzard game connecting to the server, unless a supported game is documented as needing a loader.
_Avoid_: Client (when a chat client could be meant), app

**Chat client**:
Anything connected to chat that isn't a game: a bot, or a telnet, IRC or WebSocket user.
_Avoid_: Bot (as the general term)

**Battle.net protocol**:
The protocols spoken by Blizzard's classic games and their chat clients. It excludes Westwood Online.
_Avoid_: PvPGN protocol

**Account**:
A player's single identity on the server, shared by every game and chat client.
_Avoid_: User, profile

**Credential**:
What an account stores to check a password under one game family's scheme. An account can hold several.
_Avoid_: Password, hash

**Role**:
A named set of permissions given to an account.
_Avoid_: Command group, flag

**Open Battle.net**:
Diablo II play in which characters stay on the player's computer.

**Closed realm**:
Diablo II play in which the server keeps the characters and an external D2GS runs the games.
_Avoid_: Realm (alone)

**Version check**:
The login step in which a game client proves which build it is by running code from a server-supplied file.
_Avoid_: CheckRevision, auth

**Loader**:
A tool that changes a game client so it accepts a server not run by Blizzard.
_Avoid_: Patch, crack

**Reference server**:
The PvPGN version that openbnet's behavior is compared with.
_Avoid_: Upstream, the old server
