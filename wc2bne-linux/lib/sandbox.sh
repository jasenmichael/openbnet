# The sandbox. On login the Battle.net client downloads an MPQ from the server and runs the CheckRevision
# DLL inside it, so any server can run code. See docs/SPEC.md, Security, before changing this file.

# box DATA EDITION net|offline [--installer FILE] CMD...  Runs CMD in the edition's sandbox. The game folder is
# C:\war2, with Wine's own files hidden from it; C:\Maps is the shared maps, where the game folder's Maps link
# leads. FILE appears read-only as /home/box/setup.exe. WC2BNE_OFFLINE always wins over net.
box() {
  local data=$1 edition=$2 net=$3 installer=''; shift 3
  if [ "$1" = --installer ]; then installer=$2; shift 2; fi
  local game wine maps; game=$(game_dir "$data" "$edition") wine=$(wine_dir "$data" "$edition") maps=$(maps_dir "$data")
  local war2=/home/box/.wine/drive_c/war2 rt=${XDG_RUNTIME_DIR:-/run/user/${UID}} a resolv n xauth=${XAUTHORITY:-$HOME/.Xauthority}
  a=(--ro-bind /usr /usr --ro-bind /etc /etc
     --symlink usr/bin /bin --symlink usr/sbin /sbin --symlink usr/lib /lib
     --symlink usr/lib64 /lib64 --symlink usr/lib32 /lib32
     --proc /proc --dev /dev --dev-bind-try /dev/dri /dev/dri --dev-bind-try /dev/snd /dev/snd
     --ro-bind-try /sys /sys --tmpfs /tmp --tmpfs /run --tmpfs /var
     --dir /home/box --bind "$wine" /home/box/.wine --bind "$maps" /home/box/.wine/drive_c/Maps
     --bind "$game" "$war2" --tmpfs "$war2/.wine" --chdir /home/box
     --unshare-all --die-with-parent --new-session --clearenv
     --setenv HOME /home/box --setenv PATH /usr/bin:/bin --setenv LANG "${LANG:-C.UTF-8}"
     --setenv WINEPREFIX /home/box/.wine
     --setenv WINEDEBUG "${WINEDEBUG:--all}"
     --setenv WINEDLLOVERRIDES 'mscoree,mshtml=;ddraw,wsock32,crtdll=n,b')
  if [ -n "$installer" ]; then a+=(--ro-bind "$installer" /home/box/setup.exe); fi
  if [ "$net" = net ] && [ -z "${WC2BNE_OFFLINE:-}" ]; then
    a+=(--share-net)
    resolv=$(readlink -f /etc/resolv.conf || true)
    case $resolv in /run/*) a+=(--ro-bind "$resolv" "$resolv");; esac
  fi
  if [ -n "${DISPLAY:-}" ]; then
    n=${DISPLAY#*:}; n=${n%%.*}
    a+=(--setenv DISPLAY ":$n" --ro-bind-try "/tmp/.X11-unix/X$n" "/tmp/.X11-unix/X$n")
    if [ -f "$xauth" ]; then a+=(--ro-bind "$xauth" /run/xauth --setenv XAUTHORITY /run/xauth); fi
  fi
  if [ -S "$rt/pulse/native" ]; then
    a+=(--ro-bind "$rt/pulse/native" /run/pulse.sock --setenv PULSE_SERVER unix:/run/pulse.sock)
  fi
  # Leaving the sandbox kills wineserver; wait so it can write the registry to disk.
  bwrap "${a[@]}" sh -c '[ -d "$WINEPREFIX/drive_c" ] && cd "$WINEPREFIX/drive_c"; "$@"; rc=$?; wineserver -w; exit $rc' sh "$@" 9>&-
}
