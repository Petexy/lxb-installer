#!/bin/sh
# Cargo, with the installer built against an lxb-toolkit source tree instead
# of the installed toolkit — for tests and pictures of toolkit work that has
# not been released yet:
#
#   scripts/toolkit-tree.sh test --locked
#   scripts/toolkit-tree.sh run --locked -- --shot shot.png --scene target
#
# Cargo.toml keeps naming the installed toolkit (/usr/share/lxb-toolkit),
# which is what the package builds against, and nothing is installed.
# Instead a manifest of its own is written under target/: the installer's
# Cargo.toml with lxb-app's path pointed at the tree, its Cargo.lock, and
# the sources themselves linked in — so it builds the very files being
# edited, and Cargo.toml and Cargo.lock in the repository never change.
# (Cargo's `paths` overrides would do it without a manifest, but
# cargo warns that they do not support a toolkit whose crates point at each
# other by other paths, and means to refuse it.) The build goes to a target
# directory of its own, so switching between the two does not rebuild either.
#
# The tree is LXB_TOOLKIT_TREE when that is set, and otherwise lxb-toolkit
# beside this checkout: beside the main checkout when this is one of its git
# worktrees (git names the main checkout's .git as the common one), so every
# worktree finds the same tree wherever it was made; beside this folder when
# it is no git checkout of its own (a copy of the files, or one inside some
# other repository).
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
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
installed=/usr/share/lxb-toolkit/crates/lxb-app

if [ ! -f "$tree/crates/lxb-app/Cargo.toml" ]; then
    echo "toolkit-tree: no lxb-app in $tree (set LXB_TOOLKIT_TREE)" >&2
    exit 1
fi
if ! grep -q "path = \"$installed\"" "$here/Cargo.toml"; then
    echo "toolkit-tree: Cargo.toml no longer names $installed" >&2
    exit 1
fi

manifest=$here/target/toolkit-tree-manifest
mkdir -p "$manifest"
sed "s|path = \"$installed\"|path = \"$tree/crates/lxb-app\"|" "$here/Cargo.toml" \
    >"$manifest/Cargo.toml.new"
mv "$manifest/Cargo.toml.new" "$manifest/Cargo.toml"
cmp -s "$here/Cargo.lock" "$manifest/Cargo.lock" || cp "$here/Cargo.lock" "$manifest/Cargo.lock"
for part in src locales data docs packaging artifacts; do
    if [ -e "$here/$part" ]; then
        ln -sfn "$here/$part" "$manifest/$part"
    fi
done

# The manifest goes straight after the subcommand, before anything meant for
# the program itself (`run -- --shot …`).
command=${1:?toolkit-tree: which cargo command?}
shift
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$here/target/toolkit-tree} \
    exec cargo "$command" --manifest-path "$manifest/Cargo.toml" "$@"
