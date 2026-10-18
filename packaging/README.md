# Linexin Installer packaging

These definitions build the installer into an Arch package, so that a Linexin
live system can carry it and start it. One package comes out of one source
tree:

```text
lxb-installer   usr/bin/lxb-installer
                usr/share/applications/lxb-installer.desktop
                usr/share/icons/hicolor/scalable/apps/lxb-installer.svg
                usr/share/metainfo/io.github.petexy.lxb-installer.metainfo.xml
                usr/share/polkit-1/actions/io.github.petexy.lxb-installer.policy
```

The PKGBUILD calls `packaging/install.sh` rather than listing those five files
itself, so adding one file is one edit. What install.sh does *not* place is the
licence and the README; the PKGBUILD puts those where Arch wants them.

## Arch only

The family's other applications are packaged for Arch, Debian, Fedora and Nix.
This one is packaged for **Arch alone**, and that is a decision, not a gap:
the installer puts Linexin — an Arch-based system — on a computer, it runs from
Linexin's live system, and what it does is Arch's (`pacman`, `pacman-key`,
`mkinitcpio`, `arch-chroot`, the archiso layout of the live medium). A `.deb`
or an `.rpm` of it would install on a system it has nothing to do there. So
there is `packaging/arch` and the shared scripts, and no `debian/`, `fedora/`,
`nix/` or `flake.nix`.

## The toolkit is a build dependency, not a runtime one

`lxb-app` is a Rust **path** dependency:

```toml
lxb-app = { version = "0.9.4", path = "/usr/share/lxb-toolkit/crates/lxb-app" }
```

Cargo compiles those sources into this binary. The finished program links no
`liblxb_*.so` at all — `ldd` on it names libasound, libxkbcommon, libudev,
libgcc_s and libc — so a live system that *runs* the installer needs nothing of
lxb-toolkit installed, and a machine that *builds* it needs the crate sources
that the `lxb-toolkit` package puts under `/usr/share/lxb-toolkit/crates`.
Hence `makedepends`, and nothing of the toolkit in `depends`.

The version is asked of `Cargo.toml` rather than repeated: the PKGBUILD is
rendered from `PKGBUILD.in` and carries `@TOOLKIT_VERSION@`.

The installer needs lxb-toolkit 0.9.4: it lights what the pointer hovers through
`Page::hovered`, changes its pages and photographs them changing through
`Ui::carry_between`, `Page::low_end` and `App::shot_after`, needs a toolkit whose
lights stand on what is chosen on low-end hardware (`Ui::set_low_end`; before it
the erase question showed "Go back" lit there while Enter erased), photographs
low-end hardware and a question being answered through `App::low_end` and
`Page::act`, brings Options back to the option just changed through
`Page::menu_on`, draws its own marks, the tick and the switch through
`App::add_mark`, `Ui::chosen` and `Ui::switch` beside the shell's account marks,
and keeps a field clear of LineXinBar's on-screen keyboard through `board::rect`,
`Page::taking_keys` and `Page::shells_board` — none of which a released 0.9.3
has. `Cargo.toml` asks for 0.9.4, so Cargo refuses an older toolkit by its
version; `require_toolkit_sources` (`lib.sh`) still names each call in words for
a toolkit tree built between the two. A build container with no lxb-toolkit
0.9.4 package to install has one built from the toolkit's own repository first
(its `packaging/build.sh arch`).

## What it depends on at run time

Two kinds of dependency, both real.

**The window.** libxkbcommon, alsa-lib and systemd-libs are the keyboard, the
interface sounds and the controllers, linked outright. The Wayland client
libraries and `libEGL` are loaded by name at run time, so nothing that reads the
ELF will ever name them: `wayland` and `libglvnd` are listed by hand. Vulkan is
suggested rather than required, because without a driver wgpu falls back to
EGL. `libgcc` is Rust's unwinder, `hicolor-icon-theme` owns the directories
the icon goes in, and `polkit` is `pkexec`, which raises the worker and the
read-only measurement.

**The worker.** Every program it runs, on the live system or inside the new
system through `arch-chroot` — and the new system is a copy of the live one,
so the live system has to carry both — and the few the read-only measurement
(`--measure`) runs on the live system. That is util-linux, systemd,
e2fsprogs, dosfstools, ntfsprogs, btrfs-progs, xfsprogs, efibootmgr,
arch-install-scripts, rsync, coreutils, pciutils, shadow, sudo, pacman, gnupg,
mkinitcpio, flatpak, bash, and **GRUB and rEFInd**: the boot loader is chosen
by the disk (GRUB for BIOS and msdos tables, rEFInd beside another system),
and with no internet the worker can only take it from the live system.
os-prober, NetworkManager, xdg-user-dirs, terminus-font and dconf are
optional: the worker looks for each before it uses it and goes on without it.

`build.sh check` reads the worker's source for every program it names — each
`run::run` (and its `_input`, `_streaming` and `_status` forms), `run::chroot`,
`command_path`, `Command::new` and absolute
`/usr/bin/…`, plus the worker's own `required_tools()` and the image's
`TARGET_PROGRAMS` — and refuses unless each has a row in its table of
program → package and that package is in `depends` or `optdepends`. A program
missing from the live system would otherwise be found when an installation
reached the step that runs it, on somebody's computer, half done.

## Built for every x86-64, never for the builder

