# The command line: reads arguments and the environment, and calls everything else.
# Every cmd_ function takes DATA EDITION MODE PORCELAIN, then the command's own arguments.

WC2BNE_VERSION=0.1.0

# usage_error MESSAGE...  Exits 2.
usage_error() { die "$E_USAGE" "$* (see: wc2bne help)"; }

# need_installed DATA EDITION  Exits 3 unless the edition is installed.
need_installed() {
  is_installed "$1" "$2" || die "$E_NOT_INSTALLED" "the $2 edition is not installed; run: wc2bne install --edition $2"
}

cmd_install() {
  local data=$1 edition=$2 porcelain=$4 setup fd=''
  [ $# -eq 4 ] || usage_error "install takes no arguments"
  ! is_installed "$data" "$edition" || die "$E_FAILED" "already installed in $data/$edition (delete that directory to reinstall)"
  [ -n "${DISPLAY:-}" ] || die "$E_FAILED" "the installer needs an X display"
  need_deps bwrap wine wine32
  if needs_download "${WC2BNE_SETUP:-}" "$data/War2Combat46_en.exe" "${WC2BNE_SHA256:-$INSTALLER_SHA256}" "${WC2BNE_OFFLINE:-}"; then
    need_deps curl
  fi
  if [ -n "$porcelain" ]; then exec 7>&1; fd=7; fi
  setup=$(find_installer "${WC2BNE_SETUP:-}" "$data/War2Combat46_en.exe" \
    "${WC2BNE_SHA256:-$INSTALLER_SHA256}" "${WC2BNE_OFFLINE:-}" "$fd")
  lock_edition "$data" "$edition" install
  run_installer "$data" "$edition" "$setup" "$fd"
  if [ "$edition" = official ]; then
    reg_set_list "$data" "$edition" "$GW_OVERRIDE" 2000 01 connect-forever.classic.blizzard.com 8 'Global (GoG)'
  fi
  record_files "$data" "$edition"   # last: it marks the edition as installed
  if [ -n "$porcelain" ]; then report "$fd" step=done; else echo "installed: $edition"; fi
}

cmd_play() {
  local data=$1 edition=$2 mode=$3 changed
  [ $# -eq 4 ] || usage_error "play takes no arguments"
  need_installed "$data" "$edition"
  if [ -n "${WAYLAND_DISPLAY:-}" ]; then need_deps bwrap wine wine32 Xwayland; else need_deps bwrap wine wine32; fi
  changed=$(changed_files "$data" "$edition")
  [ -z "$changed" ] || die "$E_CHECKSUM" "changed since install, so servers would refuse it: ${changed//$'\n'/, }" \
    "(to reinstall, delete $(game_dir "$data" "$edition") and run: wc2bne install --edition $edition)"
  lock_edition "$data" "$edition" play
  run_game "$data" "$edition" "$mode" "${WAYLAND_DISPLAY:-}"
}

cmd_gateway() {
  local data=$1 edition=$2 porcelain=$4 op=${5:-list} list rows
  shift 4; if [ $# -gt 0 ]; then shift; fi
  case $op:$# in
    list:0|reset:0|set:0|add:[23]|select:1|remove:1) ;;
    *) usage_error "gateway needs: list, add HOST NAME [GMT-OFFSET], select N, remove N, set, or reset";;
  esac
  if [ $op = add ]; then gw_valid "$1" "${3:-0}" "$2" || usage_error "invalid gateway: $*"; fi
  need_installed "$data" "$edition"
  if [ $op != list ]; then need_deps bwrap wine; lock_edition "$data" "$edition" gateway; fi
  if [ $op = reset ]; then
    reg_unset "$data" "$edition" "$GW_OVERRIDE"
    echo "using the game's own gateway list"
    return
  fi
  list=$(gw_read "$data" "$edition"); mapfile -t list <<<"$list"
  case $op in
    select|remove) gw_has "$1" "${list[@]}" || usage_error "no gateway number $1";;
  esac
  case $op in
    add) list=$(gw_add "$1" "${3:-0}" "$2" "${list[@]}");;
    set) list=$(gw_from_rows "${list[0]:-2000}") || usage_error "invalid gateway list";;
    select) list=$(gw_select "$1" "${list[@]}");;
    remove) [ "$(gw_count "${list[@]}")" -gt 1 ] || die "$E_FAILED" "the last gateway can't be removed (try: wc2bne gateway reset)"
            list=$(gw_remove "$1" "${list[@]}");;
  esac
  if [ $op != list ]; then
    mapfile -t list <<<"$list"
    reg_set_list "$data" "$edition" "$GW_OVERRIDE" "${list[@]}"
  fi
  rows=$(gw_rows "${list[@]}")
  if [ -n "$porcelain" ]; then printf '%s\n' "$rows"; else gw_table <<<"$rows"; fi
}

