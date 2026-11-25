# HTTP and WebSocket API

On `http.listen`, by default `127.0.0.1:6180`, without TLS. To publish it, put a proxy or a Cloudflare Tunnel in front, set
`http.behind_proxy = true`, and block `/api/admin`; see [DEPLOYMENT.md](DEPLOYMENT.md#cloudflare). Bodies are JSON. Errors
look like `{"error": "name taken"}`.

## For everyone

| Request | Body | Answer |
|---|---|---|
| `GET /api/health` | | 200, or 503 if the database doesn't answer |
| `GET /api/status` | | name, version, users, games, uptime_secs |
| `GET /api/channels` | | list of name, users |
| `GET /api/games` | | list of product, name, players |
| `POST /api/accounts` | name, password | 201; 400, 403 or 409 |
| `POST /api/password` | name, password, new_password | 200; 401 |

Banned addresses get 403. Passwords are 1 to 64 bytes and ignore case, because the games' do.

## Administration

Needs `Authorization: Bearer <http.admin_token>`. Off while the token is empty. Meant for use on the server itself:

```sh
curl -H "Authorization: Bearer $TOKEN" -X POST http://127.0.0.1:6180/api/admin/reload
```

| Request | Body | Effect |
|---|---|---|
| `GET /api/admin/accounts/{name}` | | id, name, role, locked, muted, last_login_at, online |
| `PATCH /api/admin/accounts/{name}` | any of role, locked, muted, password | Changes it; locking disconnects |
| `POST /api/admin/accounts/{name}/disconnect` | | Disconnects it |
| `GET /api/admin/bans` | | List of network, reason, by, until (Unix time; 0 for good) |
| `POST /api/admin/bans` | target (address, range or player); hours, reason | Bans it. 0 hours means for good |
| `DELETE /api/admin/bans` | network | Lifts it |
| `POST /api/admin/announce` | text | Shows it to everyone |
| `POST /api/admin/reload` | | Reads the settings again, as `SIGHUP` does |

## Chat: `/ws`

`ws://127.0.0.1:6180/ws`, or `wss://your-name/ws` through a proxy.

Each message is a JSON object with a `type`.

| You send | |
|---|---|
| `{"type": "login", "name": "Jaina", "password": "..."}` | First |
| `{"type": "join", "channel": "War2BNE"}` | One channel at a time |
| `{"type": "say", "text": "hello"}` | Talk, or a command such as `/w Thrall hi`; up to 255 bytes |

| You get | Fields |
|---|---|
| `logged_in` | name |
| `problem` | code: bad_request, not_logged_in, already_logged_in, wrong_name_or_password, locked, bad_channel, bad_text |
| `channel` | channel: you are now in it. A `user` for each person there follows, then `entered` |
| `entered` | channel |
| `user`, `join`, `leave`, `flags` | name, flags, product |
| `talk`, `emote`, `whisper`, `whisper_sent` | name, text. Your own talk isn't sent back; your emotes are |
| `info`, `error`, `broadcast` | text |

`flags` adds up 1 (administrator), 2 (channel operator), 8 (moderator) and 16 (no UDP). `product` is `W2BN`, `STAR`,
`SEXP`, `DRTL`, `CHAT` (bot or telnet), `IIRC` (IRC), `WEBC` (this API), and so on.
