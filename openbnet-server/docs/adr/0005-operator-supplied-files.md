# Ship no Blizzard files; skip the version check without one

At login, games download an MPQ the server names and run the DLL inside it. These MPQs carry Blizzard's signature.
Warcraft II refused one whose signature we changed, so writing our own looks blocked; one with no signature wasn't tried. PvPGN ships them and icon files with its code, but its
license can't cover files that aren't its authors' to license. Warcraft II also accepts being told its version is fine
without any file, so openbnet does that unless the operator adds the MPQ. Operators who do get only ones with a known
checksum, unless they allow others.

## Consequences

- Warcraft II needs no files, and by default runs no code from the server.
- Without the check, the server can't tell builds apart, so it lists games hosted by one build to the other.
- `strict_versions`, and games with the newer login (Diablo II, later StarCraft), still need the operator's file.
