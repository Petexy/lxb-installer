#!/usr/bin/env bash

# Shared helpers for the distro package builders. This file is sourced; it is
# not an entry point on its own.

PACKAGING_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$PACKAGING_DIR/.." && pwd)"

# The one number this project releases under. It lives at the root of the
# checkout rather than in here because it is not a packaging detail: the
# manifest, the AppStream release list and `lxb-installer --version` all have
# to agree with it, and check.sh holds them to it.
#
# `read` reports failure on a final line with no newline, which under `set -e`
# would end the caller with no explanation. Take the value either way and let
# the pattern below be the one thing that rejects it.
PACKAGE_VERSION_FILE="$PROJECT_ROOT/VERSION"
IFS= read -r PACKAGE_VERSION < "$PACKAGE_VERSION_FILE" || true

if [[ ! "$PACKAGE_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "invalid package version in $PACKAGE_VERSION_FILE: $PACKAGE_VERSION" >&2
    exit 1
fi

# Where a package build does its work.
#
# Deliberately not `${TMPDIR:-/tmp}`. On a systemd machine /tmp is a tmpfs sized
# at a fraction of RAM, and building there means building in memory. The
# installer draws through the toolkit's GPU renderer: the locked graph is
# nearly 400 crates, wgpu and naga among them, and a release build of it takes
# gigabytes before the dev-profile build that the test phase adds. A live
# system, where somebody might well build this, has the least RAM to spare.
#
# The default is beside the artifacts, on whatever filesystem the checkout is
# on, which is the one place already known to hold a build of this project.
PACKAGE_WORK_ROOT="${LXB_INSTALLER_WORK_DIR:-$PACKAGING_DIR/out/build}"

package_die() {
    echo "error: $*" >&2
    exit 1
}

package_note() {
    echo "==> $*"
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || package_die "required command not found: $1"
}

host_is_like() {
    local wanted="$1"

    # In a subshell: /etc/os-release assigns a dozen names (NAME, VERSION,
    # BUILD_ID …) and none of them belong in a build script's environment.
    (
        if [[ -r /etc/os-release ]]; then
            # Distribution-supplied shell assignments only.
            # shellcheck disable=SC1091
            source /etc/os-release
        fi
        [[ " ${ID:-} ${ID_LIKE:-} " == *" $wanted "* ]]
    )
}

# require_rust_version [MINIMUM] [HINT]
#
# HINT is a line added to the refusal, for a builder that knows how its own
# distribution gets a newer Rust.
require_rust_version() {
    local minimum="${1:-1.90}"
    local hint="${2:-}"
    local actual
    local first

    require_command rustc
    require_command cargo
    # Empty when rustc does not answer, which is how rustup reads before a
    # toolchain has been chosen — and `|| true` because under the builders'
    # pipefail that failure would otherwise end the script before a word is said.
    actual="$(rustc --version 2>/dev/null | awk '{print $2}')" || true
    first="$(printf '%s\n%s\n' "$minimum" "$actual" | sort -V | head -n 1)"
    if [[ "$first" != "$minimum" ]]; then
        package_die "Rust $minimum or newer is required by the locked dependency graph (found ${actual:-none})${hint:+
$hint}"
    fi
}

# The toolkit is a *build* dependency and not a runtime one, which is the thing
# about this project that surprises everybody who packages it. `lxb-app` is a
# Rust path dependency: cargo compiles those sources into this binary, so the
# finished program links no liblxb_*.so at all and a live system that runs it
# needs nothing of the toolkit installed. A machine that *builds* it needs the
# crate sources, which the lxb-toolkit package puts under /usr/share.
#
# That path is what Cargo.toml names, and it is checked rather than assumed:
# Cargo's own error for a missing path dependency names a directory and no
# package.
PACKAGE_TOOLKIT_CRATES="${LXB_TOOLKIT_CRATE_DIR:-/usr/share/lxb-toolkit/crates}"

require_toolkit_sources() {
    local wanted
    wanted="$(package_toolkit_requirement)"

    [[ -d "$PACKAGE_TOOLKIT_CRATES/lxb-app" ]] || package_die \
        "the toolkit's crate sources are not at $PACKAGE_TOOLKIT_CRATES/lxb-app.
Cargo compiles them into this binary, so lxb-toolkit $wanted or newer has to be
installed to build. LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And the GilRs the toolkit carries, which lxb-input reads every pad
    # through. A toolkit released before it shipped one has every crate above
    # and not this one, and Cargo would name only a missing directory — or,
    # under --locked, a lock file it will not update.
    [[ -d "$PACKAGE_TOOLKIT_CRATES/lxb-gilrs" ]] || package_die \
        "the toolkit's copy of GilRs is not at $PACKAGE_TOOLKIT_CRATES/lxb-gilrs.
It carries the fix for a pad that is turned off and on again, and the toolkit's
controls read every pad through it, so this needs an lxb-toolkit that ships it.
LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And a toolkit that tells a page what the pointer hovers, which the
    # installer lights its controls by. Cargo.toml asks for 0.9.4, which a
    # released 0.9.3 cannot answer; a toolkit tree built between the two still
    # carries a newer number without every call, and Cargo would stop on a
    # missing method with no word about the toolkit, so each is named here.
    grep -q 'pub fn hovered' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        || package_die "the toolkit at $PACKAGE_TOOLKIT_CRATES/lxb-app does not tell a page
what the pointer hovers (Page::hovered), which the installer lights its controls
by. This needs lxb-toolkit 0.9.4.
LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And one that moves a page whole, says whether the session is counted
    # low-end and photographs something on its way, which every change of
    # page and every picture of one is drawn with: the toolkit has them
    # since its motion work, and a tree from between the two has hovering
    # and not these.
    grep -q 'pub fn low_end' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        && grep -q 'pub fn shot_after' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        && grep -q 'pub fn carry_between' "$PACKAGE_TOOLKIT_CRATES/lxb-render/src/renderer.rs" 2>/dev/null \
        || package_die "the toolkit at $PACKAGE_TOOLKIT_CRATES cannot move a page whole
(Ui::carry_between), say whether the session is counted low-end (Page::low_end)
or photograph something on its way (App::shot_after), which the installer
changes its pages and takes pictures of them with. This needs the lxb-toolkit
built from its tree after its motion work, or 0.9.4.
LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And one whose lights stand on what is chosen on low-end hardware: one
    # from before left the erase question's light on "Go back" there while
    # Enter answered Erase. The tree has it with App::low_end and Page::act,
    # which the installer's pictures of low-end hardware and of a question
    # being answered are taken with.
    grep -q 'pub fn set_low_end' "$PACKAGE_TOOLKIT_CRATES/lxb-render/src/renderer.rs" 2>/dev/null \
        && grep -q 'pub fn act(&mut self, action: Action)' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        && grep -q 'pub fn low_end(mut self)' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        && grep -q 'pub fn menu_on' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        || package_die "the toolkit at $PACKAGE_TOOLKIT_CRATES leaves a question's light where it was
first drawn on low-end hardware (no Ui::set_low_end), so the erase question can
show \"Go back\" lit while Enter erases, or it cannot count itself low-end
(App::low_end), press for a picture (Page::act) or open a menu on a row
(Page::menu_on), which Options comes back to the option just changed with.
This needs the lxb-toolkit built from its tree after its low-end lights and
Page::menu_on, or 0.9.4.
LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And one that can bring a page's words in after its glass (Carry::words),
    # with the caret and the disc under a tick counted among them
    # (Ui::words_between), and fade a page into its pane at the sides
    # (Ui::soft_side_edges), which every change of page is drawn with: one
    # from before has none of them, and Cargo would stop on a missing field
    # or method.
    grep -q 'pub words: f32' "$PACKAGE_TOOLKIT_CRATES/lxb-render/src/renderer.rs" 2>/dev/null \
        && grep -q 'pub fn words_between' "$PACKAGE_TOOLKIT_CRATES/lxb-render/src/renderer.rs" 2>/dev/null \
        && grep -q 'pub fn soft_side_edges' "$PACKAGE_TOOLKIT_CRATES/lxb-render/src/components.rs" 2>/dev/null \
        || package_die "the toolkit at $PACKAGE_TOOLKIT_CRATES cannot bring a page's words in after its
glass (Carry::words), count a caret or a tick's disc among them
(Ui::words_between) or fade a page into its pane at the sides
(Ui::soft_side_edges), which the installer changes its pages with. This needs
the lxb-toolkit built from its tree after them, or 0.9.4.
LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And one that draws an application's own marks (App::add_mark), the tick
    # and the switch in the marks' material (Ui::chosen, Ui::switch) and
    # carries the shell's account marks, which Where to install, You and
    # every chosen row and switch are drawn with: one from before has none
    # of them, and Cargo would stop on a missing method — or, for a missing
    # mark, the row would go without it.
    grep -q 'pub fn add_mark' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        && grep -q 'pub fn chosen' "$PACKAGE_TOOLKIT_CRATES/lxb-render/src/components.rs" 2>/dev/null \
        && grep -q 'pub fn switch_width' "$PACKAGE_TOOLKIT_CRATES/lxb-render/src/components.rs" 2>/dev/null \
        && [[ -f "$PACKAGE_TOOLKIT_CRATES/lxb-toolkit/assets/glyphs/setting-account-type.svg" ]] \
        || package_die "the toolkit at $PACKAGE_TOOLKIT_CRATES cannot draw an application's own marks
(App::add_mark), the tick or the switch in the marks' material (Ui::chosen,
Ui::switch), or does not carry the shell's account marks (setting-name,
setting-username, setting-password, setting-account-type), which Where to
install, You and every chosen row are drawn with. This needs the lxb-toolkit
built from its tree after them, or 0.9.4.
LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And one that says where LineXinBar's on-screen keyboard stands
    # (lxb_toolkit::board), which the field a pad types into with it is kept
    # above, and lets a field take a keyboard's typing with nothing said to
    # the compositor (Page::taking_keys), which the field the light stands in
    # with a pad in hand does, and reads the letter y as Options, which a
    # search is summoned with from a keyboard: one from before has none of
    # them, and Cargo would stop on the first two with no word about the
    # toolkit.
    grep -q 'pub fn rect(width: f32, height: f32)' "$PACKAGE_TOOLKIT_CRATES/lxb-toolkit/src/board.rs" 2>/dev/null \
        && grep -q 'pub fn taking_keys' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        && grep -q 'fn letter_press' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        || package_die "the toolkit at $PACKAGE_TOOLKIT_CRATES does not say where LineXinBar's
