#!/usr/bin/env bats
# Where the data folder is and what it looks like.
load helper

put_installer() { mkdir -p "$1"; printf good >"$1/War2Combat46_en.exe"; }

@test "run from source, the data folder is data/ beside the launcher, even through a symlink" {
  put_installer "$DATA"
  ln -s "$WC2BNE" "$T/bin/wc2bne"
  cd "$T/home"
  wc2bne install
  [ -f "$DATA/community/Warcraft II BNE.exe" ]
}

@test "WC2BNE_HOME moves the data folder, and may be relative" {
  put_installer "$T/elsewhere"
  cd "$T"
  WC2BNE_HOME=elsewhere "$WC2BNE" install
  [ -f "$T/elsewhere/community/Warcraft II BNE.exe" ]
  cd "$T/home"
  WC2BNE_HOME=$T/elsewhere "$WC2BNE" play
}

@test "where the launcher's folder is read-only, the data folder is in the user's data directory" {
  put_installer "$HOME/.local/share/wc2bne"
  chmod a-w "$T/repo/wc2bne-linux"
  run "$WC2BNE" install
  chmod u+w "$T/repo/wc2bne-linux"
  [ "$status" -eq 0 ]
  [ -f "$HOME/.local/share/wc2bne/community/Warcraft II BNE.exe" ]
}

@test "the game folder shows the game's own files, with Wine's hidden in .wine" {
  put_installer "$DATA"
  "$WC2BNE" install
  run ls "$DATA/community"
  [[ $output == *"Warcraft II BNE.exe"* ]]
  [[ $output != *drive_c* && $output != *prefix* && $output != *setup.exe* && $output != *install.log* ]]
  [ -d "$DATA/community/.wine/drive_c" ]
}

@test "the data folder can be moved" {
  put_installer "$DATA"
  "$WC2BNE" install
  "$WC2BNE" gateway add my.server.example "My server"
  mv "$DATA" "$T/moved here"
  export WC2BNE_HOME="$T/moved here"
  "$WC2BNE" play
  run "$WC2BNE" gateway list
  [[ $output == *my.server.example* ]]
}

@test "each edition has its own registry" {
  put_installer "$DATA"
  "$WC2BNE" install
  "$WC2BNE" install --edition official
  "$WC2BNE" gateway add my.server.example "My server"
  run "$WC2BNE" gateway list --edition official
  [[ $output != *my.server.example* ]]
}

@test "the sandbox of one edition doesn't include the other" {
  put_installer "$DATA"
  "$WC2BNE" install
  "$WC2BNE" install --edition official
  rm "$STUB_LOG/bwrap"
  "$WC2BNE" play
  run grep -e /official -e "^$DATA\$" "$STUB_LOG/bwrap"
  [ "$status" -ne 0 ]
}

@test "the installer can't be changed from inside the sandbox" {
  put_installer "$DATA"
  "$WC2BNE" install
  grep -B1 -xF "$DATA/War2Combat46_en.exe" "$STUB_LOG/bwrap" | grep -xF -- --ro-bind
}

@test "Wine environments are created with Wine's default architecture" {
  put_installer "$DATA"
  "$WC2BNE" install
  run grep -x WINEARCH "$STUB_LOG/bwrap"
  [ "$status" -ne 0 ]
}
