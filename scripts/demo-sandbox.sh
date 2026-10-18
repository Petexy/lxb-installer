#!/usr/bin/env bash
# Runs the installer's demo in a bubblewrap sandbox where no disk of this
# computer can be reached — the one way the installer is run on a
# development machine (docs/design.md, "Proven on virtual disks only"):
#
#   scripts/demo-sandbox.sh --demo                       # the demo, in a window
#   scripts/demo-sandbox.sh --demo --shot x.png --scene review --width 1280 --height 800
#   scripts/demo-sandbox.sh --home /tmp/lxb-nested/home --demo   # beside a nested LineXinBar
#   scripts/demo-sandbox.sh --probe                      # look inside, run nothing of ours
#   scripts/demo-sandbox.sh --print --demo               # show the bwrap line, run nothing
#
# Options of its own, before the installer's:
#   --binary PATH    the installer to run (target/toolkit-tree/release/lxb-installer,
#                    which scripts/toolkit-tree.sh builds, or where there is none
#                    target/release/lxb-installer, which cargo build does)
#   --home DIR       an existing scratch HOME to use and keep (never your own
#                    home, nor a folder above it or a hidden one in it); without
#                    it a new /tmp/lxb-demo.XXXXXX is made and removed afterwards
#   --session-bus    let the session bus in (off by default; see below)
#   --probe          run the checks below inside the same sandbox instead
#   --print          print the bwrap command line and stop
#
# The installer's own arguments must include --demo, and only the demo's and
# the pictures' flags are let through: --worker, --measure and --list-disks
# are refused wherever they stand, and so is anything this script does not
# know. A picture's file (--shot FILE) is written in the scratch HOME inside
# the sandbox and moved to FILE outside it afterwards, by this script.
#
# What the sandbox is (bwrap, unprivileged, a user namespace of its own):
#   - /dev is bwrap's own small one (null, zero, full, random, urandom, tty,
#     pts, shm) plus the GPU's render nodes (/dev/dri/renderD*), which is all
#     wgpu's Vulkan drivers open to draw: no block device, no card node, no
#     /dev/input, no /dev/snd, no /dev/uinput.
#   - /usr and a few files of /etc (the loader's cache, fonts, passwd and
#     group, the time zone) are read-only; /home, /mnt, /media, /var, /srv,
#     /opt and /run are not there at all, so neither is /run/archiso — the
#     installer cannot even think it is on the live medium.
#   - /sys is read-only with nothing in /sys/block, /sys/class/block or
#     /sys/dev/block, and /proc (the sandbox's own) shows /proc/partitions
#     and /proc/diskstats empty: lsblk finds no disk. /sys/devices stays for
#     the GPU drivers, read-only.
#   - No system D-Bus (/run/dbus is not there), so no polkit, pkexec, udisks
#     or logind; no_new_privs is set, so a setuid program gains nothing;
#     every capability is dropped and no further user namespace can be made.
#   - Writable: only the scratch HOME, /tmp (a tmpfs of its own) and
#     XDG_RUNTIME_DIR (/run/user/UID, a tmpfs holding the one Wayland socket
#     of the session given by WAYLAND_DISPLAY — yours, or a nested
#     LineXinBar's — and nothing else).
#   - Its own network namespace (no network; the demo is online by
#     pretending), its own process ids, and it ends with this script.
#
# The session bus is off: nothing the demo shows needs it. The shell's
# on-screen keyboard, its settings and the window all reach the installer
# through Wayland or the scratch HOME's ~/.config/lxb/shell.toml, never
# D-Bus. What --session-bus adds is only the toolkit's keeping the screen
# awake while Installing is up (org.freedesktop.ScreenSaver and
# PowerManagement on the session bus) — and with it a door out of the
# sandbox, to every service of the session.
#
# A pad cannot be used inside: the toolkit reads pads from /dev/input, which
# is not given. A picture says --pad; so can a window (--demo --pad), which
# then opens as with a pad in hand — its clicks do not put the pad down, and
# the first key does — so that it is driven as a pad drives it by clicks,
# and a board's keys type into it.
#
# The session's desktop (XDG_CURRENT_DESKTOP) is let through, a name and
# nothing more: it is how the installer knows it runs under LineXinBar,
# whose own on-screen keyboard a pad then types with. So is WAYLAND_DEBUG,
# which only prints what the window says to its compositor.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary=""
home=""
session_bus=0
probe=0
print=0

refuse() {
    echo "demo-sandbox: $*" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --binary) [ $# -ge 2 ] || refuse "--binary wants a path"; binary=$2; shift 2 ;;
        --home) [ $# -ge 2 ] || refuse "--home wants a directory"; home=$2; shift 2 ;;
        --session-bus) session_bus=1; shift ;;
        --probe) probe=1; shift ;;
        --print) print=1; shift ;;
        --) shift; break ;;
        *) break ;;
    esac
done
if [ -z "$binary" ]; then
    binary="$here/target/toolkit-tree/release/lxb-installer"
    [ -e "$binary" ] || binary="$here/target/release/lxb-installer"
