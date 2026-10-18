#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/lib.sh
source "$script_dir/../lib.sh"

output_dir="$PACKAGING_DIR/out/arch"
allow_foreign=false
makepkg_extra=()

usage() {
    cat <<'EOF'
Usage: packaging/arch/build.sh [OPTIONS] [-- MAKEPKG OPTIONS]

Options:
  --output-dir DIR       Artifact directory (default: packaging/out/arch)
  --work-dir DIR         Where makepkg builds (default: packaging/out/build)
  --allow-foreign-host   Permit a test build on an Arch-derived non-Arch host

The build is not run under /tmp: that is a tmpfs on most machines, and this
draws through a GPU renderer whose locked graph is nearly 400 crates. A release
build of it takes gigabytes, and check() builds the whole of it again in the
dev profile. LXB_INSTALLER_WORK_DIR sets the same thing.

The package must run on every x86-64 computer Linexin is installed on, so a
makepkg.conf that tunes for the building machine's own CPU (-march=native,
target-cpu=native, as CachyOS's does) is refused. Build in a plain Arch
container instead, for example a distrobox of archlinux:latest.

Examples:
  packaging/arch/build.sh
  packaging/arch/build.sh -- --syncdeps
  packaging/arch/build.sh -- --nocheck
  packaging/arch/build.sh --work-dir /var/tmp/lxb-installer
EOF
}

while (($#)); do
    case "$1" in
        --output-dir)
            (($# >= 2)) || package_die "--output-dir requires a value"
            output_dir="$2"
            shift 2
            ;;
        --work-dir)
            (($# >= 2)) || package_die "--work-dir requires a value"
            package_set_work_root "$2"
            shift 2
            ;;
        --allow-foreign-host)
            allow_foreign=true
            shift
            ;;
        --)
            shift
            makepkg_extra=("$@")
            break
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *) package_die "unknown Arch builder option: $1" ;;
    esac
done

if [[ "$allow_foreign" != true ]] && ! host_is_like arch; then
    package_die "build Arch packages on Arch Linux or pass --allow-foreign-host for a derivative"
fi
if [[ "$output_dir" != /* ]]; then
    output_dir="$PWD/$output_dir"
fi

require_command makepkg
require_command sha256sum
require_rust_version 1.90
require_toolkit_sources

# makepkg reads its flags from makepkg.conf and the drop-ins beside it, and the
# environment adds to them. A build tuned for this machine's CPU runs here and
# dies with an illegal instruction on an older one, before main, even on
# --version; for an installer that is somebody's computer that cannot be set
# up. So ask makepkg's own files, the way makepkg reads them, and refuse.
tuned="$(
    # makepkg.conf is written for makepkg, not for `set -u`.
    set +u
    # shellcheck disable=SC1091
    . /etc/makepkg.conf 2>/dev/null
    for conf in /etc/makepkg.conf.d/*.conf; do
        # shellcheck disable=SC1090
        [[ -r "$conf" ]] && . "$conf"
    done
    printf '%s\n' "${CFLAGS:-} ${CXXFLAGS:-}" "${RUSTFLAGS:-}"
)"
tuned="$tuned
${CFLAGS:-} ${CXXFLAGS:-}
${RUSTFLAGS:-} ${CARGO_BUILD_RUSTFLAGS:-} ${CARGO_ENCODED_RUSTFLAGS:-}"
if grep -Eq -- '-march=native|-mtune=native|target-cpu=native|-march=x86-64-v[234]|target-cpu=x86-64-v[234]' <<<"$tuned"; then
    package_die "this machine's makepkg.conf or environment tunes builds for one CPU:
$(grep -Eo -- '-march=[^ ]+|-mtune=[^ ]+|target-cpu=[^ ]+' <<<"$tuned" | sort -u | tr '\n' ' ')
The installer has to run on every x86-64 computer. Build it in a plain Arch
container (distrobox create -i archlinux:latest) with Arch's own makepkg.conf."
fi

work="$(package_work_dir lxb-installer-arch)"
cleanup() {
    if [[ -n "${work:-}" && "$work" == */lxb-installer-arch.* && -d "$work" ]]; then
        rm -rf -- "$work"
    fi
}
trap cleanup EXIT

# check() builds the whole graph a second time in the dev profile; that is most
# of this number. Ask for it before extracting anything, so a machine without
# the room is told now rather than partway through.
if [[ " ${makepkg_extra[*]} " == *" --nocheck "* ]]; then
    require_free_space "$work" 3072
else
    require_free_space "$work" 6144
fi

# makepkg.conf wins over the environment, so a machine that has set BUILDDIR
# builds there whatever this script was told. Say so rather than let the build
# fail somewhere this script was careful not to put it.
if [[ -n "$(
    . /etc/makepkg.conf 2>/dev/null
    printf '%s' "${BUILDDIR:-}"
)" ]]; then
    package_note "note: makepkg.conf sets BUILDDIR; makepkg builds there, not in $work"
fi

source_dir="$work/lxb-installer-$PACKAGE_VERSION"
source_archive="$work/lxb-installer-$PACKAGE_VERSION.tar.gz"
snapshot_source "$source_dir"
archive_snapshot "$source_dir" "$source_archive"
checksum="$(sha256sum "$source_archive" | awk '{print $1}')"
sed -e "s/@SOURCE_SHA256@/$checksum/" \
    -e "s/@VERSION@/$PACKAGE_VERSION/" \
    -e "s/@TOOLKIT_VERSION@/$(package_toolkit_requirement)/" \
    "$script_dir/PKGBUILD.in" > "$work/PKGBUILD"

package_note "building the Arch package"
(
    cd "$work"
    makepkg --cleanbuild --clean --force "${makepkg_extra[@]}"
)

mkdir -p "$output_dir"
# Ask makepkg where it put things rather than looking beside the PKGBUILD: a
# makepkg.conf that sets PKGDEST writes elsewhere, and a search of this
# directory would then quietly collect nothing. `--packagelist` also names the
# debug package when makepkg.conf asks for one.
collected=0
while IFS= read -r artifact; do
    [[ -f "$artifact" ]] || continue
    install -m0644 "$artifact" "$output_dir/$(basename "$artifact")"
    collected=$((collected + 1))
done < <(cd "$work" && makepkg --packagelist)
((collected > 0)) || package_die "makepkg reported no package files to collect"

package_note "created $collected Arch artifact(s) in $output_dir"
