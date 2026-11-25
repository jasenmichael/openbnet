# Finding, downloading and checking the installer.

INSTALLER_SHA256=2b53fabb11dafc811c2167719cf2ac29ae41bd38363289748631305847436ecd
INSTALLER_URLS=(https://ftp.war2.ru/war2/War2Combat46_en.exe
                https://war2combat46en.netlify.app/War2Combat46_en.exe)

# has_sha256 FILE SHA256  Succeeds when the file's checksum matches. Reads the file.
has_sha256() { [ "$(sha256sum <"$1" | cut -d' ' -f1)" = "$2" ]; }

# report FD LINE  Prints a progress line to file descriptor FD; nothing when FD is empty.
report() { if [ -n "$1" ]; then printf '%s\n' "$2" >&"$1"; fi; }

# percentages  Filters curl's progress bar into "percent=N" lines, without repeats. Pure.
percentages() { tr '\r' '\n' | sed -u -n 's/^[# ]* \([0-9]\{1,3\}\)\.[0-9]%.*/percent=\1/p' | awk '$0 != last { print; fflush(); last = $0 }'; }

# fetch URL FILE FD  Downloads over HTTPS, with progress on the terminal, or on FD if not empty. Network; writes FILE.
fetch() {
  local a=(-fL --progress-bar --connect-timeout 20 --proto '=https' -o "$2" "$1")
  if [ -n "$3" ]; then curl "${a[@]}" 2>&1 | percentages >&"$3"; else curl "${a[@]}"; fi
}

# download_installer FILE SHA256 FD URL...  Tries each URL until one has the right checksum, then
# puts it at FILE, setting aside whatever was there. Network; writes files.
download_installer() {
  local file=$1 sha=$2 fd=$3 part=$1.part url; shift 3
  mkdir -p "$(dirname "$file")"
  for url in "$@"; do
    warn "downloading $url"
    report "$fd" step=download
    if fetch "$url" "$part" "$fd" && has_sha256 "$part" "$sha"; then
      if [ -e "$file" ]; then
        warn "wrong checksum: $file; moving it to $file.bad"
        mv --backup=numbered "$file" "$file.bad"
      fi
      mv "$part" "$file"
      return 0
    fi
    rm -f "$part"
  done
  die "$E_CHECKSUM" "could not download an installer with sha256 $sha"
}

# needs_download CHOSEN KEPT SHA256 OFFLINE  Succeeds when find_installer would download. Reads files.
needs_download() { [ -z "$1" ] && [ -z "$4" ] && ! { [ -f "$2" ] && has_sha256 "$2" "$3"; }; }

# find_installer CHOSEN KEPT SHA256 OFFLINE FD  -> path of a verified installer: CHOSEN if not empty,
# else KEPT, else a download to KEPT. Reads files; may download.
find_installer() {
  local chosen=$1 kept=$2 sha=$3 offline=$4 fd=$5
  if [ -n "$chosen" ]; then
    [ -f "$chosen" ] || die "$E_FAILED" "installer not found: $chosen"
    has_sha256 "$chosen" "$sha" || die "$E_CHECKSUM" "wrong checksum: $chosen (expected sha256 $sha)"
    printf '%s\n' "$chosen"
    return
  fi
  if ! { [ -f "$kept" ] && has_sha256 "$kept" "$sha"; }; then
    [ -z "$offline" ] || die "$E_FAILED" "no usable installer in $(dirname "$kept"), and WC2BNE_OFFLINE prevents downloading it"
    download_installer "$kept" "$sha" "$fd" "${INSTALLER_URLS[@]}"
  fi
  printf '%s\n' "$kept"
}

# run_installer DATA EDITION INSTALLER FD  Installs the edition silently, offline. Sandbox; writes files.
run_installer() {
  local data=$1 edition=$2 installer=$3 fd=$4 game wine log
  game=$(game_dir "$data" "$edition") wine=$(wine_dir "$data" "$edition") log=$(wine_dir "$data" "$edition")/install.log
  mkdir -p "$wine" "$(maps_dir "$data")"; chmod 700 "$game"
  if [ -L "$game/Maps" ]; then rm "$game/Maps"; fi   # left by an install that stopped late; leads to the user's maps
  rm -f "$(checksums_file "$data" "$edition")"   # until the install finishes, the edition isn't installed
  report "$fd" step=prepare
  box "$data" "$edition" offline wineboot -i
  report "$fd" step=install
  box "$data" "$edition" offline --installer "$installer" wine 'Z:\home\box\setup.exe' /VERYSILENT /SUPPRESSMSGBOXES \
    /NORESTART /NOICONS /LANG=en '/DIR=C:\war2' "/COMPONENTS=$(edition_components "$edition")" \
    '/LOG=Z:\home\box\.wine\install.log' || die "$E_FAILED" "installer failed; see $log"
  [ -f "$game/$EXE" ] || die "$E_FAILED" "installer finished but the game is missing; see $log"
  report "$fd" step=finish
  share_maps "$data" "$edition"
  rm -f "$wine/dosdevices/z:"   # it maps the whole sandbox; the game doesn't need it
}
