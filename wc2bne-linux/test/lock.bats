#!/usr/bin/env bats
# Only one thing at a time may use an edition.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "a second play is refused while the game runs, with exit code 6" {
  "$WC2BNE" install
  start_game
  run "$WC2BNE" play
  stop_game
  [ "$status" -eq 6 ]
  [[ $output == *"in use"* ]]
}

@test "nothing can change an edition while its game runs" {
  "$WC2BNE" install
  start_game
  for a in "gateway add x.example X" "gateway select 2" "gateway remove 1" "gateway reset" "config set port 6113" "exec -- true"; do
    run "$WC2BNE" $a
    [ "$status" -eq 6 ] || { stop_game; echo "'$a' exited $status"; false; }
  done
  stop_game
}

@test "the lock doesn't reach the game" {
  "$WC2BNE" install
  "$WC2BNE" play
  played locks=0
}

@test "the other edition can be used meanwhile" {
  "$WC2BNE" install
  "$WC2BNE" install --edition official
  start_game
  run "$WC2BNE" gateway select 1 --edition official
  stop_game
  [ "$status" -eq 0 ]
}

@test "a launcher that was killed doesn't leave the edition in use" {
  "$WC2BNE" install
  start_game
  kill -9 "$GAME"
  rm "$STUB_LOG/keep-running"   # the real sandbox dies with the launcher; the stand-in needs telling
  while [ -L "$DATA/community/.wine/drive_c/war2" ]; do sleep 0.05; done
  run "$WC2BNE" gateway select 2
  [ "$status" -eq 0 ]
}

@test "status says what is using an edition" {
  "$WC2BNE" install
  run "$WC2BNE" status --porcelain
  [[ $output == *$'\ncommunity_in_use=no\n'* ]]
  start_game
  run "$WC2BNE" status --porcelain
  stop_game
  [[ $output == *$'\ncommunity_in_use=play\n'*official_in_use=no* ]]
}
