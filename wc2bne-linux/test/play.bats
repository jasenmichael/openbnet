#!/usr/bin/env bats
# Starting the game.
load helper
setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "play needs an installed game" {
  run "$WC2BNE" play
  [ "$status" -ne 0 ]
  [[ $output == *"not installed"* ]]
}

@test "on Wayland, fullscreen plays in a nested display shaped like the screen, centered" {
  "$WC2BNE" install
  WAYLAND_DISPLAY=wayland-1 run "$WC2BNE" play
  [ "$status" -eq 0 ]
  grep -q -- '-geometry 854x480 .*-fullscreen' "$STUB_LOG/xwayland"
  played display=:42
  played PosX=107
  played PosY=0
  played ShowWindowFrame=No
  played Windowed=Yes
}

@test "on Wayland, --window plays in a 640x480 nested display" {
  "$WC2BNE" install
  WAYLAND_DISPLAY=wayland-1 run "$WC2BNE" play --window
  [ "$status" -eq 0 ]
  grep -q -- '-geometry 640x480 .*-decorate' "$STUB_LOG/xwayland"
  played PosX=0
}

@test "without Wayland, it plays in a Wine desktop" {
  "$WC2BNE" install
  run "$WC2BNE" play
  [ "$status" -eq 0 ]
  grep -q 'args=explorer /desktop=wc2bne,640x480' "$STUB_LOG/played"
  played PosX=0
}

@test "the game has network access, unless WC2BNE_OFFLINE is set" {
  "$WC2BNE" install
  "$WC2BNE" play
  played net=1
  WC2BNE_OFFLINE=1 "$WC2BNE" play
  played net=
}

@test "the sandbox never sees the home directory" {
  "$WC2BNE" install
  "$WC2BNE" play
  run grep -xF -e "$HOME" -e /home -e / "$STUB_LOG/bwrap"
  [ "$status" -ne 0 ]
}
