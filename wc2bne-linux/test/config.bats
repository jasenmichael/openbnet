#!/usr/bin/env bats
# Settings.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "settings have defaults" {
  "$WC2BNE" install
  run "$WC2BNE" config list --porcelain
  [ "$status" -eq 0 ]
  [ "$output" = $'edition=community\nmode=fullscreen\nport=6112' ]
}

@test "a setting is remembered in wc2bne.conf in the data folder" {
  "$WC2BNE" config set edition official
  run "$WC2BNE" config get edition
  [ "$output" = official ]
  grep -qx 'edition=official' "$DATA/wc2bne.conf"
  "$WC2BNE" install
  grep -q 'program.gog' "$STUB_LOG/setup-args"
}

@test "setting one thing keeps the others, and comments" {
  printf '# my notes\nmode=window\n' >"$DATA/wc2bne.conf"
  "$WC2BNE" config set edition official
  "$WC2BNE" config set edition community
  [ "$(cat "$DATA/wc2bne.conf")" = $'# my notes\nmode=window\nedition=community' ]
}

@test "the mode setting is used by play" {
  "$WC2BNE" install
  "$WC2BNE" config set mode window
  WAYLAND_DISPLAY=w "$WC2BNE" play
  grep -q 640x480 "$STUB_LOG/xwayland"
}

@test "options beat the environment, which beats wc2bne.conf" {
  "$WC2BNE" install
  "$WC2BNE" config set mode fullscreen
  export WAYLAND_DISPLAY=w WC2BNE_MODE=window
  "$WC2BNE" play
  grep -q 640x480 "$STUB_LOG/xwayland"
  "$WC2BNE" play --fullscreen
  grep -q 854x480 "$STUB_LOG/xwayland"
  WC2BNE_EDITION=official run "$WC2BNE" play
  [ "$status" -eq 3 ]
}

@test "unknown settings and bad values exit 2" {
  "$WC2BNE" install
  for a in "set colour red" "set mode sideways" "set edition other" "set port 80" "set port x" "get colour" "set mode" "frob"; do
    run "$WC2BNE" config $a
    [ "$status" -eq 2 ] || { echo "'$a' exited $status"; false; }
  done
}

@test "a bad value in wc2bne.conf is an error that names it" {
  echo 'mode=sideways' >"$DATA/wc2bne.conf"
  run "$WC2BNE" play
  [ "$status" -eq 2 ]
  [[ $output == *mode*sideways* ]]
}

@test "wc2bne.conf is never run as code" {
  echo 'edition=$(touch '"$T"'/pwned)' >"$DATA/wc2bne.conf"
  run "$WC2BNE" config list
  [ ! -e "$T/pwned" ]
}

@test "the UDP port is kept in the edition's registry" {
  "$WC2BNE" install
  "$WC2BNE" config set port 6113
  run "$WC2BNE" config get port
  [ "$output" = 6113 ]
  run grep port "$DATA/wc2bne.conf"
  [ "$status" -ne 0 ]
}

@test "config set repairs an invalid wc2bne.conf" {
  printf 'mode=sideways\nedition=other\n' >"$DATA/wc2bne.conf"
  "$WC2BNE" config set mode window
  "$WC2BNE" config set edition community
  [ "$(cat "$DATA/wc2bne.conf")" = $'mode=window\nedition=community' ]
}

@test "the chosen data folder is remembered, and WC2BNE_HOME still wins" {
  mkdir -p "$T/games" "$T/other"
  (cd "$T" && "$WC2BNE" config set data-folder games)
  run "$WC2BNE" config get data-folder
  [ "$output" = "$T/games" ]
  run "$WC2BNE" status --porcelain
  [[ $output == "data=$T/games"* ]]
  WC2BNE_HOME=$T/other run "$WC2BNE" status --porcelain
  [[ $output == "data=$T/other"* ]]
}

@test "setting data-folder to default forgets the choice" {
  "$WC2BNE" config set data-folder "$T/games"
  "$WC2BNE" config set data-folder default
  run "$WC2BNE" config get data-folder
  [ "$output" = "$DATA" ]
}
