# The data folder and its settings.

# data_folder CHOSEN REMEMBERED LAUNCHER_DIR USER_DATA_DIR  -> absolute path: CHOSEN if not empty, else REMEMBERED, else
# data/ beside a launcher run from source, else (a launcher in a read-only place) wc2bne in USER_DATA_DIR. Reads
# permissions.
data_folder() {
  if [ -n "$1" ]; then absolute "$1"
  elif [ -n "$2" ]; then printf '%s\n' "$2"
  elif [ -d "$3/data" ] || [ -w "$3" ]; then printf '%s\n' "$3/data"
  else printf '%s\n' "$4/wc2bne"; fi
}

# absolute PATH  -> PATH, from the current directory if relative. Reads the current directory.
absolute() { case $1 in /*) printf '%s\n' "$1";; *) printf '%s\n' "$PWD/$1";; esac; }

# recall_folder CONFIG_DIR  -> the remembered data folder, or nothing. Reads a file.
recall_folder() { head -n 1 "$1/wc2bne/data-folder" 2>/dev/null || true; }

# remember_folder CONFIG_DIR PATH  Remembers the data folder; an empty PATH forgets it. Writes a file.
remember_folder() {
  if [ -z "$2" ]; then rm -f "$1/wc2bne/data-folder"; return; fi
  mkdir -p "$1/wc2bne"; printf '%s\n' "$2" >"$1/wc2bne/data-folder"
}

# setting_default NAME  -> default value; fails for an unknown setting. Pure.
setting_default() {
  case $1 in
    edition) echo community;;
    mode) echo fullscreen;;
    port) echo 6112;;
    *) return 1;;
  esac
}

# valid_setting NAME VALUE  Pure.
valid_setting() {
  case $1 in
    edition) valid_edition "$2";;
    mode) [[ $2 == fullscreen || $2 == window ]];;
    port) [[ $2 =~ ^[0-9]{4,5}$ ]] && [ "$2" -ge 1024 ] && [ "$2" -le 65435 ];;  # the game tries the next 100 too
    *) return 1;;
  esac
}

# conf_get NAME  -> the last value for NAME in wc2bne.conf text on stdin; nothing when absent. Pure.
conf_get() { sed -n "s/^[[:space:]]*$1[[:space:]]*=[[:space:]]*//p" | tail -n 1 | sed 's/[[:space:]]*$//'; }

# conf_set NAME VALUE  Filters wc2bne.conf text: sets NAME, keeping everything else. Pure.
conf_set() { sed "/^[[:space:]]*$1[[:space:]]*=/d"; printf '%s=%s\n' "$1" "$2"; }

# read_conf DATA  -> text of wc2bne.conf; nothing when absent. Reads a file.
read_conf() { cat "$1/wc2bne.conf" 2>/dev/null || true; }

# save_setting DATA NAME VALUE  Writes wc2bne.conf.
save_setting() {
  local text; text=$(read_conf "$1" | conf_set "$2" "$3")
  mkdir -p "$1"; printf '%s\n' "$text" >"$1/wc2bne.conf"
}

# setting NAME OPTION ENVIRONMENT CONF_TEXT  -> the first that isn't empty, else the default. Exits 2 if invalid. Pure.
setting() {
  local value=$2 from="--$1"
  if [ -z "$value" ]; then value=$3 from="WC2BNE_${1^^}"; fi
  if [ -z "$value" ]; then value=$(conf_get "$1" <<<"$4") from=wc2bne.conf; fi
  if [ -z "$value" ]; then value=$(setting_default "$1"); fi
  valid_setting "$1" "$value" || usage_error "invalid $1 from $from: $value"
  printf '%s\n' "$value"
}
