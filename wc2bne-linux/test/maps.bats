#!/usr/bin/env bats
# Maps are shared by all editions and belong to the user.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "an edition's Maps folder leads to the shared maps, by a relative link" {
  "$WC2BNE" install
  [ "$(cat "$DATA/Maps/Stock.pud")" = stock ]
  [ "$(cat "$DATA/Maps/Ladder/L.pud")" = ladder ]
  [ "$(readlink "$DATA/community/Maps")" = ../Maps ]
}

@test "a map added once is seen by both editions" {
  "$WC2BNE" install
  "$WC2BNE" install --edition official
  echo mine >"$DATA/community/Maps/Mine.pud"
  [ "$(cat "$DATA/official/Maps/Mine.pud")" = mine ]
}

@test "installing never overwrites the user's maps" {
  mkdir -p "$DATA/Maps"
  echo edited >"$DATA/Maps/Stock.pud"
  "$WC2BNE" install
  [ "$(cat "$DATA/Maps/Stock.pud")" = edited ]
  [ "$(cat "$DATA/Maps/Ladder/L.pud")" = ladder ]
}

@test "deleting an edition's folder keeps the maps" {
  "$WC2BNE" install
  rm -r "$DATA/community"
  "$WC2BNE" install
  [ "$(cat "$DATA/Maps/Stock.pud")" = stock ]
}

@test "the game finds its maps inside the sandbox" {
  "$WC2BNE" install
  run "$WC2BNE" exec -- cat war2/Maps/Stock.pud
  [ "$output" = stock ]
}
