#!/usr/bin/env bats
# How `wc2bne install` finds, downloads and checks the installer.
bats_require_minimum_version 1.5.0
load helper
setup_more() { KEPT=$DATA/War2Combat46_en.exe; mkdir -p "$DATA"; }

@test "a WC2BNE_SETUP file with the wrong checksum is refused" {
  unset WC2BNE_SHA256
  printf other >"$T/mine.exe"
  WC2BNE_SETUP=$T/mine.exe run "$WC2BNE" install
  [ "$status" -ne 0 ]
  [[ $output == *checksum*"$PINNED"* ]]
  nothing_ran
}

@test "a WC2BNE_SETUP file with the right checksum is installed, ahead of the data folder" {
  printf good >"$T/mine.exe"
  printf vendored >"$KEPT"
  WC2BNE_SETUP=$T/mine.exe run "$WC2BNE" install
  [ "$status" -eq 0 ]
  ran good
}

@test "without WC2BNE_SETUP, the installer in the data folder is used" {
  printf good >"$KEPT"
  run "$WC2BNE" install
  [ "$status" -eq 0 ]
  ran good
}

@test "with nothing in the data folder, it downloads from the primary URL" {
  rmdir "$DATA"
  serve "$PRIMARY" good
  run "$WC2BNE" install
  [ "$status" -eq 0 ]
  ran good
}

@test "a downloaded installer is kept in the data folder, so the next edition needs no download" {
  serve "$PRIMARY" good
  "$WC2BNE" install --edition community
  rm -r "$STUB_WEB" "$STUB_LOG/ran"
  run "$WC2BNE" install --edition official
  [ "$status" -eq 0 ]
  ran good
}

@test "when the primary URL fails, it downloads from the fallback" {
  serve "$FALLBACK" good
  run "$WC2BNE" install
  [ "$status" -eq 0 ]
  ran good
}

@test "when the primary URL serves the wrong file, it downloads from the fallback" {
  serve "$PRIMARY" tampered
  serve "$FALLBACK" good
  run "$WC2BNE" install
  [ "$status" -eq 0 ]
  ran good
}

@test "when no download has the right checksum, nothing runs and nothing is kept" {
  serve "$PRIMARY" tampered
  serve "$FALLBACK" tampered
  run "$WC2BNE" install
  [ "$status" -ne 0 ]
  [[ $output == *"could not download"* ]]
  nothing_ran
  [ -z "$(ls -A "$DATA")" ]
}

@test "a missing WC2BNE_SETUP file is an error, even if a download would work" {
  serve "$PRIMARY" good
  WC2BNE_SETUP=$T/missing.exe run "$WC2BNE" install
  [ "$status" -ne 0 ]
  [[ $output == *"not found"*missing.exe* ]]
  nothing_ran
}

@test "a kept installer with the wrong checksum is replaced by a download and set aside" {
  printf old >"$KEPT"
  serve "$PRIMARY" good
  run "$WC2BNE" install
  [ "$status" -eq 0 ]
  ran good
  [ "$(cat "$KEPT.bad")" = old ]
}

@test "a kept installer with the wrong checksum is left alone when no download works" {
  printf old >"$KEPT"
  run "$WC2BNE" install
  [ "$status" -ne 0 ]
  nothing_ran
  [ "$(cat "$KEPT")" = old ]
}

@test "WC2BNE_OFFLINE prevents downloading" {
  serve "$PRIMARY" good
  WC2BNE_OFFLINE=1 run "$WC2BNE" install
  [ "$status" -ne 0 ]
  [[ $output == *WC2BNE_OFFLINE* ]]
  nothing_ran
}

@test "an already installed edition does not download again" {
  serve "$PRIMARY" good
  "$WC2BNE" install
  rm "$KEPT"
  run "$WC2BNE" install
  [ "$status" -ne 0 ]
  [[ $output == *"already installed"* ]]
  [ ! -e "$KEPT" ]
}

@test "without a display it fails before downloading" {
  serve "$PRIMARY" good
  DISPLAY= run "$WC2BNE" install
  [ "$status" -ne 0 ]
  [[ $output == *display* ]]
  [ ! -e "$KEPT" ]
}

@test "install --porcelain reports its steps and the download's progress" {
  serve "$PRIMARY" good
  run --separate-stderr "$WC2BNE" install --porcelain
  [ "$status" -eq 0 ]
  [ "$output" = $'step=download\npercent=50\npercent=100\nstep=prepare\nstep=install\nstep=finish\nstep=done' ]
}