# current_port DATA EDITION  -> the UDP game port, or the default if the edition isn't installed. Sandbox.
current_port() {
  local port=''
  if is_installed "$1" "$2"; then port=$(reg_get_number "$1" "$2" "$REG_PORT"); fi
  printf '%s\n' "${port:-$(setting_default port)}"
}

cmd_config() {
  local data=$1 edition=$2 mode=$3 porcelain=$4 op=${5:-list} name=${6:-} value=${7:-} gap=' = '
  case $op:$# in
    list:[45])
      if [ -n "$porcelain" ]; then gap='='; fi
      printf '%s\n' "edition$gap$edition" "mode$gap$mode" "port$gap$(current_port "$data" "$edition")";;
    get:6)
      case $name in
        edition) printf '%s\n' "$edition";;
        mode) printf '%s\n' "$mode";;
        port) current_port "$data" "$edition";;
        data-folder) printf '%s\n' "$data";;
        *) usage_error "unknown setting: $name";;
      esac;;
    set:7)
      if [ "$name" = data-folder ]; then
        if [ "$value" = default ]; then value=''; else value=$(absolute "$value"); fi
        remember_folder "${XDG_CONFIG_HOME:-$HOME/.config}" "$value"
        return
      fi
      valid_setting "$name" "$value" || usage_error "invalid setting: $name $value"
      if [ "$name" = port ]; then
        need_installed "$data" "$edition"
        need_deps bwrap wine
        lock_edition "$data" "$edition" config
        reg_set_number "$data" "$edition" "$REG_PORT" "$value"
      else
        save_setting "$data" "$name" "$value"
      fi;;
    *) usage_error "config needs: list, get NAME, or set NAME VALUE";;
  esac
}

