# One account, one credential per password scheme

Older games send a password hash, and Warcraft III uses SRP. Neither stored form can produce the other, so an account holds a separate credential for each scheme. Unlike PvPGN, a missing or corrupt credential always refuses the login or password change, and the stored hash isn't accepted as proof in Warcraft III logins unless an operator enables it for a loader.

## Consequences

- A player may need to set their password through the API before using a game with the other scheme.
- The older scheme's hash is unsalted and enough to log in with, so a leaked database exposes those accounts.
