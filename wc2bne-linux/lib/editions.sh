# Editions: where they live and what the installer should include.

EXE='Warcraft II BNE.exe'

# valid_edition NAME  Succeeds for a known edition. Pure.
valid_edition() { case $1 in community|official) return 0;; *) return 1;; esac; }

# game_dir DATA EDITION  -> path of the game folder. Pure.
game_dir() { printf '%s\n' "$1/$2"; }

# wine_dir DATA EDITION  -> path of the Wine environment. Pure.
wine_dir() { printf '%s\n' "$1/$2/.wine"; }

# maps_dir DATA  -> path of the shared maps. Pure.
maps_dir() { printf '%s\n' "$1/Maps"; }

# edition_components EDITION  -> the Inno Setup /COMPONENTS value. Pure.
edition_components() {
  local c='program,aieorcamp,ai\campaign,mapeditor,addgate,addgate\rewrite,incomingport,incomingport\defaultport,ddrawcfg'
  case $1 in
    community) c+=',program\lc,program\lc\w2plobbmap,program\lc\w2pcpusavec,program\lc\w2pautowlat,program\lc\w2pplysound'
               c+=',holepuncher,iron_96,intf,intf\warlatplus,intf\quickjoin,intf\up1lvl,intf\lobbyfix';;
    official)  c+=',program\gog';;
  esac
  printf '%s\n' "$c"
}

# is_installed DATA EDITION  Succeeds when the game is there. Reads files.
is_installed() { [ -f "$(game_dir "$1" "$2")/$EXE" ]; }

# copy_missing FROM TO  Copies files that TO doesn't have yet, never overwriting. Writes files.
copy_missing() {
  local rel
  while IFS= read -r -d '' rel; do
    if [ ! -e "$2/$rel" ]; then mkdir -p "$(dirname "$2/$rel")"; cp "$1/$rel" "$2/$rel"; fi
  done < <(cd "$1" && find . -type f -print0)
}

# share_maps DATA EDITION  Moves the edition's own maps into the shared maps and leaves a link. Writes files.
share_maps() {
  local own maps; own=$(game_dir "$1" "$2")/Maps maps=$(maps_dir "$1")
  mkdir -p "$maps"
  if [ -d "$own" ] && [ ! -L "$own" ]; then copy_missing "$own" "$maps"; rm -r "$own"; fi
  ln -sfn ../Maps "$own"
}

# The files Battle.net's version check covers.
CHECKED_FILES=("$EXE" storm.dll battle.snp)

# checksums_file DATA EDITION  -> path, outside the sandbox. Pure.
checksums_file() { printf '%s\n' "$1/.checksums/$2"; }

# record_files DATA EDITION  Remembers the checked files' checksums. Writes a file.
record_files() {
  local file; file=$(checksums_file "$1" "$2")
  mkdir -p "$(dirname "$file")"
  (cd "$(game_dir "$1" "$2")" && sha256sum -- "${CHECKED_FILES[@]}") >"$file"
}

# changed_files DATA EDITION  -> names of checked files that are missing or changed, one per line. Reads files.
changed_files() {
  local dir file f; dir=$(game_dir "$1" "$2") file=$(checksums_file "$1" "$2")
  for f in "${CHECKED_FILES[@]}"; do
    if ! [ -f "$dir/$f" ] || ! grep -qxF "$(cd "$dir" && sha256sum -- "$f")" "$file" 2>/dev/null; then
      printf '%s\n' "$f"
    fi
  done
}

# remove_edition DATA EDITION  Deletes the game folder, following no links, and its checksums. Deletes files.
remove_edition() { rm -rf -- "$(game_dir "$1" "$2")" "$(checksums_file "$1" "$2")"; }
