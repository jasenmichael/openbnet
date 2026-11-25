#!/usr/bin/env bats
# Removing an edition.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "uninstall --yes removes the edition, and keeps maps, settings and the other edition" {
  "$WC2BNE" install
  "$WC2BNE" install --edition official
  "$WC2BNE" config set mode window
  run "$WC2BNE" uninstall --yes
  [ "$status" -eq 0 ]
  [ ! -e "$DATA/community" ]
  [ "$(cat "$DATA/Maps/Stock.pud")" = stock ]
  [ -e "$DATA/wc2bne.conf" ]
  "$WC2BNE" play --edition official
  run "$WC2BNE" status --porcelain
  [[ $output == *community=absent*official=installed* ]]
}

@test "uninstall asks first, and does nothing without a yes" {
  "$WC2BNE" install
  run "$WC2BNE" uninstall </dev/null
  [ "$status" -eq 1 ]
  run answer n "$WC2BNE" uninstall
  [ "$status" -eq 1 ]
  [ -e "$DATA/community/storm.dll" ]
  answer y "$WC2BNE" uninstall
  [ ! -e "$DATA/community" ]
}

@test "uninstall is refused while the game runs" {
  "$WC2BNE" install
  start_game
  run "$WC2BNE" uninstall --yes
  stop_game
  [ "$status" -eq 6 ]
  [ -e "$DATA/community/storm.dll" ]
}