fi

[ "$(id -u)" -ne 0 ] || refuse "not as root"
command -v bwrap >/dev/null || refuse "bubblewrap (bwrap) is not installed"

# ---------------------------------------------------------------------------
# The installer's arguments: --demo, and only the flags a demo or a picture
# takes. Each flag that takes a value takes exactly the next argument, which
# may not itself look like a flag.
args=()
demo=0
shot=""
shot_name=""
if [ "$probe" -eq 0 ]; then
    while [ $# -gt 0 ]; do
        flag=$1
        shift
        case "$flag" in
            --worker | --measure | --list-disks)
                refuse "$flag is never run on this machine" ;;
            --demo)
                demo=1; args+=("$flag") ;;
            --pad | --low-end | --shells-board | --help | -h | --version)
                args+=("$flag") ;;
            --shot)
                [ $# -ge 1 ] || refuse "--shot wants a file"
                shot=$1
                shift
                shot_name=$(basename -- "$shot")
                case "$shot_name" in
                    *.png) ;;
                    *) refuse "--shot writes a .png, not $shot" ;;
                esac
                args+=("$flag" "$shot_name") ;;
            --width | --height | --scene | --pointer | --shot-at | --from | --press | --then | --then-at \
            | --greeting | --search)
                [ $# -ge 1 ] || refuse "$flag wants a value"
                case "$1" in
                    --*) refuse "$flag wants a value, not $1" ;;
                esac
                args+=("$flag" "$1")
                shift ;;
            *)
                refuse "$flag is not a demo's or a picture's flag" ;;
        esac
    done
    [ "$demo" -eq 1 ] || refuse "only the demo runs here: give --demo"
    [ -f "$binary" ] && [ -x "$binary" ] || refuse "no installer at $binary (build it: cargo build --release --locked, or scripts/toolkit-tree.sh build --release --locked)"
    binary=$(realpath -- "$binary")
