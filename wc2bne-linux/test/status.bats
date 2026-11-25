#!/usr/bin/env bats
# Seeing what is installed, and opening it.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "status --porcelain describes the data folder and editions" {
  "$WC2BNE" install
  run "$WC2BNE" status --porcelain
  [ "$status" -eq 0 ]
  [ "$output" = "data=$DATA
installer=present
edition=community
mode=fullscreen
community=installed
official=absent
community_in_use=no
official_in_use=no" ]
}

@test "status says when game files have changed" {
  "$WC2BNE" install
  echo x >>"$DATA/community/storm.dll"
  run "$WC2BNE" status --porcelain
  [ "$status" -eq 0 ]
  [[ $output == *community=changed* ]]
}

@test "status works before anything is installed, and creates nothing" {
  rm -r "$DATA"
  hide bwrap wine
  run "$WC2BNE" status
  [ "$status" -eq 0 ]
  [[ $output == *"$DATA"* ]]
  [ ! -e "$DATA" ]
}

@test "open opens the edition's game folder" {
  "$WC2BNE" install
  "$WC2BNE" open
  [ "$(cat "$STUB_LOG/opened")" = "$DATA/community" ]
}

@test "open exits 3 when the edition isn't installed" {
  run "$WC2BNE" open
  [ "$status" -eq 3 ]
}

@test "open --maps opens the shared maps" {
  "$WC2BNE" install
  "$WC2BNE" open --maps
  [ "$(cat "$STUB_LOG/opened")" = "$DATA/Maps" ]
}
