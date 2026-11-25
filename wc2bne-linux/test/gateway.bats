#!/usr/bin/env bats
# Managing the list of servers the game offers.
load helper
setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "gateway list shows the game's list" {
  "$WC2BNE" install
  run "$WC2BNE" gateway list
  [ "$status" -eq 0 ]
  [[ $output == *one.example*"Two Server"* ]]
}

@test "an added gateway is listed and survives" {
  "$WC2BNE" install
  "$WC2BNE" gateway add my.server.example "My server" 5
  run "$WC2BNE" gateway list
  [[ $output == *my.server.example*"My server"* ]]
}

@test "gateway add refuses an invalid host" {
  "$WC2BNE" install
  run "$WC2BNE" gateway add 'bad host;' Name
  [ "$status" -ne 0 ]
}

@test "gateway reset goes back to the game's list" {
  "$WC2BNE" install
  "$WC2BNE" gateway add my.server.example "My server"
  "$WC2BNE" gateway reset
  run "$WC2BNE" gateway list
  [[ $output != *my.server.example* ]]
  [[ $output == *one.example* ]]
}

gw() { "$WC2BNE" gateway "$@"; }
list() { "$WC2BNE" gateway list --porcelain; }

@test "gateway list --porcelain prints number, selected, host, offset and name, separated by tabs" {
  "$WC2BNE" install
  [ "$(list)" = $'1\t1\tone.example\t-3\tOne\n2\t0\ttwo.example\t0\tTwo Server' ]
}

@test "adding a gateway keeps the others and selects the new one" {
  "$WC2BNE" install
  gw add my.server.example "My server" -5
  [ "$(list)" = $'1\t0\tone.example\t-3\tOne\n2\t0\ttwo.example\t0\tTwo Server\n3\t1\tmy.server.example\t-5\tMy server' ]
}

@test "select chooses a gateway by number" {
  "$WC2BNE" install
  gw select 2
  [ "$(list)" = $'1\t0\tone.example\t-3\tOne\n2\t1\ttwo.example\t0\tTwo Server' ]
}

@test "removing a gateway keeps the same one selected" {
  "$WC2BNE" install
  gw select 2
  gw remove 1
  [ "$(list)" = $'1\t1\ttwo.example\t0\tTwo Server' ]
}

@test "removing the selected gateway selects the first" {
  "$WC2BNE" install
  gw select 2
  gw remove 2
  [ "$(list)" = $'1\t1\tone.example\t-3\tOne' ]
}

@test "the last gateway can't be removed" {
  "$WC2BNE" install
  gw remove 2
  run gw remove 1
  [ "$status" -eq 1 ]
  [ "$(list)" = $'1\t1\tone.example\t-3\tOne' ]
}

@test "a number that isn't in the list exits 2" {
  "$WC2BNE" install
  for a in "select 3" "select 0" "select x" "select" "remove 9" "remove 08" "select 1 2"; do
    run gw $a
    [ "$status" -eq 2 ] || { echo "'$a' exited $status"; false; }
  done
}

@test "reset restores the game's list" {
  "$WC2BNE" install
  gw select 2
  gw remove 1
  gw reset
  [ "$(list)" = $'1\t1\tone.example\t-3\tOne\n2\t0\ttwo.example\t0\tTwo Server' ]
}

@test "the official edition starts with Blizzard's server selected" {
  "$WC2BNE" install --edition official
  run "$WC2BNE" gateway list --edition official --porcelain
  [ "$output" = $'1\t1\tconnect-forever.classic.blizzard.com\t8\tGlobal (GoG)' ]
}

@test "names that could corrupt the list are refused" {
  "$WC2BNE" install
  for n in 'a|b' 'a\0b' "" $'a\tb'; do
    run gw add ok.example "$n"
    [ "$status" -eq 2 ] || { echo "'$n' exited $status"; false; }
  done
}

@test "listing gateways and reading the port don't start Wine" {
  "$WC2BNE" install
  "$WC2BNE" config set port 6113
  rm "$STUB_LOG/bwrap"
  [ "$(list)" = $'1\t1\tone.example\t-3\tOne\n2\t0\ttwo.example\t0\tTwo Server' ]
  [ "$("$WC2BNE" config get port)" = 6113 ]
  [ ! -e "$STUB_LOG/bwrap" ]
}

@test "lists saved by the game are read exactly, including odd characters" {
  "$WC2BNE" install
  cat >"$DATA/community/.wine/user.reg" <<'E'
WINE REGISTRY Version 2
[Software\\Battle.net\\Configuration] 1764000000
"Battle.net gateways"=str(7):"2000\00002\0server.example\0005\0Caf\xe9 \"One\"\000127.0.0.1\0000\0Dash \5Two\0"
E
  [ "$(LC_ALL=C.UTF-8 list)" = $'1\t0\tserver.example\t5\tCafé "One"\n2\t1\t127.0.0.1\t0\tDash \x05Two' ]
}

@test "gateway set saves a whole list at once, starting Wine once" {
  "$WC2BNE" install
  rm "$STUB_LOG/bwrap"
  printf '1\t0\ta.example\t5\tA\n2\t1\tb.example\t-8\tB \x05two\n' | gw set
  [ "$(list)" = $'1\t0\ta.example\t5\tA\n2\t1\tb.example\t-8\tB \x05two' ]
  [ "$(grep -cx -- -- "$STUB_LOG/bwrap")" -eq 1 ]
}

@test "gateway set refuses an invalid list, and keeps the old one" {
  "$WC2BNE" install
  before=$(list)
  for l in "" $'1\t0\ta.example\t0\tA\n' $'1\t1\ta.example\t0\tA\n2\t1\tb.example\t0\tB\n' $'1\t1\tbad host\t0\tA\n' $'1\t1\ta.example\n' $'1\t1\ta.example\t0\tA\textra\n' $'1\t1\ta.example\t0\ta|b\n'; do
    run gw set <<<"${l%$'\n'}"
    [ "$status" -eq 2 ] || { echo "exited $status for: $l"; false; }
  done
  [ "$(list)" = "$before" ]
}
