# Help text. Everything here is pure.

help_main() { cat <<'E'
Play Warcraft II: Battle.net Edition on Linux.

usage: wc2bne COMMAND [--edition community|official] [ARGS]

commands:
  install   install an edition of the game
  uninstall remove an edition
  play      start the game
  open      open the game folder
  status    show the data folder and what is installed
  config    show and change settings
  gateway   list and change the servers the game offers
  doctor    check for missing programs, and offer to install them
  exec      run a command inside the sandbox
  help      show help for a command

editions:
  community   2.0.2.0 with plugins, for PvPGN servers (default)
  official    2.0.2.1 (GoG), for Blizzard's server, with your own CD key

other options: --help, --version
E
}

# help_command COMMAND  -> its help, or fails if there is no such command.
help_command() {
  case $1 in
    install) cat <<'E'
usage: wc2bne install [--edition E] [--porcelain]
Installs an edition. The installer is WC2BNE_SETUP if set, else a copy kept earlier, else a download.
--porcelain prints step=download, prepare, install, finish, done, and percent=N while downloading.
E
    ;;
    uninstall) cat <<'E'
usage: wc2bne uninstall [--edition E] [--yes]
Deletes an edition's game folder, with its saved games and settings. Maps and the other edition are kept.
Asks first, unless --yes is given.
E
    ;;
    play) cat <<'E'
usage: wc2bne play [--edition E] [--fullscreen|--window]
Starts the game.
E
    ;;
    gateway) cat <<'E'
usage: wc2bne gateway [--edition E] list [--porcelain]
       wc2bne gateway [--edition E] add HOST NAME [GMT-OFFSET]
       wc2bne gateway [--edition E] select NUMBER
       wc2bne gateway [--edition E] remove NUMBER
       wc2bne gateway [--edition E] set < LIST
       wc2bne gateway [--edition E] reset
Lists and changes the servers the game offers. Changes make your own list, which servers can't replace;
reset goes back to the game's. --porcelain prints NUMBER, SELECTED (1 or 0), HOST, GMT-OFFSET and NAME,
separated by tabs, in a format that won't change; set reads the same format.
E
    ;;
    config) cat <<'E'
usage: wc2bne config list [--porcelain]
       wc2bne config get NAME
       wc2bne config [--edition E] set NAME VALUE
Shows and changes settings:
  edition   community or official     used when --edition isn't given
  mode      fullscreen or window      used when play isn't told
  port      1024-65435                UDP game port, kept separately for each edition
  data-folder  a path, or default     where everything is kept; nothing is moved. WC2BNE_HOME beats it
--porcelain prints NAME=VALUE lines in a format that won't change.
E
    ;;
    open) cat <<'E'
usage: wc2bne open [--edition E] [--maps]
Opens the game folder, or the shared maps. Don't double-click the game there: start it with wc2bne
play, which runs it in the sandbox.
E
    ;;
    status) cat <<'E'
usage: wc2bne status [--porcelain]
Shows the data folder, the settings in effect, and whether each edition is installed, changed (a file that
servers check was modified) or absent. --porcelain prints NAME=VALUE lines in a format that won't change.
E
    ;;
    doctor) cat <<'E'
usage: wc2bne doctor [--porcelain]
Checks for the programs wc2bne needs. On a terminal, offers to install missing ones; nothing is installed
without a yes. --porcelain only reports: NAME=ok, missing or optional, then install=COMMAND lines, which need
to run as root and don't ask questions.
E
    ;;
    exec) cat <<'E'
usage: wc2bne exec [--edition E] -- COMMAND...
Runs a command inside the edition's sandbox, with network access.
E
    ;;
    *) return 1;;
  esac
}
