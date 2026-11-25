# One user at a time per edition. Two Wine servers on one registry would overwrite each other's changes.

# lock_file DATA EDITION  -> path. Pure.
lock_file() { printf '%s\n' "$1/.locks/$2"; }

# lock_edition DATA EDITION PURPOSE  Holds the edition until this process exits, or exits 6 if something else has it.
# The lock is file descriptor 9, which must be closed for anything run in the sandbox (9>&-).
lock_edition() {
  local file; file=$(lock_file "$1" "$2")
  mkdir -p "$(dirname "$file")"
  exec 9>>"$file"
  flock -w 1 9 || die "$E_IN_USE" "the $2 edition is in use ($(cat "$file")); close it first"
  printf '%s\n' "$3" >"$file"
}

# edition_user DATA EDITION  -> what holds the edition (the PURPOSE it gave), or nothing. Reads files.
edition_user() {
  local file; file=$(lock_file "$1" "$2")
  if [ -f "$file" ] && ! flock -n 8 8<"$file"; then cat "$file"; fi
}
