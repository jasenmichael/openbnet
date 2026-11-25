#!/usr/bin/env bats
# Game files the server checks must be unchanged.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "play refuses to start when a checked file has changed, and names it" {
  "$WC2BNE" install
  for f in "Warcraft II BNE.exe" storm.dll battle.snp; do
    cp "$DATA/community/$f" "$T/saved"
    echo changed >>"$DATA/community/$f"
    run "$WC2BNE" play
    [ "$status" -eq 5 ] || { echo "$f: $status"; false; }
    [[ $output == *"$f"* ]]
    cp "$T/saved" "$DATA/community/$f"
  done
  "$WC2BNE" play
}

@test "other files can be edited freely" {
  "$WC2BNE" install
  echo changed >>"$DATA/community/Maps/Stock.pud"
  echo new >"$DATA/community/notes.txt"
  "$WC2BNE" play
}

@test "the record of checked files is out of the game's reach" {
  "$WC2BNE" install
  "$WC2BNE" play
  run grep -F checksums "$STUB_LOG/bwrap"
  [ "$status" -ne 0 ]
  [ -s "$DATA/.checksums/community" ]
}
