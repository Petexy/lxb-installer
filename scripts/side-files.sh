#!/usr/bin/env bash
# Runs one of the side-file tests (`manual::side_files`: the real e2fsprogs
# and ntfsprogs on image files the test makes) by name, by hand, inside a
# bubblewrap sandbox — the only way they are run (README.md, Verify):
#
#   scripts/side-files.sh ext4                    # the ext4 test
#   scripts/side-files.sh ntfs                    # the NTFS test
#   scripts/side-files.sh ntfs --strace FILE      # under strace, the record in FILE
#   scripts/side-files.sh --probe                 # look inside, run no test
#   scripts/side-files.sh --print ext4            # show the bwrap line, run nothing
#
# The test binary is built first, outside the sandbox, running nothing of it
# (`scripts/toolkit-tree.sh test --release --locked --lib --no-run`); then,
# inside, `scripts/toolkit-tree.sh test --release --locked --lib -- --ignored
# --exact <the test> --nocapture` has cargo start the one test set aside, as
# cargo starts every test binary. Both with the same emptied environment, so
# the second finds nothing to build.
#
# What the sandbox is (bwrap, unprivileged, a user namespace of its own):
#   - /dev is a tmpfs holding null, zero and urandom and nothing else: no
#     block device, no tty, no pts, no shm.
#   - No /sys and no /run at all: the sandbox's root is a tmpfs of its own
#     (read-only once it is set up), not this computer's.
#   - /proc is the sandbox's own, with its own process ids. Two of its files
#     show the whole computer whatever the namespace — /proc/swaps (every
#     swap device) and /proc/partitions (every disk) — and are replaced by
#     empty tables: the tools' own checks whether their device is mounted or
#     a swap read the first and look up every name in it.
#   - Every host path it needs comes in as an overlay, never as a bind: /usr,
#     /etc, the toolkit's tree, cargo's and rustup's homes (the toolchain)
#     and this worktree read-only, and the worktree's target/ with its
#     writes (cargo's lock and fingerprints, toolkit-tree.sh's manifest)
#     going to a tmpfs that vanishes with the sandbox. So /proc/mounts
#     names no device — only overlay, tmpfs, proc and dev — and the tools'
#     mounted checks, which look up every device name it lists, have no
#     disk's name to look up. /bin, /lib, /lib64 and /sbin are this
#     system's links into /usr.
#   - Writable: where the tests make their images — a tmpfs of its own,
#     gone with the sandbox. That is the test binary's `images::IMAGES`,
#     the `artifacts/storage-tests` of the manifest toolkit-tree.sh builds
#     from (target/toolkit-tree-manifest), which is a folder of target/'s
#     own or, where the manifest's `artifacts` is the link toolkit-tree.sh
#     makes, the worktree's artifacts/storage-tests; this script finds
#     which and mounts the tmpfs there, after target/'s overlay, and one at
#     the worktree's artifacts/storage-tests as well. Then target/, as
#     above. Nothing on this computer is written at all.
#   - No network (a namespace of its own), no new privileges, every
#     capability dropped, no further user namespace; it ends with this
#     script.
#
# --strace FILE runs the test under `strace -f`, tracing every way a path is
# opened, started, connected to or looked at (open, openat, execve,
# connect, readlink, readlinkat, stat, statx, newfstatat, lstat, access,
# faccessat, faccessat2, and the clones that make the tree). The
# record is written to FILE through a descriptor this script opens and hands
# in: FILE is outside the sandbox and never named inside it.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The toolkit's tree, found as toolkit-tree.sh finds it: LXB_TOOLKIT_TREE, or
# lxb-toolkit beside the main checkout (through git, so that a worktree finds
# the main checkout's), or beside this folder when it is no git checkout of
# its own.
if [ -n "${LXB_TOOLKIT_TREE:-}" ]; then
    tree=$LXB_TOOLKIT_TREE
