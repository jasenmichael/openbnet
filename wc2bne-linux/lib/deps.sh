# Programs the launcher needs from the PC, and how to install them.

# dep_present NAME WINE_PATH  Succeeds when the program is there. wine32 means Wine's 32-bit Windows libraries, looked
# for near WINE_PATH (the real path of wine), because the game is a 32-bit program. Reads files.
dep_present() {
  local d
  case $1 in
    wine32)
      for d in "${2%/bin/*}"/lib*/wine/i386-windows "${2%/bin/*}"/lib/*/wine/i386-windows; do [ ! -d "$d" ] || return 0; done
      return 1;;
    *) command -v "$1" >/dev/null;;
  esac
}

# missing_deps NAME...  -> the names that aren't there, one per line. Reads files.
missing_deps() {
  local d wine; wine=$(command -v wine || true)
  if [ -n "$wine" ]; then wine=$(readlink -f "$wine"); fi
  for d; do dep_present "$d" "$wine" || printf '%s\n' "$d"; done
}

# package_manager  -> apt, dnf, pacman, zypper, or nothing. Reads PATH.
package_manager() {
  local m
  for m in apt-get dnf pacman zypper; do
    if command -v $m >/dev/null; then printf '%s\n' "${m%-get}"; return; fi
  done
}

# package_for MANAGER NAME  -> the package that has the program. Pure.
package_for() {
  case $1:$2 in
    *:bwrap) echo bubblewrap;;
    apt:wine32) echo wine32:i386;;
    zypper:wine32) echo wine-32bit;;
    *:wine32) echo wine;;
    apt:Xwayland|zypper:Xwayland) echo xwayland;;
    dnf:Xwayland) echo xorg-x11-server-Xwayland;;
    pacman:Xwayland) echo xorg-xwayland;;
    apt:xrandr) echo x11-xserver-utils;;
    pacman:xrandr) echo xorg-xrandr;;
    *:xdg-open) echo xdg-utils;;
    *) echo "$2";;
  esac
}

# install_commands MANAGER PREFIX ASK NAME...  -> shell commands, one per line, that install the programs. PREFIX is
# "sudo " or empty; without ASK (empty) the package manager doesn't ask its own questions. Pure.
install_commands() {
  local manager=$1 prefix=$2 ask=$3 packages=() flag='' p d; shift 3
  for d; do
    p=$(package_for "$manager" "$d")
    if [[ " ${packages[*]} " != *" $p "* ]]; then packages+=("$p"); fi
  done
  case $manager in
    apt) if [[ " ${packages[*]} " == *:i386\ * ]]; then printf '%s\n' "${prefix}dpkg --add-architecture i386"; fi
         if [ -z "$ask" ]; then flag='-y '; fi
         printf '%s\n' "${prefix}apt-get update" "${prefix}apt-get install $flag${packages[*]}";;
    dnf) if [ -z "$ask" ]; then flag='-y '; fi
         printf '%s\n' "${prefix}dnf install $flag${packages[*]}";;
    pacman) if [ -z "$ask" ]; then flag='--noconfirm '; fi
         printf '%s\n' "${prefix}pacman -S --needed $flag${packages[*]}";;
    zypper) if [ -z "$ask" ]; then flag='--non-interactive '; fi
         printf '%s\n' "${prefix}zypper ${flag}install ${packages[*]}";;
  esac
}

# need_deps NAME...  Exits 4 unless all are there. On a terminal, first offers to install what is missing, showing
# the commands; nothing runs without a yes.
need_deps() {
  local missing commands prefix='' reply='' line
  missing=$(missing_deps "$@"); mapfile -t missing < <(printf '%s' "$missing${missing:+$'\n'}")
  [ ${#missing[@]} -gt 0 ] || return 0
  warn "missing: ${missing[*]}"
  if [ "$EUID" -ne 0 ]; then prefix='sudo '; fi
  commands=$(install_commands "$(package_manager)" "$prefix" ask "${missing[@]}")
  if [ -n "$commands" ]; then
    printf 'To install:\n%s\n' "$commands" | sed '2,$s/^/  /' >&2
    if [ -t 0 ]; then
      read -r -p 'Run these commands now? [y/N] ' reply || true
      if [[ $reply == [Yy] || $reply == [Yy][Ee][Ss] ]]; then
        # Read commands from fd 3 so they can still ask their own questions on the terminal.
        while IFS= read -r line <&3; do $line || break; done 3<<<"$commands"
        hash -r
        [ -n "$(missing_deps "${missing[@]}")" ] || return 0
        die "$E_MISSING_DEPENDENCY" "still missing: $(missing_deps "${missing[@]}" | tr '\n' ' ')"
      fi
    fi
  fi
  exit "$E_MISSING_DEPENDENCY"
}
