# Messages and errors.
#
# Conventions for every file in lib/:
#   - Functions take arguments and print their result; they don't read or set globals other than constants.
#   - "pure" means no files, processes, environment or clock: same arguments, same output.
#   - cli.sh reads the environment and passes values down. box (sandbox.sh) also reads the session's
#     display, sound and language variables.
#   - `data` is the data folder; `edition` is community or official.

# Exit codes, part of the public interface (docs/SPEC.md).
E_FAILED=1 E_USAGE=2 E_NOT_INSTALLED=3 E_MISSING_DEPENDENCY=4 E_CHECKSUM=5 E_IN_USE=6

# warn MESSAGE...  Prints to stderr.
warn() { printf 'wc2bne: %s\n' "$*" >&2; }

# die CODE MESSAGE...  Prints to stderr and exits with CODE.
die() { local code=$1; shift; warn "$@"; exit "$code"; }
