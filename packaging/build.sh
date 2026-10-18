#!/usr/bin/env bash

set -euo pipefail

packaging_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    cat <<'EOF_USAGE'
Usage: packaging/build.sh TARGET [TARGET OPTIONS]

Targets:
  check    Validate the package definition and its staged payload
  arch     Build the Arch package with makepkg

The installer puts Linexin, an Arch-based system, on a computer, and it runs
from Linexin's live system; Arch is therefore the one package format. Build it
on Arch with a generic toolchain. See packaging/README.md.
EOF_USAGE
}

target="${1:-}"
if [[ -z "$target" || "$target" == "-h" || "$target" == "--help" ]]; then
    usage
    [[ -n "$target" ]] && exit 0
    exit 2
fi
shift

case "$target" in
    check) exec "$packaging_dir/check.sh" "$@" ;;
    arch) exec "$packaging_dir/arch/build.sh" "$@" ;;
    *)
        echo "unknown package target: $target" >&2
        usage >&2
        exit 2
        ;;
esac