else
    main=$here
    if command -v git >/dev/null 2>&1 \
        && top=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null) \
        && [ "$top" = "$(cd "$here" && pwd -P)" ]; then
        common=$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)
        main=$(dirname "$common")
    fi
    tree=$(dirname "$main")/lxb-toolkit
fi
cargo_home=${CARGO_HOME:-$HOME/.cargo}
rustup_home=${RUSTUP_HOME:-$HOME/.rustup}
images="$here/artifacts/storage-tests"

# Defined before any line calls it: bash looks a function up only when it
# is called, and one not yet defined is "command not found", exit 127.
refuse() {
    echo "side-files: $*" >&2
    exit 2
}

# Where the test binary makes its images (`images::IMAGES`: the manifest's
# artifacts/storage-tests, CARGO_MANIFEST_DIR being toolkit-tree.sh's
# manifest), followed through any link the manifest has: a folder under
# target/, or the worktree's own artifacts/storage-tests.
manifest="$here/target/toolkit-tree-manifest"
made=$(realpath -m "$manifest/artifacts/storage-tests")
case "$made" in
    "$images" | "$here/target/"*) ;;
    *) refuse "the test binary's images would be made at $made, outside this worktree's artifacts/storage-tests and target/" ;;
esac

test_name=""
record=""
probe=0
print=0
while [ $# -gt 0 ]; do
    case "$1" in
        ext4)
            test_name=worker::manual::side_files::an_ext4_on_a_side_file_is_shrunk_by_its_commands_checks_clean_and_grows_back ;;
        ntfs)
            test_name=worker::manual::side_files::an_ntfs_on_a_side_file_is_read_as_the_measure_reads_it_and_shrunk_by_its_commands ;;
        --strace)
            shift
            [ $# -gt 0 ] || refuse "--strace needs the file the record goes to"
            record=$1 ;;
        --probe) probe=1 ;;
        --print) print=1 ;;
        *) refuse "unknown argument '$1' (ext4, ntfs, --strace FILE, --probe, --print)" ;;
    esac
    shift
done
if [ "$probe" -eq 0 ] && [ -z "$test_name" ]; then
    refuse "which test: ext4 or ntfs"
fi
command -v bwrap >/dev/null || refuse "bubblewrap (bwrap) is not installed"
[ -d "$tree" ] || refuse "no toolkit tree at $tree (set LXB_TOOLKIT_TREE)"
[ -d "$cargo_home" ] || refuse "no cargo home at $cargo_home"
[ -d "$rustup_home" ] || refuse "no rustup home at $rustup_home"
if [ -n "$record" ]; then
    command -v strace >/dev/null || refuse "strace is not installed"
fi
mkdir -p "$images" "$made"

# overlayfs mounts a read-only overlay only over two layers at least: each
# host path is given an empty folder of this script's own as its second,
# which adds nothing to it.
empty=$(mktemp -d "${TMPDIR:-/tmp}/lxb-side-files.XXXXXX")
trap 'rmdir "$empty"' EXIT

# The one environment both cargo runs see.
environment=(PATH=/usr/bin HOME="$HOME" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home"
    LANG=C.UTF-8
    LXB_TOOLKIT_TREE="$tree")

# ---------------------------------------------------------------------------
# The sandbox.
bw=(bwrap
    --unshare-all --unshare-user --disable-userns
    --die-with-parent --new-session --cap-drop ALL
    --clearenv
)
for setting in "${environment[@]}"; do
    bw+=(--setenv "${setting%%=*}" "${setting#*=}")
done
bw+=(--overlay-src /usr --overlay-src "$empty" --ro-overlay /usr)
for top in bin sbin lib lib64; do
    if [ -L "/$top" ]; then
        bw+=(--symlink "$(readlink "/$top")" "/$top")
    elif [ -d "/$top" ]; then
        bw+=(--overlay-src "/$top" --overlay-src "$empty" --ro-overlay "/$top")
    fi
