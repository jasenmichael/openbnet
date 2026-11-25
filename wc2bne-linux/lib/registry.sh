# Battle.net settings in an edition's Windows registry. Reading uses the saved
# registry file; changing it starts Wine, offline.

REG_KEY='HKCU\Software\Battle.net\Configuration'
REG_PORT='Game Data Port'

REG_SECTION='[Software\\Battle.net\\Configuration]'   # as written in user.reg

# reg_raw NAME  -> the value as written in user.reg text on stdin, from REG_SECTION; nothing when absent. Pure.
reg_raw() {
  SECTION=$REG_SECTION NAME="\"$1\"=" awk '
    /^\[/ { inside = index($0, ENVIRON["SECTION"]) == 1; next }
    inside && index($0, ENVIRON["NAME"]) == 1 { print substr($0, length(ENVIRON["NAME"]) + 1); exit }'
}

# reg_unescape TEXT  -> TEXT without Wine's escapes; each NUL becomes a newline. Pure.
reg_unescape() {
  local rest=$1 out='' c LC_ALL=C.UTF-8
  while [[ $rest =~ ^([^\\]*)\\(.*)$ ]]; do
    out+=${BASH_REMATCH[1]} rest=${BASH_REMATCH[2]}
    if [[ $rest =~ ^x([0-9a-f]{1,4})(.*)$ ]]; then
      printf -v c "\\u${BASH_REMATCH[1]}"; out+=$c rest=${BASH_REMATCH[2]}
    elif [[ $rest =~ ^([0-7]{1,3})(.*)$ ]]; then
      rest=${BASH_REMATCH[2]}
      if (( 8#${BASH_REMATCH[1]} == 0 )); then out+=$'\n'; else printf -v c "\\x$(printf %x "$((8#${BASH_REMATCH[1]}))")"; out+=$c; fi
    else
      out+=${rest:0:1} rest=${rest:1}
    fi
  done
  printf '%s' "$out$rest"
}

# read_registry DATA EDITION  -> text of the edition's user.reg; nothing when absent. Reads a file. Wine saves it on
# exit, so it may be a little old while the game runs.
read_registry() { cat "$(wine_dir "$1" "$2")/user.reg" 2>/dev/null || true; }

# reg_get_list DATA EDITION NAME  -> one line per string of a REG_MULTI_SZ; nothing when unset. Reads a file.
reg_get_list() {
  local raw; raw=$(read_registry "$1" "$2" | reg_raw "$3")
  if [[ $raw =~ ^str\(7\):\"(.*)\"$ ]]; then reg_unescape "${BASH_REMATCH[1]}"; fi
}

# reg_set_list DATA EDITION NAME STRING...  Strings must not contain "|".
reg_set_list() {
  local data=$1 edition=$2 name=$3 IFS='|'; shift 3
  box "$data" "$edition" offline wine reg add "$REG_KEY" /v "$name" /t REG_MULTI_SZ /s '|' /d "$*" /f >/dev/null
}

# reg_set_number DATA EDITION NAME NUMBER
reg_set_number() { box "$1" "$2" offline wine reg add "$REG_KEY" /v "$3" /t REG_DWORD /d "$4" /f >/dev/null; }

# reg_get_number DATA EDITION NAME  -> a REG_DWORD in decimal; nothing when unset. Reads a file.
reg_get_number() {
  local raw; raw=$(read_registry "$1" "$2" | reg_raw "$3")
  if [[ $raw =~ ^dword:([0-9a-f]{8})$ ]]; then printf '%d\n' "0x${BASH_REMATCH[1]}"; fi
}

# reg_unset DATA EDITION NAME  Succeeds even when it wasn't set.
reg_unset() { box "$1" "$2" offline wine reg delete "$REG_KEY" /v "$3" /f >/dev/null 2>&1 || true; }
