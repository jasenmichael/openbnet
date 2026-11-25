# wc2bne-linux

The launcher that installs and runs the original Warcraft II: Battle.net Edition on Linux.

## Language

**Edition**:
One of the game builds the installer offers, `community` or `official`. It decides which servers accept the player, not which server or account is used.
_Avoid_: Profile, version, account

**Data folder**:
The single movable directory holding everything the launcher creates: settings, the installer, shared maps, and one game folder per installed edition.
_Avoid_: Home, config dir, install dir

**Game folder**:
The directory of one installed edition, laid out like the game's folder on Windows.
_Avoid_: Profile directory

**Shared maps**:
The one collection of maps that every installed edition uses.

**Wine environment**:
The simulated Windows, including the registry, belonging to one edition.
_Avoid_: Prefix, bottle

**Installer**:
The War2Combat 4.6.1 setup program that the game files come from.
_Avoid_: Setup, exe

**Gateway**:
A Battle.net-compatible server the player can choose in the game.
_Avoid_: Realm, host

**Gateway list**:
The gateways the game offers. The **game's list** can be replaced by a server; the **override list** belongs to the player and takes precedence.

**Portable**:
The data folder can be moved or copied as a whole and still work. Wine and other dependencies come from the PC.