done
bw+=(--overlay-src /etc --overlay-src "$empty" --ro-overlay /etc
    --overlay-src "$tree" --overlay-src "$empty" --ro-overlay "$tree"
    --overlay-src "$cargo_home" --overlay-src "$empty" --ro-overlay "$cargo_home"
    --overlay-src "$rustup_home" --overlay-src "$empty" --ro-overlay "$rustup_home"
    --overlay-src "$here" --overlay-src "$empty" --ro-overlay "$here"
    --overlay-src "$here/target" --tmp-overlay "$here/target"
    --tmpfs "$images"
)
# The images' own tmpfs where the test binary makes them, mounted after
# target/'s overlay so that it is what the path reaches, not the overlay.
if [ "$made" != "$images" ]; then
    bw+=(--tmpfs "$made")
fi
bw+=(
    --proc /proc
    --ro-bind-data 3 /proc/swaps
    --ro-bind-data 4 /proc/partitions
    --tmpfs /dev
    --dev-bind /dev/null /dev/null
    --dev-bind /dev/zero /dev/zero
    --dev-bind /dev/urandom /dev/urandom
    --remount-ro /dev
    --chdir "$here"
)

if [ "$probe" -eq 1 ]; then
    # Read-only looks from inside; none can change anything outside.
    inside=(/usr/bin/sh -c '
        echo "== /dev"; ls -A /dev
        echo "== block devices under /dev"; find /dev -type b 2>/dev/null; echo "(end)"
        echo "== /sys"; ls -A /sys 2>&1; echo "(end)"
        echo "== /run"; ls -A /run 2>&1; echo "(end)"
        echo "== /proc/mounts"; cat /proc/mounts
        echo "== a device named in /proc/mounts"; cut -d" " -f1 /proc/mounts | grep "^/" ; echo "(end)"
        echo "== /proc/swaps"; cat /proc/swaps; echo "(end)"
        echo "== /proc/partitions"; cat /proc/partitions; echo "(end)"
        echo "== privileges"; grep -E "^(NoNewPrivs|CapEff|CapBnd|CapPrm)" /proc/self/status
        echo "== the images'"'"' folders"; grep -F " '"$images"' " /proc/mounts; grep -F " '"$made"' " /proc/mounts; echo "(end)"
        echo "== writes"; for at in / /etc /usr /dev "$HOME" "$PWD" "$PWD/target" '"$images"' '"$made"'; do
            if touch "$at/.side-files-probe" 2>/dev/null; then rm -f "$at/.side-files-probe"; echo "$at writable"; else echo "$at read-only"; fi
        done')
else
    inside=("$here/scripts/toolkit-tree.sh" test --release --locked --lib --
        --ignored --exact "$test_name" --nocapture)
    if [ -n "$record" ]; then
        inside=(strace -f -s 512
            -e trace=execve,open,openat,readlink,readlinkat,stat,statx,newfstatat,lstat,access,faccessat,faccessat2,clone,clone3,vfork,connect
            -o /proc/self/fd/5
            "${inside[@]}")
    fi
fi

# Last, the sandbox's own root: read-only, so only the images' tmpfs and the
# overlays' vanishing upper layers take a write.
bw+=(--remount-ro /)

if [ "$print" -eq 1 ]; then
    printf '%q ' "${bw[@]}" "${inside[@]}"
    printf '3<swaps 4<partitions\n'
    exit 0
fi

if [ "$probe" -eq 0 ]; then
    env -i "${environment[@]}" "$here/scripts/toolkit-tree.sh" test --release --locked --lib --no-run
fi
swaps=$'Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n'
partitions=$'major minor  #blocks  name\n\n'
if [ -n "$record" ]; then
    exec 5>"$record"
fi
"${bw[@]}" "${inside[@]}" 3<<<"$swaps" 4<<<"$partitions"
