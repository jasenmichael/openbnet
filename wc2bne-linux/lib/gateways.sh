# Gateway lists. battle.snp reads the override list before the game's list, and a server can only replace the
# game's. A list is: list version, 2-digit selected number, then host / GMT offset / name for each gateway.

GW_OVERRIDE='Override Battle.net gateways'
GW_GAME='Battle.net gateways'

# gw_valid HOST TZ NAME  Succeeds when the fields are safe to store. Pure.
gw_valid() { [[ $1 =~ ^[A-Za-z0-9.-]+$ && $2 =~ ^-?[0-9]+$ && -n $3 && $3 != *['|\']* && $3 != *[$'\t\n\r']* ]]; }

# gw_count LIST...  -> number of gateways. Pure.
gw_count() { printf '%s\n' $(( $# < 2 ? 0 : ($# - 2) / 3 )); }

# gw_has NUMBER LIST...  Succeeds when there is such a gateway. Pure.
gw_has() { local n=$1; shift; [[ $n =~ ^[1-9][0-9]?$ ]] && [ "$n" -le "$(gw_count "$@")" ]; }

# gw_add HOST TZ NAME [LIST...]  -> the list with a gateway appended and selected. Pure.
gw_add() {
  local host=$1 tz=$2 name=$3; shift 3
  [ $# -ge 2 ] || set -- 2000 01
  gw_select $(( $(gw_count "$@") + 1 )) "$@" "$host" "$tz" "$name"
}

# gw_select NUMBER LIST...  -> the list with that gateway selected. Pure.
gw_select() { local n=$1 version=$2; shift 3; printf '%s\n' "$version" "$(printf %02d "$n")" "$@"; }

# gw_remove NUMBER LIST...  -> the list without that gateway; the selected one stays selected, or the first
# becomes so. Pure.
gw_remove() {
  local n=$1 selected=$((10#$3)) list=("${@:2}")
  if [ "$selected" -eq "$n" ]; then selected=1; elif [ "$selected" -gt "$n" ]; then selected=$((selected - 1)); fi
  gw_select "$selected" "${list[@]:0:2+(n-1)*3}" "${list[@]:2+n*3}"
}

# gw_rows LIST...  -> "number TAB selected(1|0) TAB host TAB offset TAB name" for each gateway. Pure.
gw_rows() {
  local selected=$((10#${2:-0})) n=0; shift 2 || return 0
  while [ $# -ge 3 ]; do
    n=$((n + 1))
    printf '%s\t%s\t%s\t%s\t%s\n' $n $(( n == selected )) "$1" "$2" "$3"
    shift 3
  done
}

# gw_from_rows VERSION  -> the list described by gw_rows lines on stdin; numbers are ignored. Fails if a line is
# invalid, or not exactly one is selected. Pure.
gw_from_rows() {
  local version=$1 number selected host tz name extra n=0 chosen=0 fields=()
  while IFS=$'\t' read -r number selected host tz name extra; do
    gw_valid "$host" "$tz" "$name" && [ -z "$extra" ] || return 1
    n=$((n + 1)); fields+=("$host" "$tz" "$name")
    case $selected in
      0) ;;
      1) [ $chosen -eq 0 ] || return 1; chosen=$n;;
      *) return 1;;
    esac
  done
  [ $chosen -gt 0 ] && [ $n -le 99 ] || return 1
  gw_select $chosen "$version" 01 "${fields[@]}"
}

# gw_table  Filters gw_rows output into a table for people. Pure.
gw_table() { awk -F '\t' '{ printf "%s %2d  %-40s %s\n", ($2 ? "*" : " "), $1, $3, $5 }'; }

# gw_read DATA EDITION  -> the list in effect: the override list, else the game's. Reads a file.
gw_read() {
  local list; list=$(reg_get_list "$1" "$2" "$GW_OVERRIDE")
  if [ -z "$list" ]; then list=$(reg_get_list "$1" "$2" "$GW_GAME"); fi
  printf '%s\n' "$list"
}