on-screen keyboard stands (lxb_toolkit::board::rect), which the field a
controller types into with it is kept above, or cannot take a keyboard's
typing quietly (Page::taking_keys), which the field the light stands in takes
it with while a controller is in hand, or does not read the letter y as
Options, which a search is summoned with. This needs the lxb-toolkit built
from its tree after them, or 0.9.4.
LXB_TOOLKIT_CRATE_DIR points somewhere else."

    # And one that knows when that keyboard stands over the window
    # (Page::shells_board), which the room kept for it follows — a field
    # clicked or tapped, and the room given back the moment the board is put
    # away: Cargo would stop on it with no word about the toolkit.
    grep -q 'pub fn shells_board' "$PACKAGE_TOOLKIT_CRATES/lxb-app/src/lib.rs" 2>/dev/null \
        || package_die "the toolkit at $PACKAGE_TOOLKIT_CRATES does not know when LineXinBar's
on-screen keyboard stands over the window (Page::shells_board), which the
room kept for it follows. This needs the lxb-toolkit built from its tree
after it, or 0.9.4.
LXB_TOOLKIT_CRATE_DIR points somewhere else."
}

# What Cargo.toml asks of the toolkit, which is the number the package
# definition has to require. Read rather than repeated: a requirement written
# out twice is a requirement that is wrong in one place one day.
package_toolkit_requirement() {
    local version
    version="$(awk '
        /^lxb-app[[:space:]]*=/ {
            if (match($0, /version[[:space:]]*=[[:space:]]*"[^"]+"/)) {
                line = substr($0, RSTART, RLENGTH)
                sub(/version[[:space:]]*=[[:space:]]*"/, "", line)
                sub(/"$/, "", line)
                print line
                exit
            }
        }' "$PROJECT_ROOT/Cargo.toml")"
    [[ -n "$version" ]] || package_die "could not read the lxb-app requirement from Cargo.toml"
    printf '%s\n' "$version"
}

# Resolve `.` and `..` so that the guard in package_work_dir compares places
# rather than spellings: `--work-dir ../build` names somewhere outside the
# checkout, and against the raw string it still begins with $PROJECT_ROOT.
#
# Lexically, and deliberately not with `realpath`: PROJECT_ROOT and PACKAGING_DIR
# come from `cd`+`pwd`, which keeps whatever symlinks the caller walked through.
# Resolving them on one side of that comparison and not the other is how the
# same directory gets two spellings and the guard reads the wrong one.
package_normalize_path() {
    local path="$1"
    local rest="$path"
    local out=""
    local segment

    while [[ -n "$rest" ]]; do
        segment="${rest%%/*}"
        if [[ "$rest" == */* ]]; then
            rest="${rest#*/}"
        else
            rest=""
        fi
        case "$segment" in
            '' | .) ;;
            # `..` above the root stays at the root, as the kernel does it.
            ..) out="${out%/*}" ;;
            *) out="$out/$segment" ;;
        esac
    done

    printf '%s\n' "${out:-/}"
}

