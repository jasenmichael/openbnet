#!/usr/bin/env bats
# Command line conventions: help, usage errors, exit codes.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "--help and help list the commands" {
  for a in --help -h help; do
    run "$WC2BNE" $a
    [ "$status" -eq 0 ]
    [[ $output == *install*play*gateway*exec* ]]
  done
}

@test "help COMMAND and COMMAND --help describe one command" {
  run "$WC2BNE" help gateway
  [ "$status" -eq 0 ]
  [[ $output == *"wc2bne gateway"*add* ]]
  first=$output
  run "$WC2BNE" gateway --help
  [ "$output" = "$first" ]
}

@test "--version prints a version" {
  run "$WC2BNE" --version
  [ "$status" -eq 0 ]
  [[ $output =~ ^wc2bne\ [0-9]+\.[0-9]+\.[0-9]+$ ]]
}

@test "usage errors exit 2" {
  for a in "" nonsense "play --nonsense" "play extra" "install --window" "--edition" "install --edition other" "exec" "help nonsense"; do
    run "$WC2BNE" $a
    [ "$status" -eq 2 ] || { echo "'$a' exited $status"; false; }
  done
}

@test "commands that need the game exit 3 when the edition isn't installed" {
  for a in play "gateway list" "exec -- true"; do
    run "$WC2BNE" --edition official $a
    [ "$status" -eq 3 ] || { echo "'$a' exited $status: $output"; false; }
    [[ $output == *"wc2bne install --edition official"* ]]
  done
}

@test "--edition chooses the edition, before or after the command" {
  "$WC2BNE" install --edition official
  grep -q 'program.gog' "$STUB_LOG/setup-args"
  "$WC2BNE" --edition=community install
  grep -q 'program.lc' "$STUB_LOG/setup-args"
}

@test "exec passes everything after -- through" {
  "$WC2BNE" install
  run "$WC2BNE" exec -- echo --edition --window
  [ "$output" = "--edition --window" ]
}
