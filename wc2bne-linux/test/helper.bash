# Shared test setup. Everything outside the launcher is a stand-in from test/stubs, and PATH holds only
# those and a fixed list of basic tools, so a test can remove one to make it "missing".

PRIMARY=ftp.war2.ru/war2/War2Combat46_en.exe
FALLBACK=war2combat46en.netlify.app/War2Combat46_en.exe
PINNED=2b53fabb11dafc811c2167719cf2ac29ae41bd38363289748631305847436ecd
GOOD=770e607624d689265ca6c44884d0807d9b054d23c473c106c72be9de08b7376c  # sha256 of "good"

TOOLS="awk basename bash cat chmod cp cut dirname du env find flock grep head id kill ln ls mkdir mktemp mv
  readlink rm rmdir script sed sh sha256sum sleep sort tail touch tr wc xargs"

setup() {
  T=$BATS_TEST_TMPDIR
  mkdir -p "$T/repo/wc2bne-linux" "$T/home" "$T/web" "$T/log" "$T/bin" "$T/run" "$T/lib/wine/i386-windows"
  cp -r "$BATS_TEST_DIRNAME/../wc2bne" "$T/repo/wc2bne-linux/"
  [ ! -d "$BATS_TEST_DIRNAME/../lib" ] || cp -r "$BATS_TEST_DIRNAME/../lib" "$T/repo/wc2bne-linux/"
  WC2BNE=$T/repo/wc2bne-linux/wc2bne
  local t; for t in $TOOLS; do ln -s "$(command -v $t)" "$T/bin/$t"; done
  for t in "$BATS_TEST_DIRNAME"/stubs/*; do cp "$t" "$T/bin/"; done
  export HOME=$T/home DISPLAY=:99 XDG_RUNTIME_DIR=$T/run STUB_LOG=$T/log STUB_ROOT=$T/root STUB_WEB=$T/web
  export WC2BNE_SHA256=$GOOD
  local v; for v in ${!WC2BNE_@}; do [ $v = WC2BNE_SHA256 ] || [ $v = WC2BNE ] || unset $v; done
  unset XAUTHORITY XDG_DATA_HOME XDG_CONFIG_HOME WAYLAND_DISPLAY
  DATA=$T/repo/wc2bne-linux/data
  type setup_more >/dev/null 2>&1 && setup_more
  OLDPATH=$PATH; export PATH=$T/bin
}
teardown() { PATH=$OLDPATH; }

hide() { mkdir -p "$T/hidden"; local t; for t; do mv "$T/bin/$t" "$T/hidden/"; done; }   # make tools "missing"
no_wine32() { rm -r "$T/lib/wine/i386-windows"; }
# use_manager NAME  Adds a stand-in package manager and sudo, which "installs" everything hidden.
use_manager() {
  printf '#!/bin/sh\nexit 0\n' >"$T/bin/$1"
  cat >"$T/bin/sudo" <<E
#!$T/bin/bash
echo "\$*" >>"$T/log/sudo"
case "\$*" in *install*|*-S*) mv "$T"/hidden/* "$T/bin/"; mkdir -p "$T/lib/wine/i386-windows";; esac
E
  chmod +x "$T/bin/$1" "$T/bin/sudo"
}
# answer TEXT COMMAND...  Runs COMMAND on a terminal, typing TEXT.
answer() { local text=$1; shift; printf '%s\n' "$text" | SHELL=$T/bin/bash script -qec "$(printf '%q ' "$@")" /dev/null; }

serve() { mkdir -p "$T/web/$(dirname "$1")"; printf %s "$2" >"$T/web/$1"; }  # serve URL CONTENT
ran() { [ "$(cat "$STUB_LOG/ran")" = "$1" ]; }                               # content of the installer that ran
nothing_ran() { [ ! -e "$STUB_LOG/ran" ]; }
# start_game ARGS...  Plays in the background until stop_game; sets GAME to its process.
start_game() {
  touch "$STUB_LOG/keep-running"
  "$WC2BNE" play "$@" >/dev/null 2>&1 3>&- & GAME=$!
  local i; for i in {1..100}; do [ ! -e "$STUB_LOG/played" ] || return 0; sleep 0.05; done
  return 1
}
stop_game() { rm "$STUB_LOG/keep-running"; wait "$GAME"; }

played() { grep -qxF -- "$1" "$STUB_LOG/played"; }                           # a line of what the game saw