elif [ $# -gt 0 ]; then
    refuse "--probe takes no installer arguments"
fi

# ---------------------------------------------------------------------------
# The scratch HOME.
real_home=$(realpath -- "$HOME")
if [ -z "$home" ]; then
    home=$(mktemp -d /tmp/lxb-demo.XXXXXX)
    trap 'rm -rf -- "$home"' EXIT
else
    [ -d "$home" ] || refuse "--home $home is not a directory"
    home=$(realpath -- "$home")
    [ -O "$home" ] || refuse "--home $home is not yours"
    case "$real_home/" in
        "$home"/*) refuse "--home $home is your home or holds it" ;;
    esac
    case "$home" in
        "$real_home"/.*) refuse "--home $home is a hidden folder of your home" ;;
    esac
fi

uid=$(id -u)
runtime=/run/user/$uid

# ---------------------------------------------------------------------------
# The sandbox.
bw=(bwrap
    --unshare-all --unshare-user --disable-userns
    --die-with-parent --new-session --cap-drop ALL
    --clearenv
    --ro-bind /usr /usr
)
# /bin, /lib and the rest as this system has them: links into /usr, or
# directories of their own.
for top in bin sbin lib lib64; do
    if [ -L "/$top" ]; then
        bw+=(--symlink "$(readlink "/$top")" "/$top")
    elif [ -d "/$top" ]; then
        bw+=(--ro-bind "/$top" "/$top")
    fi
done
for file in /etc/ld.so.cache /etc/ld.so.conf /etc/ld.so.conf.d /etc/fonts \
    /etc/passwd /etc/group /etc/nsswitch.conf /etc/localtime /etc/vulkan /etc/drirc; do
    if [ -e "$file" ]; then
        bw+=(--ro-bind "$file" "$file")
    fi
done
bw+=(--proc /proc
    --ro-bind-data 3 /proc/partitions
    --ro-bind-data 4 /proc/diskstats
    --dev /dev
)
for node in /dev/dri/renderD*; do
    if [ -c "$node" ]; then
        bw+=(--dev-bind "$node" "$node")
    fi
done
bw+=(--ro-bind /sys /sys
    --tmpfs /sys/block --tmpfs /sys/class/block --tmpfs /sys/dev/block
    --remount-ro /sys/block --remount-ro /sys/class/block --remount-ro /sys/dev/block
)
# Each disk's own directory under /sys/devices too (a partition's is inside
# its disk's): the GPU drivers need /sys/devices, nothing needs a disk's.
for entry in /sys/class/block/*; do
    [ -e "$entry" ] && [ ! -e "$entry/partition" ] || continue
    device=$(realpath -- "$entry")
    case "$device" in
        /sys/devices/*) bw+=(--tmpfs "$device" --remount-ro "$device") ;;
    esac
done
bw+=(--tmpfs /tmp
    --perms 0700 --tmpfs "$runtime"
    --bind "$home" "$home"
    --chdir "$home"
    --setenv HOME "$home"
    --setenv XDG_RUNTIME_DIR "$runtime"
    --setenv PATH /usr/bin
)
for name in LANG LANGUAGE LC_ALL LC_MESSAGES XKB_DEFAULT_LAYOUT XKB_DEFAULT_VARIANT \
    XCURSOR_THEME XCURSOR_SIZE LXB_BACKGROUND_HANDOFF RUST_BACKTRACE \
    XDG_CURRENT_DESKTOP WAYLAND_DEBUG; do
    if [ -n "${!name:-}" ]; then
        bw+=(--setenv "$name" "${!name}")
    fi
done

# The one Wayland socket: the session's given by WAYLAND_DISPLAY (a name in
# XDG_RUNTIME_DIR, or a whole path). A picture needs none and gets none.
if [ "$probe" -eq 1 ] || [ -z "$shot" ]; then
    display=${WAYLAND_DISPLAY:-}
    if [ -n "$display" ]; then
        case "$display" in
            /*) socket=$display ;;
            *) socket=${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR is not set}/$display ;;
        esac
        [ -S "$socket" ] || refuse "WAYLAND_DISPLAY names no socket: $socket"
        bw+=(--ro-bind "$socket" "$runtime/wayland-0" --setenv WAYLAND_DISPLAY wayland-0)
    elif [ "$probe" -eq 0 ]; then
        refuse "a window needs WAYLAND_DISPLAY (a picture, --shot, does not)"
    fi
fi

if [ "$session_bus" -eq 1 ]; then
    address=${DBUS_SESSION_BUS_ADDRESS:-}
    case "$address" in
        unix:path=*) bus=${address#unix:path=}; bus=${bus%%,*} ;;
        *) refuse "--session-bus needs DBUS_SESSION_BUS_ADDRESS=unix:path=…, not '$address'" ;;
    esac
    [ -S "$bus" ] || refuse "the session bus is not at $bus"
    bw+=(--ro-bind "$bus" "$runtime/bus" --setenv DBUS_SESSION_BUS_ADDRESS "unix:path=$runtime/bus")
fi

if [ "$probe" -eq 1 ]; then
    # Read-only looks from inside; none of them can change anything, and
    # every one of them is expected to find nothing or fail.
    inside=(/usr/bin/sh -c '
        echo "== /dev"; ls -A /dev
        echo "== /dev/dri"; ls -A /dev/dri
        echo "== block devices under /dev"; find /dev -type b 2>/dev/null; echo "(end)"
        echo "== lsblk"; lsblk; echo "lsblk exit $?"
        echo "== /sys/block, /sys/class/block, /sys/dev/block"; ls -A /sys/block /sys/class/block /sys/dev/block
        echo "== /proc/partitions"; cat /proc/partitions; echo "(end)"
        echo "== /proc/diskstats"; cat /proc/diskstats; echo "(end)"
        echo "== busctl --system"; busctl --system list 2>&1; echo "busctl --system exit $?"
        echo "== /run"; find /run 2>&1
        echo "== /run/archiso"; if [ -e /run/archiso ]; then echo PRESENT; else echo absent; fi
        echo "== the Wayland socket"; if [ -z "${WAYLAND_DISPLAY:-}" ]; then echo "none given"
        elif command -v wayland-info >/dev/null; then wayland-info 2>&1 | grep -c "interface:" | sed "s/^/globals offered: /"
        else echo "(no wayland-info to ask it)"; fi
        echo "== /home, /mnt, /media, /var"; ls -d /home /mnt /media /var 2>&1
        echo "== privileges"; grep -E "^(NoNewPrivs|CapEff|CapBnd|CapPrm)" /proc/self/status
        echo "== a disk under /sys/devices"; find /sys/devices -path "*/block/*" -name dev 2>/dev/null; echo "(end)"
        echo "== writes"; for at in / /etc /app /usr /sys /run /tmp "$XDG_RUNTIME_DIR" "$HOME"; do
            if touch "$at/.demo-sandbox-probe" 2>/dev/null; then rm -f "$at/.demo-sandbox-probe"; echo "$at writable"; else echo "$at read-only"; fi
        done')
else
    bw+=(--ro-bind "$binary" /app/lxb-installer)
    inside=(/app/lxb-installer "${args[@]}")
fi

# Last, the sandbox's own root, which holds /etc's few files, /app and
# /run: read-only, so only the scratch HOME, /tmp and XDG_RUNTIME_DIR take a
# write, each a mount of its own.
bw+=(--remount-ro /)

if [ "$print" -eq 1 ]; then
    printf '%q ' "${bw[@]}" "${inside[@]}"
    printf '3</dev/null 4</dev/null\n'
    exit 0
fi

status=0
"${bw[@]}" "${inside[@]}" 3</dev/null 4</dev/null || status=$?

# The picture comes out of the scratch HOME to where it was asked for.
if [ -n "$shot" ] && [ "$status" -eq 0 ]; then
    if [ -f "$home/$shot_name" ]; then
        mv -f -- "$home/$shot_name" "$shot"
    else
        echo "demo-sandbox: the installer wrote no $shot_name" >&2
        status=1
    fi
fi
exit "$status"