cmd_uninstall() {
  local data=$1 edition=$2 game reply=''
  [ $# -eq 4 ] || usage_error "uninstall takes no arguments"
  need_installed "$data" "$edition"
  game=$(game_dir "$data" "$edition")
  if [ -z "$yes" ]; then
    if [ -t 0 ]; then
      read -r -p "Delete $game, with its saved games and settings? Maps are kept. [y/N] " reply || true
    fi
    [[ $reply == [Yy] || $reply == [Yy][Ee][Ss] ]] || die "$E_FAILED" "nothing was removed (to remove without asking: --yes)"
  fi
  lock_edition "$data" "$edition" uninstall
  remove_edition "$data" "$edition"
  echo "removed: $edition"
}

# edition_state DATA EDITION  -> installed, changed or absent. Reads files.
edition_state() {
  if ! is_installed "$1" "$2"; then echo absent
  elif [ -n "$(changed_files "$1" "$2")" ]; then echo changed
  else echo installed; fi
}

cmd_status() {
  local data=$1 edition=$2 mode=$3 porcelain=$4 gap=': ' installer=absent e used
  [ $# -eq 4 ] || usage_error "status takes no arguments"
  if [ -n "$porcelain" ]; then gap='='; fi
  if [ -f "$data/War2Combat46_en.exe" ]; then installer=present; fi
  printf '%s\n' "data$gap$data" "installer$gap$installer" "edition$gap$edition" "mode$gap$mode"
  for e in community official; do printf '%s\n' "$e$gap$(edition_state "$data" "$e")"; done
  for e in community official; do
    used=$(edition_user "$data" "$e")
    printf '%s\n' "${e}_in_use$gap${used:-no}"
  done
}

cmd_open() {
  local data=$1 edition=$2
  [ $# -eq 4 ] || usage_error "open takes no arguments"
  need_installed "$data" "$edition"
  need_deps xdg-open
  if [ -n "$maps" ]; then xdg-open "$(maps_dir "$data")"; else xdg-open "$(game_dir "$data" "$edition")"; fi
}

cmd_doctor() {
  local porcelain=$4 needed=(bwrap wine wine32 curl) optional=(xrandr xdg-open) missing d state
  [ $# -eq 4 ] || usage_error "doctor takes no arguments"
  if [ -n "${WAYLAND_DISPLAY:-}" ]; then needed+=(Xwayland); else optional+=(Xwayland); fi
  missing=$(missing_deps "${needed[@]}" "${optional[@]}")
  for d in "${needed[@]}" "${optional[@]}"; do
    state=ok
    if grep -qxF "$d" <<<"$missing"; then
      state=missing
      if [[ " ${optional[*]} " == *" $d "* ]]; then state=optional; fi
    fi
    if [ -n "$porcelain" ]; then printf '%s\n' "$d=$state"
    else printf '%-10s %s\n' "$d" "${state/optional/missing (optional)}"; fi
  done
  if [ -n "$porcelain" ]; then
    missing=$(missing_deps "${needed[@]}")
    [ -n "$missing" ] || return 0
    mapfile -t missing <<<"$missing"
    install_commands "$(package_manager)" '' '' "${missing[@]}" | sed 's/^/install=/'
    exit "$E_MISSING_DEPENDENCY"
  fi
  need_deps "${needed[@]}"
}

cmd_exec() {
  local data=$1 edition=$2; shift 4
  [ $# -gt 0 ] || usage_error "exec needs a command"
  need_installed "$data" "$edition"
  need_deps bwrap wine
  lock_edition "$data" "$edition" exec
  box "$data" "$edition" net "$@"
}

main() {
  local data; data=$(data_folder "${WC2BNE_HOME:-}" "$(recall_folder "${XDG_CONFIG_HOME:-$HOME/.config}")" "$WC2BNE_ROOT" "${XDG_DATA_HOME:-$HOME/.local/share}")
  local cmd='' edition='' mode='' porcelain='' yes='' maps='' help='' args=() conf
  while [ $# -gt 0 ]; do
    case $1 in
      --edition) [ $# -ge 2 ] || usage_error "--edition needs a value"; edition=$2; shift;;
      --edition=*) edition=${1#*=};;
      --window|--fullscreen) mode=${1#--};;
      --porcelain) porcelain=1;;
      --yes) yes=1;;
      --maps) maps=1;;
      -h|--help) help=1;;
      --version) echo "wc2bne $WC2BNE_VERSION"; return;;
      --) shift; args+=("$@"); break;;
      -[!0-9]*) usage_error "unknown option: $1";;
      *) if [ -n "$cmd" ]; then args+=("$1"); else cmd=$1; fi;;
    esac
    shift
  done
  if [ "$cmd" = help ]; then help=1 cmd=${args[0]:-}; fi
  if [ -n "$help" ]; then
    if [ -z "$cmd" ]; then help_main; else help_command "$cmd" || usage_error "unknown command: $cmd"; fi
    return
  fi
  [ -n "$cmd" ] || usage_error "no command given"
  help_command "$cmd" >/dev/null || usage_error "unknown command: $cmd"
  if [ -n "$mode" ] && [ "$cmd" != play ]; then usage_error "--$mode is only for play"; fi
  if [ -n "$maps" ] && [ "$cmd" != open ]; then usage_error "--maps is only for open"; fi
  if [ -n "$yes" ] && [ "$cmd" != uninstall ]; then usage_error "--yes is only for uninstall"; fi
  if [ -n "$porcelain" ]; then
    case $cmd in config|gateway|status|install|doctor) ;; *) usage_error "--porcelain is not for $cmd";; esac
  fi
  conf=$(read_conf "$data")
  case "$cmd ${args[*]:0:2}" in
    "config set edition"|"config set mode"|"config set data-folder") conf='';;   # so that an invalid file can be repaired
  esac
  edition=$(setting edition "$edition" "${WC2BNE_EDITION:-}" "$conf")
  mode=$(setting mode "$mode" "${WC2BNE_MODE:-}" "$conf")
  "cmd_$cmd" "$data" "$edition" "$mode" "$porcelain" "${args[@]}"
}
