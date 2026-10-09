#!/bin/sh
set -eu
app_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ ! -x "$app_dir/loglens" ]; then "$app_dir/build.sh" >/dev/null; fi
# The source-command file provider is rooted at cwd. Resolve the input's
# directory here so both relative and absolute user paths work.
if [ "$#" -eq 0 ]; then exec "$app_dir/loglens"; fi
file=$1
shift
case "$file" in
  --*) printf '%s\n' 'usage: loglens <file> [--top N] [--json]' >&2; exit 2 ;;
esac
# Check usage before resolving a file, including an inaccessible directory.
need_top=false
top=5
json=false
for option in "$@"; do
  if [ "$need_top" = true ]; then
    n=$option
    case "$n" in ''|*[!0-9]*) printf '%s\n' 'usage: loglens <file> [--top N] [--json]' >&2; exit 2 ;; esac
    while [ "${n#0}" != "$n" ]; do n=${n#0}; done
    if [ -z "$n" ] || [ "${#n}" -gt 2 ] || [ "$n" -gt 50 ]; then
      printf '%s\n' 'usage: loglens <file> [--top N] [--json]' >&2; exit 2
    fi
    top=$n
    need_top=false
  else
    case "$option" in
      --json) json=true ;;
      --top) need_top=true ;;
      *) printf '%s\n' 'usage: loglens <file> [--top N] [--json]' >&2; exit 2 ;;
    esac
  fi
done
if [ "$need_top" = true ]; then printf '%s\n' 'usage: loglens <file> [--top N] [--json]' >&2; exit 2; fi
dir=$(dirname -- "$file")
base=$(basename -- "$file")
if ! cd -- "$dir" 2>/dev/null; then
  printf '%s\n' 'loglens: cannot read file' >&2
  exit 1
fi
if [ "$json" = true ]; then
  exec "$app_dir/loglens" "$base" --top "$top" --json
fi
exec "$app_dir/loglens" "$base" --top "$top"
