# Port PvPGN openly under GPL-3.0-or-later

PvPGN's source headers allow GPL version 2 or any later version, and openbnet may reuse its code, so openbnet can't be relicensed permissively. We chose GPL-3.0-or-later because it is compatible with Apache-2.0 Rust libraries. Ported code must keep credit to PvPGN and its authors.

## Considered Options

- **GPL-2.0-or-later**: matches PvPGN, but the combined work would become GPLv3 once an Apache-2.0-only library is used.
- **Clean-room with a permissive license**: needs a separate specification process and gives up reuse of PvPGN code.
