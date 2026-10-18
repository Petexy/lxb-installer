#!/usr/bin/env bash

set -euo pipefail

packaging_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=packaging/lib.sh
source "$packaging_dir/lib.sh"

build=true
case "${1:-}" in
    --no-build) build=false ;;
    -h | --help)
        echo "Usage: packaging/check.sh [--no-build]"
        exit 0
        ;;
    "") ;;
    *) package_die "unknown check option: $1" ;;
esac

shell_scripts() {
    find "$PACKAGING_DIR" -type f -name '*.sh' -print0
    find "$PROJECT_ROOT/scripts" -type f -name '*.sh' -print0 2>/dev/null || true
}

package_note "checking shell syntax"
while IFS= read -r -d '' script; do
    bash -n "$script"
done < <(shell_scripts)
bash -n "$PACKAGING_DIR/arch/PKGBUILD.in"

if command -v shellcheck >/dev/null 2>&1; then
    while IFS= read -r -d '' script; do
        shellcheck -x "$script"
    done < <(shell_scripts)
else
    package_note "shellcheck is not installed; syntax was checked but not linted"
fi

package_note "checking the package version is consistent"
read_toml_version() {
    local file="$1"
    local wanted_section="$2"
    awk -v wanted="$wanted_section" '
        /^\[/ { section = $0; next }
        section == wanted \
            && match($0, /^version[[:space:]]*=[[:space:]]*"[^"]+"/) {
            line = substr($0, RSTART, RLENGTH)
            sub(/^version[[:space:]]*=[[:space:]]*"/, "", line)
            sub(/"$/, "", line)
            print line
            exit
        }' "$file"
}

# `lxb-installer --version` prints CARGO_PKG_VERSION, so the manifest is where
# a running program says what it is. There is no bumper script: the version
# is shared with the family's other repos and moves with them, by hand, in
# the four places this compares.
manifest_version="$(read_toml_version "$PROJECT_ROOT/Cargo.toml" "[package]")"
[[ -n "$manifest_version" ]] \
    || package_die "could not read [package] version from Cargo.toml"
[[ "$manifest_version" == "$PACKAGE_VERSION" ]] \
    || package_die "VERSION says $PACKAGE_VERSION but Cargo.toml says $manifest_version"

# Every packaged build is --locked or --frozen, so a lock file naming the old
# version is not a stale file: it is a build that refuses to start.
locked_version="$(awk '
    /^name = "lxb-installer"$/ { found = 1; next }
    found && match($0, /^version = "[^"]+"/) {
        line = substr($0, RSTART, RLENGTH)
        sub(/^version = "/, "", line)
        sub(/"$/, "", line)
        print line
        exit
    }' "$PROJECT_ROOT/Cargo.lock")"
[[ "$locked_version" == "$PACKAGE_VERSION" ]] \
    || package_die "Cargo.lock still says lxb-installer $locked_version, not $PACKAGE_VERSION.
Run: cargo update --workspace --offline"

# AppStream orders releases newest first, and the newest one is what a
# software centre shows as this application's version.
newest_release="$(awk '
    match($0, /<release version="[^"]+"/) {
        line = substr($0, RSTART, RLENGTH)
        sub(/^<release version="/, "", line)
        sub(/"$/, "", line)
        print line
        exit
    }' "$PROJECT_ROOT/data/io.github.petexy.lxb-installer.metainfo.xml")"
[[ "$newest_release" == "$PACKAGE_VERSION" ]] \
    || package_die "the metainfo's newest release is $newest_release, not $PACKAGE_VERSION.
Add a <release> for $PACKAGE_VERSION to data/io.github.petexy.lxb-installer.metainfo.xml"

grep -Fqx 'pkgver=@VERSION@' "$PACKAGING_DIR/arch/PKGBUILD.in" \
    || package_die "arch/PKGBUILD.in no longer reads its version from VERSION"

package_note "checking the PKGBUILD asks for the toolkit Cargo.toml asks for"
# Rendered by arch/build.sh from Cargo.toml, so what is checked is that it
# still is: a literal that crept back in would be right on the day it was
# written and wrong from the next release onwards.
toolkit_version="$(package_toolkit_requirement)"
grep -Fq 'lxb-toolkit>=@TOOLKIT_VERSION@' "$PACKAGING_DIR/arch/PKGBUILD.in" \
    || package_die "arch/PKGBUILD.in no longer takes its toolkit requirement from Cargo.toml"
package_note "it is lxb-toolkit $toolkit_version or newer, to build"

package_note "checking every program the worker runs is a dependency"
# The worker runs its programs by name, on the live system and inside the new
# system through arch-chroot — and the new system is a copy of the live one.
# The measure entry (--measure) runs its own few on the live system too.
# A program missing from the live system is found only when an installation
# reaches the step that runs it, on somebody's computer, half done. So the
# worker's source is read for every program it names, and each must be in
# the table below, and its package in the PKGBUILD.
#
# The table is the one place a program is mapped to its Arch package; a new
# program in the worker with no row here stops this check until it has one.
program_packages() {
    cat <<'EOF'
arch-chroot arch-install-scripts
blkid util-linux
bootctl systemd
btrfs btrfs-progs
chpasswd shadow
chown coreutils
dconf dconf
du coreutils
dumpe2fs e2fsprogs
e2fsck e2fsprogs
efibootmgr efibootmgr
env coreutils
findmnt util-linux
flatpak flatpak
gpgconf gnupg
groupdel shadow
grub-install grub
grub-mkconfig grub
locale-gen glibc
lsblk util-linux
lspci pciutils
mkfs.btrfs btrfs-progs
mkfs.ext4 e2fsprogs
mkfs.fat dosfstools
mkfs.xfs xfsprogs
mkinitcpio mkinitcpio
mkswap util-linux
mount util-linux
nmcli networkmanager
ntfscat ntfsprogs
ntfsresize ntfsprogs
os-prober os-prober
pacman pacman
pacman-key pacman
partx util-linux
passwd shadow
pkexec polkit
resize2fs e2fsprogs
rsync rsync
runuser util-linux
sfdisk util-linux
sh bash
sync coreutils
systemctl systemd
timeout coreutils
udevadm systemd
umount util-linux
useradd shadow
userdel shadow
visudo sudo
wipefs util-linux
xdg-user-dirs-update xdg-user-dirs
EOF
}

# Every source file as one line, so a call broken over several lines by
# rustfmt reads the same as one that is not. Test modules are part of the
# files and their programs are matched too; the two names only tests use are
# skipped by name: a program that must not exist, and printf, the one program
# the test build's fence lets any test run (`run::fence::ANY_TEST`), which no
# installation runs.
source_text="$(find "$PROJECT_ROOT/src" -name '*.rs' -print0 | sort -z | xargs -0 cat | tr '\n' ' ')"
named_programs() {
    # run::run("sfdisk", …), run::run_input(…), run::run_streaming(…), and
    # run::run_status(…), whose exit code is the answer (e2fsck's)
    grep -oE 'run::run(_input|_streaming|_status)?\([[:space:]]*"[^"]+"' <<<"$source_text" \
        | sed -E 's/.*"([^"]+)"$/\1/'
    # run::chroot(&root, "pacman", …) and its _input and _streaming forms
    grep -oE 'run::chroot(_input|_streaming)?\([^,()]+,[[:space:]]*"[^"]+"' <<<"$source_text" \
        | sed -E 's/.*"([^"]+)"$/\1/'
    # chroot("chown", …): a program handed to a runner a step is given (its
    # `Chroot`, run::chroot in an installation, a recorder in a test)
    grep -oE '(^|[^:_A-Za-z0-9])chroot\([[:space:]]*"[^"]+"' <<<"$source_text" \
        | sed -E 's/.*"([^"]+)"$/\1/'
    # command_path("os-prober"): looked up before it is run
    grep -oE 'command_path\("[^"]+"\)' <<<"$source_text" | sed -E 's/.*"([^"]+)".*/\1/'
    # The partition plan's own Command::new("wipefs", …)
    grep -oE 'Command::new\([[:space:]]*"[^"]+"' <<<"$source_text" | sed -E 's/.*"([^"]+)"$/\1/'
    # Programs reached by absolute path: pkexec, systemctl, chpasswd …
    grep -oE '"/usr/bin/[A-Za-z0-9._-]+"' <<<"$source_text" | sed -E 's|"/usr/bin/([^"]+)"|\1|'
    # The worker's own list of what the live system must have, and the
    # image's list of what the new system must have.
    awk '/^pub fn required_tools/,/^}/' "$PROJECT_ROOT/src/worker/system/check.rs" \
        | grep -oE '"[A-Za-z0-9._-]+"' | tr -d '"'
    awk '/^pub const TARGET_PROGRAMS/,/^\];/' "$PROJECT_ROOT/src/worker/system/image.rs" \
        | grep -oE '"[A-Za-z0-9._-]+"' | tr -d '"'
}
mapfile -t programs < <(named_programs | sed -E 's|^/usr/s?bin/||' \
    | grep -vxF -e lxb-installer -e definitely-not-a-real-tool-name -e printf | sort -u)
((${#programs[@]} > 20)) \
    || package_die "found only ${#programs[@]} programs in the worker's source; the patterns above no longer match it"

# The PKGBUILD's own arrays, read the way makepkg reads them. Sourced in a
# subshell: it assigns pkgname, source and the rest, which belong to nobody here.
mapfile -t declared < <(
    set +u
    # shellcheck disable=SC1091
    source "$PACKAGING_DIR/arch/PKGBUILD.in"
    printf '%s\n' "${depends[@]}"
    printf '%s\n' "${optdepends[@]%%:*}"
)
missing=()
for program in "${programs[@]}"; do
    package="$(program_packages | awk -v p="$program" '$1 == p { print $2; exit }')"
    if [[ -z "$package" ]]; then
        missing+=("$program (no row in check.sh's table)")
    elif ! printf '%s\n' "${declared[@]}" | grep -Fqx "$package"; then
        missing+=("$program ($package is not in depends or optdepends)")
    fi
done
((${#missing[@]} == 0)) || package_die "the worker runs programs the package does not bring:
$(printf '  %s\n' "${missing[@]}")"
# And the two loader packages, whose files the worker copies rather than runs.
for package in grub refind; do
    printf '%s\n' "${declared[@]}" | grep -Fqx "$package" \
        || package_die "$package is not in depends; the worker copies its files to set the boot loader up"
done
package_note "${#programs[@]} programs, each brought by a declared package"

package_note "checking the shown name agrees everywhere"
# Three places name this application to a person, and they are read by three
# different things: the desktop entry by every menu, the AppStream component by
# every software centre, and the string handed to the toolkit by the window
# itself. Two of them agreeing is what a rename looks like when it is half done.
desktop_name="$(awk -F= '/^Name=/ { print substr($0, 6); exit }' \
    "$PROJECT_ROOT/data/lxb-installer.desktop")"
metainfo_name="$(awk 'match($0, /<name>[^<]+<\/name>/) {
        line = substr($0, RSTART, RLENGTH)
        sub(/^<name>/, "", line)
        sub(/<\/name>$/, "", line)
        print line
        exit
    }' "$PROJECT_ROOT/data/io.github.petexy.lxb-installer.metainfo.xml")"
# The window title is a message, so the id is read out of the source and the
# words out of en-GB.ftl — the fallback catalogue, which is the language the
# desktop entry's own untranslated Name= is in.
window_title_id="$(grep -o 'App::new(APP_ID, i18n::text("[^"]*"))' \
    "$PROJECT_ROOT/src/main.rs" | sed -e 's/^.*text("//' -e 's/"))$//' | sort -u)"
[[ -n "$window_title_id" ]] \
    || package_die "src/main.rs names no window title message"
[[ "$(printf '%s\n' "$window_title_id" | wc -l)" -eq 1 ]] \
    || package_die "src/main.rs opens windows under more than one message:
$window_title_id"
window_name="$(awk -v id="$window_title_id" \
    '$1 == id && $2 == "=" { sub(/^[^=]*= /, ""); print; exit }' \
    "$PROJECT_ROOT/locales/en-GB.ftl")"
[[ -n "$desktop_name" ]] || package_die "the desktop entry has no Name="
[[ -n "$metainfo_name" ]] || package_die "the metainfo has no <name>"
[[ -n "$window_name" ]] \
    || package_die "locales/en-GB.ftl has no $window_title_id, which is the message src/main.rs opens its window under"
[[ "$desktop_name" == "$metainfo_name" && "$desktop_name" == "$window_name" ]] \
    || package_die "this application is called three things:
  desktop entry  $desktop_name
  metainfo       $metainfo_name
  window title   $window_name"
package_note "it is called '$desktop_name'"

package_note "checking the polkit actions name the program pkexec is handed"
# pkexec picks the action whose exec.path is exactly the program it was given
# and whose exec.argv1, where an action has one, is exactly the first argument.
# The interface hands it src/launch.rs's PROGRAM with --worker (the install
# action) or --measure (the read-only measure action); install.sh puts the
# binary at /usr/bin/lxb-installer. All of it has to agree, or the person is
# asked polkit's generic question instead of the installer's own — and with
# argv1 on both actions, neither mode can be raised under the other's question.
policy="$PROJECT_ROOT/data/io.github.petexy.lxb-installer.policy"
launched="$(sed -n 's|^pub const PROGRAM: &str = "\([^"]*\)";|\1|p' "$PROJECT_ROOT/src/launch.rs")"
[[ "$launched" == /usr/bin/lxb-installer ]] \
    || package_die "src/launch.rs hands pkexec '$launched', not /usr/bin/lxb-installer"
# One annotation of one action: read from the action's own block, from its id
# to its </action>, so the two actions' values are never mixed up.
action_annotation() {
    awk -v id="$1" -v key="org.freedesktop.policykit.exec.$2" '
        index($0, "<action id=\"" id "\">") { inside = 1 }
        inside && index($0, "<annotate key=\"" key "\">") {
            line = $0
            sub(/.*<annotate key="[^"]*">/, "", line)
            sub(/<\/annotate>.*/, "", line)
            print line
        }
        inside && /<\/action>/ { inside = 0 }' "$policy"
}
[[ "$(grep -c '<action id=' "$policy")" -eq 2 ]] \
    || package_die "the polkit file should hold two actions, install and measure"
for action in install:--worker measure:--measure; do
    id="io.github.petexy.lxb-installer.${action%%:*}"
    argument="${action#*:}"
    exec_path="$(action_annotation "$id" path)"
    argv1="$(action_annotation "$id" argv1)"
    [[ "$exec_path" == /usr/bin/lxb-installer ]] \
        || package_die "the polkit action $id has exec.path '$exec_path', not /usr/bin/lxb-installer"
    [[ "$argv1" == "$argument" ]] \
        || package_die "the polkit action $id has exec.argv1 '$argv1', not $argument"
done
if command -v xmllint >/dev/null 2>&1; then
    xmllint --noout --nonet "$policy" || package_die "the polkit file is not well-formed XML"
fi

if [[ "$build" == true ]]; then
    package_note "building the release artefact"
    require_rust_version 1.90
    require_toolkit_sources
    (cd "$PROJECT_ROOT" && cargo build --locked --release)
fi

target_dir="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
[[ "$target_dir" == /* ]] || target_dir="$PROJECT_ROOT/$target_dir"

package_note "checking the binary reports the packaged version"
program="$target_dir/release/lxb-installer"
[[ -x "$program" ]] || package_die "missing release binary: $program
Run without --no-build, or build it first."
reported="$("$program" --version)"
[[ "$reported" == "lxb-installer $PACKAGE_VERSION" ]] \
    || package_die "the binary reports '$reported', not 'lxb-installer $PACKAGE_VERSION'"

package_note "staging the payload"
stage="$(package_work_dir lxb-installer-check)"
cleanup() {
    if [[ -n "${stage:-}" && "$stage" == */lxb-installer-check.* && -d "$stage" ]]; then
        rm -rf -- "$stage"
    fi
}
trap cleanup EXIT

"$PACKAGING_DIR/install.sh" \
    --destdir "$stage" \
    --prefix /usr \
    --target-dir "$target_dir" >/dev/null
staged="$(cd "$stage" && find . -mindepth 1 \( -type f -o -type l \) -printf '%P\n' | sort)"

package_note "checking the payload is complete"
for expected in \
    usr/bin/lxb-installer \
    usr/share/applications/lxb-installer.desktop \
    usr/share/icons/hicolor/scalable/apps/lxb-installer.svg \
    usr/share/metainfo/io.github.petexy.lxb-installer.metainfo.xml \
    usr/share/polkit-1/actions/io.github.petexy.lxb-installer.policy; do
    printf '%s\n' "$staged" | grep -Fqx "$expected" \
        || package_die "the staged payload is missing $expected"
done
[[ "$(stat -c %a "$stage/usr/bin/lxb-installer")" == 755 ]] \
    || package_die "usr/bin/lxb-installer is not staged 0755"

# The list above is what the package promises; this is what the checkout has.
# A file added to data/ and not to install.sh is a file that exists everywhere
# except in the package. data/pictures/, data/glyphs/, data/places/ and
# data/flags/ are compiled into the binary (the PKGBUILD installs the places'
# and the flags' licence notices).
while IFS= read -r file; do
    printf '%s\n' "$staged" | grep -Fq "/$file" \
        || package_die "data/$file is in the checkout but in no package"
done < <(cd "$PROJECT_ROOT/data" && find . -maxdepth 1 -type f -printf '%P\n' | sort)

package_note "checking the desktop entry points at what was installed"
exec_name="$(awk -F= '/^Exec=/ { print $2; exit }' \
    "$stage/usr/share/applications/lxb-installer.desktop" | awk '{print $1}')"
icon_name="$(awk -F= '/^Icon=/ { print $2; exit }' \
    "$stage/usr/share/applications/lxb-installer.desktop")"
[[ -x "$stage/usr/bin/$exec_name" ]] \
    || package_die "the desktop entry runs '$exec_name', which the package does not install"
[[ -f "$stage/usr/share/icons/hicolor/scalable/apps/$icon_name.svg" ]] \
    || package_die "the desktop entry wears '$icon_name', which the package does not install"

package_note "checking the desktop entry is valid"
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$stage/usr/share/applications/lxb-installer.desktop" \
        || package_die "the installed desktop entry is not valid"
else
    package_note "desktop-file-validate is not installed; the entry was not validated"
fi

package_note "checking the AppStream metadata is valid"
if command -v appstreamcli >/dev/null 2>&1; then
    # `--no-net`: this must pass on a build machine with no network.
    appstreamcli validate --no-net --pedantic \
        "$stage/usr/share/metainfo/io.github.petexy.lxb-installer.metainfo.xml" \
        || package_die "the installed metainfo does not validate"
else
    package_note "appstreamcli is not installed; the metadata was not validated"
fi

package_note "all checks passed for lxb-installer $PACKAGE_VERSION"
