# Operators supply the version-check files

Game clients run code from the version-check MPQ a server sends, and PvPGN's license may not cover the MPQ and icon files it ships. openbnet doesn't include them. It serves files from an operator's directory. By default, MPQs are served only if their checksum is known, and other types only if games don't run code from them.
