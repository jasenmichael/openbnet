#!/usr/bin/env bats
# Missing programs.
load helper

setup_more() { mkdir -p "$DATA"; printf good >"$DATA/War2Combat46_en.exe"; }

@test "a missing program exits 4, names it, and prints the command that installs it" {
  use_manager apt-get
  hide bwrap
  run "$WC2BNE" install </dev/null
  [ "$status" -eq 4 ]
  [[ $output == *bwrap*"sudo apt-get install bubblewrap"* ]]
  [ ! -e "$STUB_LOG/sudo" ]
}

@test "each package manager has its own command" {
  hide bwrap
  for m in "dnf:sudo dnf install bubblewrap" "pacman:sudo pacman -S --needed bubblewrap" "zypper:sudo zypper install bubblewrap"; do
    use_manager "${m%%:*}"
    run "$WC2BNE" install </dev/null
    [[ $output == *"${m#*:}"* ]] || { echo "$m: $output"; false; }
    rm "$T/bin/${m%%:*}"
  done
}

@test "with no known package manager it only names what is missing" {
  hide bwrap
  run "$WC2BNE" install </dev/null
  [ "$status" -eq 4 ]
  [[ $output == *bwrap* ]]
}

@test "on a terminal, answering y installs and carries on" {
  use_manager apt-get
  hide bwrap
  answer y "$WC2BNE" install
  grep -qx 'apt-get install bubblewrap' "$STUB_LOG/sudo"
  ran good
}

@test "answering anything else installs nothing" {
  use_manager apt-get
  hide bwrap
  for a in n "" yikes; do
    run answer "$a" "$WC2BNE" install
    [ "$status" -eq 4 ]
    [ ! -e "$STUB_LOG/sudo" ]
  done
}

@test "missing 32-bit Wine support is noticed, and on apt adds the architecture first" {
  use_manager apt-get
  no_wine32
  run "$WC2BNE" install </dev/null
  [ "$status" -eq 4 ]
  [[ $output == *"sudo dpkg --add-architecture i386"*"sudo apt-get update"*"sudo apt-get install wine32:i386"* ]]
}

@test "curl is only needed for downloading" {
  hide curl
  "$WC2BNE" install
  rm "$DATA/War2Combat46_en.exe"
  run "$WC2BNE" install --edition official </dev/null
  [ "$status" -eq 4 ]
  [[ $output == *curl* ]]
}

@test "Xwayland is only needed to play on Wayland" {
  "$WC2BNE" install
  hide Xwayland
  "$WC2BNE" play
  WAYLAND_DISPLAY=w run "$WC2BNE" play </dev/null
  [ "$status" -eq 4 ]
  [[ $output == *Xwayland* ]]
}

@test "help and settings work with nothing installed" {
  hide bwrap wine curl Xwayland
  "$WC2BNE" help
  "$WC2BNE" config set mode window
  "$WC2BNE" config get mode
}

@test "doctor reports every program, and exits 0 when nothing needed is missing" {
  run "$WC2BNE" doctor
  [ "$status" -eq 0 ]
  for t in bwrap wine wine32 curl xrandr; do [[ $output == *$t* ]]; done
}

@test "doctor exits 4 when something needed is missing, but not for optional programs" {
  hide xrandr
  run "$WC2BNE" doctor </dev/null
  [ "$status" -eq 0 ]
  [[ $output == *xrandr*optional* ]]
  hide curl
  run "$WC2BNE" doctor </dev/null
  [ "$status" -eq 4 ]
}

@test "doctor can install what is missing" {
  use_manager apt-get
  hide bwrap curl
  answer y "$WC2BNE" doctor
  grep -qx 'apt-get install bubblewrap curl' "$STUB_LOG/sudo"
}

@test "doctor --porcelain lists programs and the commands that install what is needed, and never asks" {
  use_manager apt-get
  hide bwrap xrandr
  run answer y "$WC2BNE" doctor --porcelain
  [ "$status" -eq 4 ]
  [[ "${output//$'\r'/}" == *$'bwrap=missing\nwine=ok\nwine32=ok\ncurl=ok\nxrandr=optional\nxdg-open=ok\nXwayland=ok\ninstall=apt-get update\ninstall=apt-get install -y bubblewrap' ]]
  [ ! -e "$STUB_LOG/sudo" ]
}
