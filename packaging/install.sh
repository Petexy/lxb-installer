#!/usr/bin/env bash

# Stage the installer's payload: the binary, the polkit action its worker is
# raised through, and the three files that make it an application rather than
# a command.
#
# This deliberately does not install the licence or any documentation: the
# PKGBUILD places those, where Arch wants them, and this stays the part that
# is the program itself.

set -euo pipefail

packaging_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/lib.sh
source "$packaging_dir/lib.sh"

destdir=""
prefix="/usr"
target_dir="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
binary=""

usage() {
    cat <<'EOF'
Usage: packaging/install.sh --destdir DIR [--prefix /usr]
                            [--target-dir DIR] [--binary FILE]

Stages the Linexin Installer into DESTDIR. --target-dir defaults to
CARGO_TARGET_DIR or ./target; --binary names the executable outright, for a
builder that has put it somewhere neither of those describes.

  usr/bin/lxb-installer
  usr/share/applications/lxb-installer.desktop
  usr/share/icons/hicolor/scalable/apps/lxb-installer.svg
  usr/share/metainfo/io.github.petexy.lxb-installer.metainfo.xml
  usr/share/polkit-1/actions/io.github.petexy.lxb-installer.policy

The prefix is /usr and nothing else. The interface raises its worker with
`pkexec /usr/bin/lxb-installer --worker`: that path is compiled into the
program and named by the polkit action, so an installer staged anywhere else
would open, and then fail at Install by asking polkit for a program that is
not there.
EOF
}

while (($#)); do
    case "$1" in
        --destdir)
            (($# >= 2)) || package_die "--destdir requires a value"
            destdir="$2"
            shift 2
            ;;
        --prefix)
            (($# >= 2)) || package_die "--prefix requires a value"
            prefix="$2"
            shift 2
            ;;
        --target-dir)
            (($# >= 2)) || package_die "--target-dir requires a value"
            target_dir="$2"
            shift 2
            ;;
        --binary)
            (($# >= 2)) || package_die "--binary requires a value"
            binary="$2"
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *) package_die "unknown install option: $1" ;;
    esac
done

# DESTDIR in the environment is what a `make install` takes; it still works, so
# a recipe written that way is not silently installing nothing.
destdir="${destdir:-${DESTDIR:-}}"

[[ -n "$destdir" ]] || package_die "--destdir is required"
[[ "$destdir" == /* ]] || package_die "--destdir must be absolute"
prefix="${prefix%/}"
[[ "$prefix" == /usr ]] || package_die "the prefix must be /usr, not '$prefix'.
The worker is raised as /usr/bin/lxb-installer, a path compiled into the program
and named by the polkit action; see --help."

if [[ "$target_dir" != /* ]]; then
    target_dir="$PROJECT_ROOT/$target_dir"
fi
# `--destdir /` is how somebody installs onto the machine they are standing at,
# and it is the one value that would otherwise concatenate into `//usr`.
destdir="${destdir%/}"
install_root="${destdir}${prefix}"

binary="${binary:-${BINARY:-$target_dir/release/lxb-installer}}"
[[ -x "$binary" ]] || package_die "missing release binary: $binary
Build it first: cargo build --locked --release"

install -Dm0755 "$binary" "$install_root/bin/lxb-installer"
install -Dm0644 "$PROJECT_ROOT/data/lxb-installer.desktop" \
    "$install_root/share/applications/lxb-installer.desktop"
# Scalable and nothing else, as in the family's other applications: the mark
# is a shape evaluated at the size it is wanted.
install -Dm0644 "$PROJECT_ROOT/data/lxb-installer.svg" \
    "$install_root/share/icons/hicolor/scalable/apps/lxb-installer.svg"
install -Dm0644 "$PROJECT_ROOT/data/io.github.petexy.lxb-installer.metainfo.xml" \
    "$install_root/share/metainfo/io.github.petexy.lxb-installer.metainfo.xml"
# What pkexec asks polkit for when it is handed /usr/bin/lxb-installer: the
# action whose exec.path names that program. Without it pkexec falls back to
# its generic "run a program as another user" question, which says nothing of
# installing Linexin and is asked again on every try.
install -Dm0644 "$PROJECT_ROOT/data/io.github.petexy.lxb-installer.policy" \
    "$install_root/share/polkit-1/actions/io.github.petexy.lxb-installer.policy"

package_note "staged lxb-installer $PACKAGE_VERSION into $install_root"