The package has to run on every computer Linexin is installed on. A build tuned
for the building machine's CPU starts there and dies with an illegal
instruction on an older one, before `main`, even on `--version` — which is what
happens to anything built on a CachyOS host, whose `makepkg.conf` says
`-march=native` and `target-cpu=native`. `arch/build.sh` reads makepkg's own
configuration and the environment, and refuses such a build. Build in a plain
Arch container instead:

```sh
distrobox create -n arch-build -i docker.io/library/archlinux:latest
distrobox enter arch-build -- sudo pacman -S --needed base-devel rust git
# install lxb-toolkit in the box (its own packaging/build.sh arch), then:
distrobox enter arch-build -- ./packaging/build.sh arch -- --syncdeps
```

To prove the result rather than assume it, run the binary on an emulated
baseline CPU (in the box, whose glibc is generic):

```sh
bsdtar -xOf packaging/out/arch/lxb-installer-*-x86_64.pkg.tar.zst usr/bin/lxb-installer > packaging/out/lxb-installer
chmod +x packaging/out/lxb-installer
qemu-x86_64-static -cpu qemu64,-sse3,-ssse3,-sse4.1,-sse4.2,-popcnt,-avx,-avx2,-aes,-pclmuldq,-cx16 \
    packaging/out/lxb-installer --version
readelf -n packaging/out/lxb-installer | sed -n 's/.*x86 ISA needed: //p'   # x86-64-baseline, or nothing
```

## One version, in one file

The version is the single line in `VERSION` at the root of the checkout, shared
with LineXinBar, lxb-toolkit and the family's other applications. The PKGBUILD
and the source archive's name read it there. Two places cannot read a file and
carry it as a literal — `[package]` in `Cargo.toml`, which is where `--version`
gets it, and the AppStream release list — and `build.sh check` refuses to let
any of them, or `Cargo.lock`, drift apart.

## Where the build happens, and why not /tmp

Every builder works under `packaging/out/build/`, on whatever filesystem the
checkout is on — not `${TMPDIR:-/tmp}`, which on a systemd machine is a tmpfs
in RAM. The locked graph is nearly 400 crates with wgpu and naga among them,
and makepkg's `check()` builds the whole of it a second time in the dev
profile. Send it elsewhere with `--work-dir DIR` or `LXB_INSTALLER_WORK_DIR`:

```sh
./packaging/build.sh arch --work-dir /var/tmp/lxb-installer
```

A work directory inside the checkout is refused unless it is under
`packaging/out`, because the source snapshot picks up untracked files and a
build tree anywhere else would end up inside the archive built from it.
`makepkg.conf` wins over the environment, so a machine that sets `BUILDDIR`
builds there whatever `--work-dir` said; the builder notices and says so.

## Validate the definition and its payload

```sh
./packaging/build.sh check
./packaging/build.sh check --no-build   # when a current release binary exists
```

This checks shell syntax; that `VERSION`, `Cargo.toml`, `Cargo.lock` and the
AppStream release agree; that the PKGBUILD still takes its version and toolkit
requirement from them; that every program the worker runs is a declared
dependency; that the desktop entry, the AppStream component and the window
title call the application the same thing; that both polkit actions'
`exec.path`, the program `src/launch.rs` hands pkexec and the installed binary
are all `/usr/bin/lxb-installer`, and that each action's `exec.argv1` is its
own flag (`--worker`, `--measure`); then it builds the release binary, confirms
it reports the packaged version, stages the payload, proves nothing in `data/`
has stopped shipping, proves the desktop entry's `Exec` and `Icon` name things
the package installs, and hands the metadata to `desktop-file-validate` and
`appstreamcli`.

## Arch Linux

```sh
./packaging/build.sh arch
./packaging/build.sh arch -- --syncdeps
./packaging/build.sh arch -- --nocheck
./packaging/build.sh arch -- --nodeps     # in a build container, see below
```

makepkg wants a package's run-time dependencies installed before it builds
it. Here those are the live system's tools — GRUB, rEFInd, mkinitcpio,
flatpak and the rest — which a build container has no use for, and flatpak
in a distrobox collides with the shim distrobox puts at `/usr/bin/flatpak`.
`--nodeps` skips that check; the build dependencies (`cargo`, `pkgconf`,
`lxb-toolkit`) then have to be installed already, and
`pacman -T cargo pkgconf lxb-toolkit` says whether they are.

`namcap` on the result reports the worker's packages as "may not be needed":
it reads only the ELF, which links none of them. That is expected; the
program-to-package check in `build.sh check` is what says they are needed.

The wrapper creates a deterministic source archive and renders a PKGBUILD with
its real SHA-256 checksum before running `makepkg`. Artifacts are copied to
`packaging/out/arch/`. Set `PACKAGER` for a build that leaves this machine:
`Unknown Packager` in `pacman -Qi` is how a stray build is recognised.

`./packaging/source-archive.sh [FILE]` writes the same source archive on its
own, to `packaging/out/sources/` by default.

## After installing

```sh
lxb-installer                 # opens the installer (from a Linexin live system)
lxb-installer --demo          # a made-up computer; nothing is written
lxb-installer --version
pkaction --action-id io.github.petexy.lxb-installer.install --verbose
pkaction --action-id io.github.petexy.lxb-installer.measure --verbose
```

The polkit actions are what pkexec asks for when the interface raises the
worker (install) or the read-only measurement (measure); `pkaction` shows each
was found, that its `exec.path` is `/usr/bin/lxb-installer` and that its
`exec.argv1` is `--worker` or `--measure`.
