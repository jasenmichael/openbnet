# Showing the game. The client draws Battle.net screens as many separate top-level windows, which a compositor
# would manage one by one. So give it a private X display with no window manager: Xwayland shows that display
# as a single window and scales it in fullscreen. (Wine's Managed=N looks right too, but any click on the
# background kills input.)

# fullscreen_width SCREEN_W SCREEN_H  -> width, at 480 high, shaped like the screen so scaling doesn't stretch
# the 4:3 picture. At least 640; 640 if the size is unknown. Pure.
fullscreen_width() {
  local w=640
  if [ "${1:-0}" -gt 0 ] && [ "${2:-0}" -gt 0 ]; then w=$(( (480 * $1 / $2 + 1) / 2 * 2 )); fi
  printf '%s\n' $(( w < 640 ? 640 : w ))
}

# screen_size  -> "WIDTH HEIGHT" of the first active screen, or "0 0". Runs xrandr.
screen_size() {
  local size; size=$({ xrandr 2>/dev/null || true; } | awk '/\*/ && !n { sub("x", " ", $1); print $1; n = 1 }')
  printf '%s\n' "${size:-0 0}"
}

# ddraw_ini X  Filters war2_ddraw.ini from stdin to stdout: frameless and pinned at X,0, because the wrapper
# saves its last position on exit. Pure.
ddraw_ini() {
  sed -E "s/\r$//; s/^Windowed=.*/Windowed=Yes/; s/^ShowWindowFrame=.*/ShowWindowFrame=No/; s/^PosX=.*/PosX=$1/; s/^PosY=.*/PosY=0/; s/$/\r/"
}

# place_window DATA EDITION X  Rewrites the edition's war2_ddraw.ini. Writes a file.
place_window() {
  local ini; ini=$(game_dir "$1" "$2")/war2_ddraw.ini
  local text; text=$(ddraw_ini "$3" <"$ini")
  printf '%s\n' "$text" >"$ini"
}

# start_x fullscreen|window WIDTH  Starts a private X display and exports DISPLAY for it; stopped on exit.
# The one function that must set globals: a trap can't be given arguments later.
X_PID='' X_FD_FILE=''
stop_x() {
  if [ -n "$X_PID" ]; then kill "$X_PID" 2>/dev/null || true; fi
  if [ -n "$X_FD_FILE" ]; then rm -f "$X_FD_FILE"; fi
}
start_x() {
  local a=(-geometry "$2x480" -noreset -nolisten tcp) i
  if [ "$1" = fullscreen ]; then a+=(-fullscreen); else a+=(-decorate); fi
  X_FD_FILE=$(mktemp); trap stop_x EXIT; trap 'exit 130' INT TERM
  Xwayland "${a[@]}" -displayfd 3 3>"$X_FD_FILE" 2>/dev/null 9>&- & X_PID=$!
  for i in {1..50}; do
    if [ -s "$X_FD_FILE" ]; then
      DISPLAY=":$(tr -d '\n' <"$X_FD_FILE")"; export DISPLAY XAUTHORITY=/nonexistent
      return 0
    fi
    kill -0 "$X_PID" 2>/dev/null || break
    sleep 0.1
  done
  stop_x; X_PID=''
  return 1
}

# run_game DATA EDITION fullscreen|window WAYLAND  Plays until the game exits. WAYLAND is empty without Wayland,
# and then, or if Xwayland won't start, the game gets a Wine desktop instead.
run_game() {
  local data=$1 edition=$2 mode=$3 wayland=$4 w=640 size
  if [ "$mode" = fullscreen ]; then
    read -r -a size <<<"$(screen_size)"
    w=$(fullscreen_width "${size[@]}")
  fi
  if [ -n "$wayland" ] && command -v Xwayland >/dev/null && start_x "$mode" "$w"; then
    place_window "$data" "$edition" $(( (w - 640) / 2 ))
    box "$data" "$edition" net sh -c 'cd "$WINEPREFIX/drive_c/war2" && exec wine "$0"' "$EXE"
  else
    place_window "$data" "$edition" 0
    box "$data" "$edition" net sh -c 'cd "$WINEPREFIX/drive_c/war2" && exec wine explorer /desktop=wc2bne,640x480 "C:\\war2\\$0"' "$EXE"
  fi
}