package_set_work_root() {
    local root="$1"

    [[ -n "$root" ]] || package_die "--work-dir requires a value"
    if [[ "$root" != /* ]]; then
        root="$PWD/$root"
    fi
    PACKAGE_WORK_ROOT="$(package_normalize_path "$root")"
}

# A relative LXB_INSTALLER_WORK_DIR has to become absolute here rather than at
# first use: the guard in package_work_dir asks whether the work root is inside
# the checkout, and a relative path would answer no to that question however far
# inside it actually is.
if [[ -n "${LXB_INSTALLER_WORK_DIR:-}" ]]; then
    package_set_work_root "$LXB_INSTALLER_WORK_DIR"
fi

# A private directory under the work root, named so the cleanup traps can
# recognise their own before removing anything.
package_work_dir() {
    local slug="$1"

    # `snapshot_source` takes tracked files *plus* anything untracked and not
    # ignored, which is what lets a local build package the tree a developer is
    # actually testing. A build directory inside the checkout and outside
    # packaging/out — which is both ignored and skipped by name — would be
    # swept up by that: a source archive containing its own build tree, growing
    # each time it is built.
    # `out"*` rather than `out"/*` would also accept packaging/outtakes, which
    # snapshot_source skips by exact name and would therefore sweep up.
    if [[ "$PACKAGE_WORK_ROOT" == "$PROJECT_ROOT"/* \
        && "$PACKAGE_WORK_ROOT" != "$PACKAGING_DIR/out" \
        && "$PACKAGE_WORK_ROOT" != "$PACKAGING_DIR/out"/* ]]; then
        package_die "the build directory must be outside the checkout or under packaging/out: $PACKAGE_WORK_ROOT"
    fi

    mkdir -p "$PACKAGE_WORK_ROOT" \
        || package_die "could not create the build directory $PACKAGE_WORK_ROOT"
    mktemp -d "$PACKAGE_WORK_ROOT/$slug.XXXXXX" \
        || package_die "could not create a build directory under $PACKAGE_WORK_ROOT"
}

# Refuse a build that cannot finish, while it still costs nothing to refuse.
#
# `df` reports what the filesystem has, which is not always what this user may
# take: a quota is invisible here and will still stop the build. So this catches
# the common case and never promises the uncommon one.
require_free_space() {
    local directory="$1"
    local needed_mib="$2"
    local available

    available="$(df -Pm "$directory" 2>/dev/null | awk 'NR == 2 { print $4 }')"
    if [[ ! "$available" =~ ^[0-9]+$ ]]; then
        # Not knowing is not a reason to refuse; the build will say so itself.
        return 0
    fi
    if ((available < needed_mib)); then
        package_die "$directory has ${available} MiB free but the build needs about ${needed_mib} MiB.
Point the build somewhere with room using --work-dir DIR (or LXB_INSTALLER_WORK_DIR),
or skip the test phase, which is most of that space."
    fi
}

package_source_date_epoch() {
    if [[ -n "${SOURCE_DATE_EPOCH:-}" ]]; then
        [[ "$SOURCE_DATE_EPOCH" =~ ^[0-9]+$ ]] \
            || package_die "SOURCE_DATE_EPOCH must be an integer"
        printf '%s\n' "$SOURCE_DATE_EPOCH"
        return
    fi

    git -C "$PROJECT_ROOT" log -1 --format=%ct HEAD 2>/dev/null || date +%s
}

snapshot_source() {
    local destination="$1"

    [[ "$destination" == /* ]] || package_die "snapshot destination must be absolute"

    # The file list below comes from Git, so a tree Git cannot read is not one
    # this can package — and it has to be refused here, because nothing
    # downstream will refuse it. An empty file list makes `tar -T -` write an
    # empty archive and report success, and the empty source directory that
    # unpacks from it sits *below* the checkout it was meant to be a copy of.
    # So Cargo walks up out of it, finds the very workspace it should have been
    # handed, and vendors and builds that instead. Every phase that asks Cargo a
    # question passes, and the first one to name a path of its own — %install
    # reaching for packaging/install.sh — is where it comes apart, several
    # gigabytes and one whole compile later. That is not a story: it is what
    # happened to LineXinBar, the installer's sibling, on an unpacked source
    # download.
    git -C "$PROJECT_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
        || package_die "$PROJECT_ROOT is not a Git checkout, and the source archive is built from what Git lists.
Clone the repository instead of unpacking a source download, or make this tree
one with: git init && git add -A && git commit -m 'local tree'"
    mkdir -p "$destination"
    if [[ -n "$(find "$destination" -mindepth 1 -print -quit)" ]]; then
        package_die "snapshot destination is not empty: $destination"
    fi

    # Use tracked files plus non-ignored working-tree additions, rather than
    # git archive, so an intentional local build contains the exact sources
    # the developer is testing. Asking Git for the list also keeps ignored
    # build output, editor state and local .env files out of source packages.
    while IFS= read -r -d '' path; do
        case "$path" in
            packaging/out | packaging/out/*) continue ;;
        esac
        if [[ -e "$PROJECT_ROOT/$path" || -L "$PROJECT_ROOT/$path" ]]; then
            printf '%s\0' "$path"
        fi
    done < <(git -C "$PROJECT_ROOT" ls-files -z --cached --others --exclude-standard) \
        | tar --null --no-recursion -C "$PROJECT_ROOT" -T - -cf - \
        | tar -C "$destination" -xf -

    # And say so if it did not arrive. The pipeline above reports success for an
    # empty archive, and an empty snapshot is the one failure that goes on to
    # look like a working build.
    for required in Cargo.toml packaging/install.sh; do
        [[ -f "$destination/$required" ]] \
            || package_die "the source snapshot is missing $required: $destination"
    done
}

archive_snapshot() {
    local source_dir="$1"
    local output="$2"
    local epoch
    local source_parent
    local source_name

    [[ -d "$source_dir" ]] || package_die "snapshot directory does not exist: $source_dir"
    [[ "$output" == /* ]] || package_die "archive output path must be absolute"
    [[ ! -e "$output" ]] || package_die "refusing to overwrite archive: $output"

    epoch="$(package_source_date_epoch)"
    source_parent="$(dirname "$source_dir")"
    source_name="$(basename "$source_dir")"
    mkdir -p "$(dirname "$output")"

    tar --sort=name \
        --mtime="@$epoch" \
        --owner=0 --group=0 --numeric-owner \
        -C "$source_parent" -cf - "$source_name" | gzip -n > "$output"
}
