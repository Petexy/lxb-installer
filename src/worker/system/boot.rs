//! Installing the boot loader.
//!
//! Which one is the policy of INSTALLER-HANDOFF.md §2: GRUB on BIOS or on a
//! DOS-partitioned disk, systemd-boot (no menu) when Linexin is alone,
//! rEFInd when another system is found, or what the person chose under
//! Advanced when this machine can start it ([`decide`]).
//!
//! Every loader is installed by its own tool inside the target, told never
//! to touch the firmware's boot entries itself (`bootctl --variables=no`,
//! `grub-install --no-nvram`; rEFInd's files are copied, as the legacy
//! installer did). The entry is then made once, here, from the live system
//! with `efibootmgr`, on the disk storage named and the partition number
//! the kernel gives for the EFI partition. That is the same whatever the
//! loader, does not depend on whether EFI variables are reachable inside
//! arch-chroot, and never works a disk out by cutting digits off a name
//! (which turns `nvme0n1p1` into `nvme0n1p`).
//!
//! The kernels are always on a FAT partition mounted at `/boot`: the EFI
//! partition itself, or beside a shared EFI partition too small for them
//! (mounted at `/efi`) a Linux boot partition the installer made. Every
//! loader then reads them through the firmware's own FAT support, without
//! a filesystem driver of its own.
//!
//! On an EFI partition the installer made, the loader is also copied to the
//! removable-media path `EFI/BOOT/BOOTX64.EFI`, which firmware starts when
//! it has no entry; a firmware that refuses the entry then still starts
//! Linexin, so that refusal is only a warning. On a shared partition
//! nothing outside the chosen loader's own directory is created, changed or
//! deleted, the fallback included, so a refused entry there stops: nothing
//! else would start the system.
use super::{packages, Install};
use crate::model::config::{Bootloader, Filesystem, Invalid};
use crate::model::disks::Table;
use crate::model::protocol::{Failure, Step, Warning};
use crate::worker::layout::{Esp, EspMount, Firmware, Mounted, Stop};
use crate::worker::{run, target};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// The boot loader this installation gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loader {
    SystemdBoot,
    Refind,
    Grub,
}

/// Directories under `EFI/` on an EFI partition that are not another
/// system: the removable-media fallback, the loaders this installer puts
/// there itself (or an earlier Linexin did), and the shared directory of
/// unified kernel images.
pub const OWN_EFI_DIRS: &[&str] = &[
    "BOOT", "Linexin", "systemd", "refind", "Linux", "grub", "arch",
];

/// The label of the firmware entry for systemd-boot and GRUB.
pub const LABEL: &str = "Linexin";

/// How many parts the step reports progress in.
const PARTS: u64 = 3;

/// Picks the loader. `esp` is where the EFI partition is mounted (`None` on
/// BIOS), `other_os` whether another system was found. Where the EFI
/// partition is mounted no longer matters: beside one at `/efi` the kernels
/// are on a Linux boot partition, which systemd-boot reads as well.
pub fn decide(
    firmware: Firmware,
    table: Table,
    esp: Option<&Esp>,
    other_os: bool,
    choice: Bootloader,
) -> Result<Loader, Invalid> {
    let unavailable = Err(Invalid::BootloaderUnavailable);
    match firmware {
        Firmware::Bios => match choice {
            Bootloader::Automatic | Bootloader::Grub => Ok(Loader::Grub),
            Bootloader::SystemdBoot | Bootloader::Refind => unavailable,
        },
        Firmware::Uefi => {
            if esp.is_none() {
                return unavailable;
            }
            match choice {
                Bootloader::Grub => Ok(Loader::Grub),
                Bootloader::Refind => Ok(Loader::Refind),
                Bootloader::SystemdBoot if table == Table::Gpt => Ok(Loader::SystemdBoot),
                Bootloader::SystemdBoot => unavailable,
                Bootloader::Automatic if table == Table::Msdos => Ok(Loader::Grub),
                Bootloader::Automatic if other_os => Ok(Loader::Refind),
                Bootloader::Automatic => Ok(Loader::SystemdBoot),
            }
        }
    }
}

/// Whether finding another system changes anything for `choice`: it picks
/// rEFInd for the automatic choice, and it makes GRUB list that system.
pub fn other_os_matters(choice: Bootloader) -> bool {
    matches!(choice, Bootloader::Automatic | Bootloader::Grub)
}

/// `Step::Bootloader`.
pub fn run(install: &mut Install) -> Result<(), Stop> {
    let mounted = install.mounted;
    let choice = install.config.bootloader;
    let other_os = other_os_matters(choice) && other_os(mounted);
    let loader = decide(
        mounted.firmware,
        mounted.table,
        mounted.esp.as_ref(),
        other_os,
        choice,
    )
    .map_err(|invalid| {
        Stop::new(
            Failure::Internal,
            format!(
                "{choice:?} on {:?} with {:?}: {invalid:?}",
                mounted.firmware, mounted.table
            ),
        )
    })?;
    run::log(&format!(
        "bootloader: {loader:?} (asked for {choice:?}, another system: {other_os})"
    ));
    install.progress(Step::Bootloader, 1, PARTS);
    match (loader, mounted.esp.as_ref()) {
        (Loader::SystemdBoot, Some(esp)) => systemd_boot(install, esp),
        (Loader::Refind, Some(esp)) => refind(install, esp),
        (Loader::Grub, esp) => grub(install, esp, other_os),
        (_, None) => Err(Stop::new(
            Failure::Internal,
            "an EFI boot loader without an EFI partition",
        )),
    }
}

/// The EFI partition's mount point inside the target, as a relative path.
fn esp_dir(esp: &Esp) -> &'static str {
    esp.mount.path().trim_start_matches('/')
}

/// Whether `/boot` is a partition this installation owns: the EFI
/// partition it made, or the Linux boot partition beside a shared one. Only
/// a shared EFI partition mounted at `/boot` is someone else's too.
pub fn boot_is_ours(esp: &Esp) -> bool {
    !(esp.shared && esp.mount == EspMount::Boot)
}

/// The Linux boot partition is there exactly when the EFI partition is
/// mounted at `/efi`, since only it can hold the kernels then.
fn check_kernel_partition(mounted: &Mounted, esp: &Esp) -> Result<(), Stop> {
    if (esp.mount == EspMount::Efi) == mounted.linux_boot.is_some() {
        return Ok(());
    }
    Err(Stop::new(
        Failure::Internal,
        format!(
            "the EFI partition is at {} and the Linux boot partition is {:?}",
            esp.mount.path(),
            mounted.linux_boot
        ),
    ))
}

// ---------------------------------------------------------------------------
// Other systems

/// Whether another system is on this machine: a directory of its own on the
/// EFI partition, or anything `os-prober` (when the live system has it)
/// finds that is not this installation.
fn other_os(mounted: &Mounted) -> bool {
    let mut found = Vec::new();
    if let Some(esp) = &mounted.esp {
        let names =
            target::list(&mounted.root, &format!("{}/EFI", esp_dir(esp))).unwrap_or_default();
        found.extend(other_systems_on_esp(&names));
    }
    if run::command_path("os-prober").is_ok() {
        let own = own_devices(mounted);
        match run::run("timeout", &["300", "os-prober"]) {
            Ok(output) => found.extend(os_prober_others(&output, &own)),
            Err(stop) => run::log(&format!("bootloader: os-prober: {}", stop.detail)),
        }
    }
    run::log(&format!("bootloader: other systems found: {found:?}"));
    !found.is_empty()
}

/// The partitions that are this installation's own, which os-prober finds
/// no other system on: the root, a /home of its own, the EFI and the Linux
/// boot partition — and, for a layout made by hand, every partition it
/// formats or makes and a /home it keeps (`Mounted::other_devices`, empty
/// on the easy path, whose list is as it was).
pub fn own_devices(mounted: &Mounted) -> Vec<&str> {
    let mut own = vec![mounted.root_device.as_str()];
    own.extend(mounted.home_device.as_deref());
    own.extend(mounted.esp.as_ref().map(|esp| esp.device.as_str()));
    own.extend(mounted.linux_boot.as_ref().map(|boot| boot.device.as_str()));
    own.extend(mounted.other_devices.iter().map(String::as_str));
    own
}

fn own_efi_dir(name: &str) -> bool {
    OWN_EFI_DIRS
        .iter()
        .any(|own| own.eq_ignore_ascii_case(name))
}

/// The directories in an EFI partition's `EFI/` that belong to another
/// system (`Microsoft`, `ubuntu`, `fedora`…). FAT ignores case, so the
/// comparison does too.
pub fn other_systems_on_esp(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !own_efi_dir(name))
        .cloned()
        .collect()
}

/// The systems in `os-prober` output that are not this installation. Each
/// line is `device:long name:short name:type`; a loader on an EFI partition
/// is `device@/EFI/<dir>/<file>`, and counts unless `<dir>` is one of
/// [`OWN_EFI_DIRS`]. A system on one of `own` (this installation's root,
/// home or EFI partition) is this installation.
pub fn os_prober_others(output: &str, own: &[&str]) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            let location = line.split(':').next().unwrap_or_default();
            match location.split_once('@') {
                Some((_, path)) => {
                    let mut parts = path.split('/').filter(|p| !p.is_empty());
                    let in_efi = parts.next().is_some_and(|p| p.eq_ignore_ascii_case("efi"));
                    !(in_efi && parts.next().is_some_and(own_efi_dir))
                }
                None => !own.contains(&location),
            }
        })
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------------------
// The kernel command line

/// What every entry passes the kernel to find the root: by UUID, read-write,
/// and on Btrfs the subvolume that is `/`.
pub fn kernel_options(root_uuid: &str, subvolume: Option<&str>) -> String {
    let mut options = format!("root=UUID={root_uuid} rw");
    if let Some(subvolume) = subvolume {
        options.push_str(&format!(" rootflags=subvol={subvolume}"));
    }
    options
}

/// What tells systemd not to switch on or mount, at the first start, a
/// partition it finds by its GPT type alone (an old system's swap or home a
/// layout made by hand leaves unused): only such a layout's entries carry
/// it (`Mounted::gpt_auto_off`).
pub const GPT_AUTO_OFF: &str = "systemd.gpt_auto=0";

/// The options every entry of this installation passes the kernel: the
/// root ([`kernel_options`]), and [`GPT_AUTO_OFF`] after it for a layout
/// made by hand. The easy path's are exactly the root's, as they were.
fn options_for(mounted: &Mounted) -> String {
    let subvolume = match mounted.filesystem {
        Filesystem::Btrfs => mounted.root_subvolume.as_deref(),
        Filesystem::Ext4 => None,
    };
    let mut options = kernel_options(&mounted.root_uuid, subvolume);
    if mounted.gpt_auto_off {
        options.push(' ');
        options.push_str(GPT_AUTO_OFF);
    }
    options
}

// ---------------------------------------------------------------------------
// systemd-boot

/// `loader.conf`: straight into Linexin with no menu (a key held at start
/// still shows it), the largest console, no editing of the command line.
pub const LOADER_CONF: &str = "default  linexin.conf\ntimeout  0\nconsole-mode max\neditor   no\n";

/// A systemd-boot entry. The kernel and its images are at the top of the
/// partition mounted at `/boot` (the EFI partition or a Linux boot
/// partition), which is where the entry is too, so the paths are its own.
pub fn entry(title: &str, pkgbase: &str, image: &str, options: &str) -> String {
    format!("title   {title}\nlinux   /vmlinuz-{pkgbase}\ninitrd  /{image}\noptions {options}\n")
}

/// `linexin.conf` (quiet, with the splash) and `linexin-fallback.conf`
/// (the fallback image, every message shown). `linexin-snapshot` reads the
/// first as the template of its recovery entries.
pub fn systemd_boot_entries(pkgbase: &str, options: &str) -> (String, String) {
    (
        entry(
            "Linexin",
            pkgbase,
            &format!("initramfs-{pkgbase}.img"),
            &format!("{options} quiet splash"),
        ),
        entry(
            "Linexin (fallback)",
            pkgbase,
            &format!("initramfs-{pkgbase}-fallback.img"),
            options,
        ),
    )
}

/// bootctl's arguments: the EFI partition where it is mounted, and the
/// Linux boot partition at `/boot` when the EFI partition is at `/efi`.
/// bootctl leaves the firmware's entries alone, and makes no directory per
/// machine for entries: they are written here, by name.
pub fn bootctl_args(mount: EspMount) -> Vec<String> {
    let mut args = vec![format!("--esp-path={}", mount.path())];
    if mount == EspMount::Efi {
        args.push("--boot-path=/boot".to_string());
    }
    args.extend(["--variables=no", "--make-entry-directory=no", "install"].map(String::from));
    args
}

/// Where bootctl puts systemd-boot's own loader, relative to the target's
/// root: on the EFI partition, wherever it is mounted — `boot/…` for one at
/// `/boot`, `efi/…` beside a Linux boot partition at `/boot`. The fallback
/// loader is copied from it.
fn systemd_boot_loader(esp: &Esp) -> String {
    format!("{}/EFI/systemd/systemd-bootx64.efi", esp_dir(esp))
}

fn systemd_boot(install: &mut Install, esp: &Esp) -> Result<(), Stop> {
    check_kernel_partition(install.mounted, esp)?;
    let root = install.root().to_path_buf();
    let pkgbase = install.image.kernel.pkgbase.clone();
    let kernel = format!("boot/vmlinuz-{pkgbase}");
    let image = format!("boot/initramfs-{pkgbase}.img");
    for file in [&kernel, &image] {
        if !target::is_file(&root, file) {
            return Err(Stop::new(
                Failure::CommandFailed,
                format!("/{file} is not in /boot"),
            ));
        }
    }
    // bootctl always writes EFI/BOOT/BOOTX64.EFI as well; on a shared
    // partition that file is another system's, so it is put back.
    let kept = if esp.shared {
        Some(Preserved::take(&root, esp_dir(esp))?)
    } else {
        None
    };
    let args = bootctl_args(esp.mount);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let installed = run::chroot(&root, "bootctl", &args);
    if let Some(kept) = kept {
        kept.restore(&root)?;
    }
    installed?;
    install.progress(Step::Bootloader, 2, PARTS);

    let options = options_for(install.mounted);
    let (main, fallback) = systemd_boot_entries(&pkgbase, &options);
    // systemd-boot reads its settings from the EFI partition only, and the
    // entries from both partitions; they go beside the kernels.
    let loader = format!("{}/loader", esp_dir(esp));
    target::create_dir(&root, &loader)?;
    target::write(&root, &format!("{loader}/loader.conf"), LOADER_CONF, 0o644)?;
    target::create_dir(&root, "boot/loader/entries")?;
    target::write(&root, "boot/loader/entries/linexin.conf", &main, 0o644)?;
    if target::is_file(&root, &format!("boot/initramfs-{pkgbase}-fallback.img")) {
        target::write(
            &root,
            "boot/loader/entries/linexin-fallback.conf",
            &fallback,
            0o644,
        )?;
    } else {
        run::log("bootloader: no fallback image, so no fallback entry");
    }
    if !esp.shared {
        install_fallback(&root, esp, &systemd_boot_loader(esp))?;
    }
    register(install, esp, LABEL, r"\EFI\systemd\systemd-bootx64.efi")
}

/// The removable-media fallback directory of an EFI partition as it was
/// before a tool that writes there ran, so that it can be put back exactly.
struct Preserved {
    esp: String,
    /// The directory's name as it is spelt on the partition, if it existed.
    dir: Option<String>,
    files: Vec<(String, Vec<u8>)>,
}

fn fallback_dir_name(root: &Path, esp: &str) -> Result<Option<String>, Stop> {
    Ok(target::list(root, &format!("{esp}/EFI"))?
        .into_iter()
        .find(|name| name.eq_ignore_ascii_case("BOOT")))
}

impl Preserved {
    fn take(root: &Path, esp: &str) -> Result<Self, Stop> {
        let dir = fallback_dir_name(root, esp)?;
        let mut files = Vec::new();
        if let Some(dir) = &dir {
            for name in target::list(root, &format!("{esp}/EFI/{dir}"))? {
                let relative = format!("{esp}/EFI/{dir}/{name}");
                if target::is_file(root, &relative) {
                    let bytes = std::fs::read(target::path(root, &relative)?).map_err(|e| {
                        Stop::new(Failure::Internal, format!("cannot read /{relative}: {e}"))
                    })?;
                    files.push((name, bytes));
                }
            }
        }
        Ok(Self {
            esp: esp.to_string(),
            dir,
            files,
        })
    }

    fn restore(self, root: &Path) -> Result<(), Stop> {
        let now = fallback_dir_name(root, &self.esp)?;
        let Some(dir) = self.dir else {
            if let Some(made) = now {
                run::log("bootloader: removing the EFI/BOOT the loader's tool made");
                target::remove(root, &format!("{}/EFI/{made}", self.esp))?;
            }
            return Ok(());
        };
        let dir = format!("{}/EFI/{}", self.esp, now.unwrap_or(dir));
        target::create_dir(root, &dir)?;
        for name in target::list(root, &dir)? {
            let relative = format!("{dir}/{name}");
            let before = self
                .files
                .iter()
                .find(|(kept, _)| kept.eq_ignore_ascii_case(&name));
            if before.is_none() && target::is_file(root, &relative) {
                target::remove(root, &relative)?;
            }
        }
        for (name, bytes) in &self.files {
            let relative = format!("{dir}/{name}");
            let current = std::fs::read(target::path(root, &relative)?).ok();
            if current.as_deref() != Some(bytes.as_slice()) {
                run::log(&format!("bootloader: /{relative} put back as it was"));
                write_bytes(root, &relative, bytes)?;
            }
        }
        Ok(())
    }
}

/// Replaces `root/relative` with `bytes`, never through a symlink.
fn write_bytes(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), Stop> {
    target::remove(root, relative)?;
    let path = target::path(root, relative)?;
    let internal = |e: std::io::Error| {
        Stop::new(
            Failure::Internal,
            format!("cannot write {}: {e}", path.display()),
        )
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(internal)?;
    file.write_all(bytes).map_err(internal)?;
    file.sync_all().map_err(internal)
}

// ---------------------------------------------------------------------------
// rEFInd

/// The parts of `/usr/share/refind` the loader uses: icons, fonts and the
/// theme. No filesystem driver: the kernels are on a FAT partition, which
/// the firmware reads itself (rEFInd 0.14.2's ext4 driver loaded but never
/// attached under OVMF, so kernels on the root were never found).
pub const REFIND_PARTS: &[&str] = &["icons", "fonts", "themes"];
pub const REFIND_THEME: &str = "themes/refind-theme-regular";

/// `refind.conf`, the legacy installer's, with the kernel's own name and,
/// on Btrfs, the subvolume's `/boot` among the places to look.
pub fn refind_conf(pkgbase: &str, btrfs: bool, theme: bool) -> String {
    let mut text = String::new();
    if theme {
        text.push_str("include themes/refind-theme-regular/theme.conf\n\n");
    }
    let scan = if btrfs { "boot,@/boot" } else { "boot" };
    text.push_str(&format!(
        "timeout 5
hideui singleuser,hints,arrows,badges
big_icon_size 128
small_icon_size 48
default_selection \"vmlinuz-{pkgbase}\"
scan_all_linux_kernels true
fold_linux_kernels true
windows_recovery_files LRS_ESP:/EFI/Microsoft/Boot/bootmgfw.efi
scanfor manual,internal,external,optical
dont_scan_dirs ESP:/EFI/boot,ESP:/EFI/Boot
extra_kernel_version_strings linux-lts,linux
also_scan_dirs {scan},EFI/manjaro,EFI/ubuntu,EFI/debian,EFI/fedora
"
    ));
    text
}

/// `refind_linux.conf`, next to the kernel: the legacy installer's three
/// ways to start.
pub fn refind_linux_conf(options: &str) -> String {
    format!(
        "\"Boot Linexin\"                 \"{options} quiet splash\"\n\
         \"Boot Linexin (terminal)\"      \"{options} systemd.unit=multi-user.target\"\n\
         \"Boot to single-user mode\"     \"{options} single\"\n"
    )
}

fn refind(install: &mut Install, esp: &Esp) -> Result<(), Stop> {
    check_kernel_partition(install.mounted, esp)?;
    ensure_package(install, "refind", "usr/share/refind/refind_x64.efi")?;
    let root = install.root().to_path_buf();
    let pkgbase = install.image.kernel.pkgbase.clone();
    let source = "usr/share/refind";
    let dir = format!("{}/EFI/refind", esp_dir(esp));
    target::create_dir(&root, &dir)?;
    target::copy_file(
        &target::path(&root, &format!("{source}/refind_x64.efi"))?,
        &root,
        &format!("{dir}/refind_x64.efi"),
        0o644,
    )?;
    for part in REFIND_PARTS {
        let from = format!("{source}/{part}");
        if target::is_dir(&root, &from) {
            target::copy_tree(
                &target::path(&root, &from)?,
                &root,
                &format!("{dir}/{part}"),
            )?;
        }
    }
    let filesystem = install.mounted.filesystem;
    let theme = target::is_dir(&root, &format!("{source}/{REFIND_THEME}"));
    let conf = refind_conf(&pkgbase, filesystem == Filesystem::Btrfs, theme);
    target::write(&root, &format!("{dir}/refind.conf"), &conf, 0o644)?;
    let options = options_for(install.mounted);
    target::write(
        &root,
        "boot/refind_linux.conf",
        &refind_linux_conf(&options),
        0o644,
    )?;
    // The Linexin icon beside the kernel, which rEFInd shows for it; not on
    // a shared EFI partition at /boot, where only the files the kernel
    // cannot do without go next to it. A Linux boot partition is ours.
    let icon = format!("{source}/icons/os_linexin.png");
    if boot_is_ours(esp) && target::is_file(&root, &icon) {
        let copied = target::path(&root, &icon).and_then(|from| {
            target::copy_file(&from, &root, &format!("boot/vmlinuz-{pkgbase}.png"), 0o644)
        });
        if let Err(stop) = copied {
            run::log(&format!(
                "bootloader: no icon for the kernel: {}",
                stop.detail
            ));
        }
    }
    install.progress(Step::Bootloader, 2, PARTS);
    if !esp.shared {
        install_fallback(&root, esp, &format!("{dir}/refind_x64.efi"))?;
        // Started from EFI/BOOT, rEFInd reads its settings from there.
        target::write(
            &root,
            &format!("{}/EFI/BOOT/refind.conf", esp_dir(esp)),
            &conf,
            0o644,
        )?;
    }
    register(install, esp, "rEFInd", r"\EFI\refind\refind_x64.efi")
}

// ---------------------------------------------------------------------------
// GRUB

/// The value of the last assignment of `key` that is not commented out,
/// without its quotes.
pub fn variable(text: &str, key: &str) -> Option<String> {
    text.lines()
        .rev()
        .find_map(|line| line.trim_start().strip_prefix(&format!("{key}=")))
        .map(|value| unquote(value.trim()).to_string())
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

/// `text` with `key=value`: replacing the last assignment that is not
/// commented out, or else the first commented-out one (`#KEY=…`, as GRUB's
/// own file offers its settings), or else added at the end.
pub fn set_variable(text: &str, key: &str, value: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let assignment = format!("{key}=");
    let active = lines
        .iter()
        .rposition(|line| line.trim_start().starts_with(&assignment));
    let commented = || {
        lines.iter().position(|line| {
            line.trim_start()
                .strip_prefix('#')
                .is_some_and(|rest| rest.trim_start().starts_with(&assignment))
        })
    };
    let new_line = format!("{key}={value}");
    match active.or_else(commented) {
        Some(index) => lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if i == index {
                    let ending = &line[line.trim_end_matches(['\n', '\r']).len()..];
                    format!("{new_line}{ending}")
                } else {
                    line.to_string()
                }
            })
            .collect(),
        None => {
            let mut out = text.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&new_line);
            out.push('\n');
            out
        }
    }
}

/// `text` with each of `added` among the words of `key` (a quoted list of
/// kernel options), each once, after those already there.
fn with_words(text: &str, key: &str, added: &[&str]) -> String {
    let current = variable(text, key).unwrap_or_default();
    let mut words: Vec<&str> = current.split_whitespace().collect();
    for word in added {
        if !words.contains(word) {
            words.push(word);
        }
    }
    set_variable(text, key, &format!("\"{}\"", words.join(" ")))
}

/// `/etc/default/grub` for Linexin: its name in the menu, `quiet splash`
/// added to whatever the image's file already passes, and other systems
/// listed when there are any. Everything else stays as the image has it
/// (the legacy installer replaced the whole file). For a layout made by
/// hand (`gpt_auto_off`) [`GPT_AUTO_OFF`] joins `GRUB_CMDLINE_LINUX`, which
/// every entry passes, the recovery ones included; the easy path's file is
/// exactly as it was.
pub fn default_grub(text: &str, other_os: bool, gpt_auto_off: bool) -> String {
    let mut text = set_variable(text, "GRUB_DISTRIBUTOR", "\"Linexin\"");
    text = with_words(&text, "GRUB_CMDLINE_LINUX_DEFAULT", &["quiet", "splash"]);
    if gpt_auto_off {
        text = with_words(&text, "GRUB_CMDLINE_LINUX", &[GPT_AUTO_OFF]);
    }
    if other_os {
        text = set_variable(&text, "GRUB_DISABLE_OS_PROBER", "false");
    }
    text
}

/// Whether the generated `grub.cfg` starts the root this installation made.
pub fn names_root(cfg: &str, root_uuid: &str) -> bool {
    cfg.contains(&format!("root=UUID={root_uuid}"))
}

/// grub-install's arguments under UEFI: its loader into the EFI partition
/// where it is mounted, and its modules and `grub.cfg` into `/boot/grub`,
/// on the partition that holds the kernels (the EFI partition at `/boot`,
/// or the Linux boot partition beside one at `/efi`), which GRUB finds by
/// that filesystem's UUID.
pub fn grub_efi_args(mount: EspMount) -> Vec<String> {
    vec![
        "--target=x86_64-efi".to_string(),
        format!("--efi-directory={}", mount.path()),
        "--boot-directory=/boot".to_string(),
        format!("--bootloader-id={LABEL}"),
        "--no-nvram".to_string(),
    ]
}

/// A whole-disk path as a command line can take it.
fn disk_path_ok(disk: &str) -> bool {
    disk.strip_prefix("/dev/")
        .is_some_and(|name| !name.contains('/') && super::safe_token(name))
}

/// **grub-install's arguments on a machine a BIOS starts**: its core goes
/// into the first MiB of the disk storage named (`mounted.disk`, the disk
/// that holds `/` — and `/boot`, which the rules keep on it — behind an
/// msdos disk's MBR, or into a gpt disk's `bios_grub` partition there),
/// and only once the kernel says `/`'s partition as it was read back
/// (`mounted.root_device`) is on that disk: `link`, where
/// `/sys/class/block/<name>` points, must have it for its parent. So the
/// disk is never one taken from a partition's name or another disk of the
/// layout. The worker hands in the machine's own reader ([`block_link`]);
/// a test hands in a kernel written by hand.
pub fn grub_bios_args(
    mounted: &Mounted,
    link: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<String>, String> {
    if !disk_path_ok(&mounted.disk) {
        return Err(format!("not a disk: {:?}", mounted.disk));
    }
    let disk = device_name(&mounted.disk).ok_or("the disk has no usable name")?;
    let root = device_name(&mounted.root_device).ok_or("/ has no usable name")?;
    let link = link(root)?;
    if parent_disk(&link, root) != Some(disk) {
        return Err(format!(
            "{} is not on {} ({link})",
            mounted.root_device, mounted.disk
        ));
    }
    Ok(vec!["--target=i386-pc".to_string(), mounted.disk.clone()])
}

fn grub(install: &mut Install, esp: Option<&Esp>, other_os: bool) -> Result<(), Stop> {
    if let Some(esp) = esp {
        check_kernel_partition(install.mounted, esp)?;
    }
    ensure_package(install, "grub", "usr/bin/grub-install")?;
    let root = install.root().to_path_buf();
    let mounted = install.mounted;
    match (mounted.firmware, esp) {
        (Firmware::Uefi, Some(esp)) => {
            let args = grub_efi_args(esp.mount);
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            run::chroot(&root, "grub-install", &args)?;
        }
        (Firmware::Uefi, None) => {
            return Err(Stop::new(
                Failure::Internal,
                "GRUB for UEFI without an EFI partition",
            ))
        }
        (Firmware::Bios, _) => {
            let args = grub_bios_args(mounted, &block_link)
                .map_err(|detail| Stop::new(Failure::Internal, detail))?;
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            run::chroot(&root, "grub-install", &args)?;
        }
    }
    install.progress(Step::Bootloader, 2, PARTS);

    let current = target::read(&root, "etc/default/grub")?.unwrap_or_default();
    target::write(
        &root,
        "etc/default/grub",
        &default_grub(&current, other_os, mounted.gpt_auto_off),
        0o644,
    )?;
    run::chroot(&root, "grub-mkconfig", &["-o", "/boot/grub/grub.cfg"])?;
    let cfg = target::read(&root, "boot/grub/grub.cfg")?.unwrap_or_default();
    if !names_root(&cfg, &mounted.root_uuid) {
        return Err(Stop::new(
            Failure::CommandFailed,
            format!(
                "grub.cfg does not start the root UUID={}",
                mounted.root_uuid
            ),
        ));
    }
    if let (Firmware::Uefi, Some(esp)) = (mounted.firmware, esp) {
        if !esp.shared {
            let loader = format!("{}/EFI/{LABEL}/grubx64.efi", esp_dir(esp));
            install_fallback(&root, esp, &loader)?;
        }
        register(install, esp, LABEL, &format!(r"\EFI\{LABEL}\grubx64.efi"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared by the loaders

/// Makes sure the target has the loader's package, downloading it when the
/// image does not carry it.
fn ensure_package(install: &mut Install, package: &str, marker: &str) -> Result<(), Stop> {
    if target::exists(install.root(), marker) {
        return Ok(());
    }
    if !install.online {
        return Err(Stop::new(
            Failure::NeedsInternet,
            format!("{package} is not in the image and the internet did not answer"),
        ));
    }
    packages::install(install, &[package])
}

/// Copies the loader at `source` (relative to the root) to the
/// removable-media path of an EFI partition the installer made.
fn install_fallback(root: &Path, esp: &Esp, source: &str) -> Result<(), Stop> {
    debug_assert!(!esp.shared, "a shared EFI partition's fallback is not ours");
    let dir = match fallback_dir_name(root, esp_dir(esp))? {
        Some(name) => format!("{}/EFI/{name}", esp_dir(esp)),
        None => format!("{}/EFI/BOOT", esp_dir(esp)),
    };
    target::create_dir(root, &dir)?;
    target::copy_file(
        &target::path(root, source)?,
        root,
        &format!("{dir}/BOOTX64.EFI"),
        0o644,
    )
}

/// What a firmware entry that could not be made means: a warning when the
/// fallback loader starts the system anyway, a stop when nothing would.
pub fn unregistered(shared: bool) -> Result<Warning, Failure> {
    if shared {
        Err(Failure::CommandFailed)
    } else {
        Ok(Warning::BootEntryNotRegistered)
    }
}

/// Makes the firmware entry for `loader` (a path on the EFI partition,
/// `\EFI\…`), first in the boot order.
fn register(install: &mut Install, esp: &Esp, label: &str, loader: &str) -> Result<(), Stop> {
    let made = firmware_entry(install.mounted, esp, label, loader);
    install.progress(Step::Bootloader, PARTS, PARTS);
    let Err(detail) = made else {
        return Ok(());
    };
    run::log(&format!("bootloader: no firmware entry: {detail}"));
    match unregistered(esp.shared) {
        Ok(warning) => {
            install.warn(warning);
            Ok(())
        }
        Err(failure) => Err(Stop::new(
            failure,
            format!("the firmware entry for the shared EFI partition was not made: {detail}"),
        )),
    }
}

fn firmware_entry(mounted: &Mounted, esp: &Esp, label: &str, loader: &str) -> Result<(), String> {
    let args = entry_args(mounted, esp, label, loader, &block_link, &partition_file)?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run::run("efibootmgr", &args)
        .map(|_| ())
        .map_err(|stop| stop.detail)
}

/// **efibootmgr's arguments for the firmware entry of `loader`** on the EFI
/// partition `esp`, from what the kernel says of it by its name (`vdb1`):
/// `link`, where `/sys/class/block/<name>` points, whose parent must be the
/// disk storage named (`mounted.disk` — the EFI partition's own disk,
/// whichever disk `/` is on); and `number`, the text of its `partition`
/// file, the number the entry is made with, never one cut off its name.
/// The worker hands in the machine's own readers ([`block_link`],
/// [`partition_file`]); a test hands in a kernel written by hand.
pub fn entry_args(
    mounted: &Mounted,
    esp: &Esp,
    label: &str,
    loader: &str,
    link: &dyn Fn(&str) -> Result<String, String>,
    number: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<String>, String> {
    let partition = device_name(&esp.device).ok_or("the EFI partition has no usable name")?;
    let disk = device_name(&mounted.disk).ok_or("the disk has no usable name")?;
    let link = link(partition)?;
    if parent_disk(&link, partition) != Some(disk) {
        return Err(format!(
            "{} is not on {} ({link})",
            esp.device, mounted.disk
        ));
    }
    let number = number(partition)
        .as_deref()
        .and_then(partition_number)
        .ok_or_else(|| format!("/sys/class/block/{partition}/partition gives no number"))?;
    Ok(efibootmgr_args(&mounted.disk, number, loader, label))
}

/// Where `/sys/class/block/<name>` points. In a test build it refuses
/// before anything is read (`run::fence::block_devices`): no test reads
/// this computer's block devices, and a test of the entry or of GRUB's
/// BIOS target hands [`entry_args`] or [`grub_bios_args`] a kernel of its
/// own.
fn block_link(name: &str) -> Result<String, String> {
    #[cfg(test)]
    run::fence::block_devices();
    let sys = format!("/sys/class/block/{name}");
    std::fs::read_link(&sys)
        .map(|link| link.to_string_lossy().into_owned())
        .map_err(|e| format!("{sys}: {e}"))
}

/// The text of `/sys/class/block/<name>/partition`, where it can be read;
/// refused in a test build as [`block_link`] is.
fn partition_file(name: &str) -> Option<String> {
    #[cfg(test)]
    run::fence::block_devices();
    std::fs::read_to_string(format!("/sys/class/block/{name}/partition")).ok()
}

/// The kernel's name of a device (`nvme0n1p1` for `/dev/nvme0n1p1`), when
/// it is one a path can be built from.
pub fn device_name(path: &str) -> Option<&str> {
    let name = path.strip_prefix("/dev/")?;
    (!name.contains('/') && super::safe_token(name)).then_some(name)
}

/// The disk a partition is on, from its link in `/sys/class/block`
/// (`…/nvme0n1/nvme0n1p1`): the directory the partition's sits in.
pub fn parent_disk<'a>(link: &'a str, partition: &str) -> Option<&'a str> {
    let mut parts = link.trim_end_matches('/').rsplit('/');
    (parts.next()? == partition).then(|| parts.next()).flatten()
}

/// The number in `/sys/class/block/<partition>/partition`.
pub fn partition_number(text: &str) -> Option<u32> {
    text.trim().parse().ok().filter(|n| *n > 0)
}

/// efibootmgr's arguments: quiet, because the table it prints otherwise
/// can hold bytes that are not text and nothing here reads it.
pub fn efibootmgr_args(disk: &str, partition: u32, loader: &str, label: &str) -> Vec<String> {
    [
        "--quiet",
        "--create",
        "--disk",
        disk,
        "--part",
        &partition.to_string(),
        "--loader",
        loader,
        "--label",
        label,
    ]
    .iter()
    .map(|a| a.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn esp(mount: EspMount, shared: bool) -> Esp {
        Esp {
            device: "/dev/vdz1".into(),
            uuid: "ABCD-1234".into(),
            mount,
            shared,
        }
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// The policy written out case by case, independently of `decide`.
    fn expected(
        firmware: Firmware,
        table: Table,
        mount: Option<EspMount>,
        other_os: bool,
        choice: Bootloader,
    ) -> Option<Loader> {
        use Bootloader as B;
        match (firmware, choice) {
            (Firmware::Bios, B::Automatic | B::Grub) => Some(Loader::Grub),
            (Firmware::Bios, _) => None,
            (Firmware::Uefi, _) if mount.is_none() => None,
            (Firmware::Uefi, B::Grub) => Some(Loader::Grub),
            (Firmware::Uefi, B::Refind) => Some(Loader::Refind),
            (Firmware::Uefi, B::SystemdBoot) => {
                (table == Table::Gpt).then_some(Loader::SystemdBoot)
            }
            (Firmware::Uefi, B::Automatic) => Some(if table == Table::Msdos {
                Loader::Grub
            } else if other_os {
                Loader::Refind
            } else {
                Loader::SystemdBoot
            }),
        }
    }

    #[test]
    fn every_combination_follows_the_policy() {
        let mut checked = 0;
        for firmware in [Firmware::Uefi, Firmware::Bios] {
            for table in [Table::Gpt, Table::Msdos] {
                for mount in [None, Some(EspMount::Boot), Some(EspMount::Efi)] {
                    for shared in [false, true] {
                        for other_os in [false, true] {
                            for choice in [
                                Bootloader::Automatic,
                                Bootloader::SystemdBoot,
                                Bootloader::Grub,
                                Bootloader::Refind,
                            ] {
                                let partition = mount.map(|m| esp(m, shared));
                                let got =
                                    decide(firmware, table, partition.as_ref(), other_os, choice);
                                let want = expected(firmware, table, mount, other_os, choice)
                                    .ok_or(Invalid::BootloaderUnavailable);
                                assert_eq!(
                                    got, want,
                                    "{firmware:?} {table:?} {mount:?} shared={shared} other={other_os} {choice:?}"
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 2 * 2 * 3 * 2 * 2 * 4);
    }

    #[test]
    fn the_policy_in_words() {
        let boot = esp(EspMount::Boot, false);
        let efi = esp(EspMount::Efi, true);
        let auto = Bootloader::Automatic;
        // Linexin alone on a GPT disk: systemd-boot.
        assert_eq!(
            decide(Firmware::Uefi, Table::Gpt, Some(&boot), false, auto),
            Ok(Loader::SystemdBoot)
        );
        // Beside Windows: rEFInd, whatever the partition's room.
        assert_eq!(
            decide(Firmware::Uefi, Table::Gpt, Some(&efi), true, auto),
            Ok(Loader::Refind)
        );
        // Alone beside a small shared partition (the kernels on a Linux boot
        // partition): systemd-boot all the same.
        assert_eq!(
            decide(Firmware::Uefi, Table::Gpt, Some(&efi), false, auto),
            Ok(Loader::SystemdBoot)
        );
        // A DOS table: GRUB, even beside another system.
        assert_eq!(
            decide(Firmware::Uefi, Table::Msdos, Some(&boot), true, auto),
            Ok(Loader::Grub)
        );
        assert_eq!(
            decide(Firmware::Bios, Table::Msdos, None, true, auto),
            Ok(Loader::Grub)
        );
        assert_eq!(
            decide(Firmware::Bios, Table::Gpt, None, false, Bootloader::Refind),
            Err(Invalid::BootloaderUnavailable)
        );
        // Chosen by name, systemd-boot and GRUB both work beside a small
        // shared partition; systemd-boot needs GPT.
        for choice in [Bootloader::SystemdBoot, Bootloader::Grub] {
            assert!(decide(Firmware::Uefi, Table::Gpt, Some(&efi), true, choice).is_ok());
        }
        assert_eq!(
            decide(
                Firmware::Uefi,
                Table::Msdos,
                Some(&efi),
                false,
                Bootloader::SystemdBoot
            ),
            Err(Invalid::BootloaderUnavailable)
        );
    }

    #[test]
    fn bootctl_is_told_where_the_kernels_are() {
        assert_eq!(
            bootctl_args(EspMount::Boot),
            [
                "--esp-path=/boot",
                "--variables=no",
                "--make-entry-directory=no",
                "install"
            ]
        );
        assert_eq!(
            bootctl_args(EspMount::Efi),
            [
                "--esp-path=/efi",
                "--boot-path=/boot",
                "--variables=no",
                "--make-entry-directory=no",
                "install"
            ]
        );
    }

    #[test]
    fn systemd_boots_loader_is_copied_from_the_efi_partition_wherever_it_is_mounted() {
        // An EFI partition of the installation's own at /efi, beside a Linux
        // boot partition at /boot, has bootctl's loader under efi/: copied
        // from boot/, the fallback would stop the installation in the
        // bootloader step, as QEMU's manual-uefi-new-table-over-used showed:
        // "cannot read …/root/boot/EFI/systemd/systemd-bootx64.efi".
        assert_eq!(
            systemd_boot_loader(&esp(EspMount::Efi, false)),
            "efi/EFI/systemd/systemd-bootx64.efi"
        );
        assert_eq!(
            systemd_boot_loader(&esp(EspMount::Boot, false)),
            "boot/EFI/systemd/systemd-bootx64.efi"
        );
        // It is on the partition bootctl was told of, under the path the
        // firmware entry names.
        for mount in [EspMount::Boot, EspMount::Efi] {
            let loader = systemd_boot_loader(&esp(mount, false));
            assert!(bootctl_args(mount)
                .contains(&format!("--esp-path=/{}", esp_dir(&esp(mount, false)))));
            assert_eq!(
                loader
                    .strip_prefix(esp_dir(&esp(mount, false)))
                    .map(|rest| rest.replace('/', r"\")),
                Some(r"\EFI\systemd\systemd-bootx64.efi".to_string())
            );
        }
    }

    #[test]
    fn grub_goes_to_the_efi_partition_and_reads_boot() {
        let at_efi = grub_efi_args(EspMount::Efi);
        assert!(at_efi.contains(&"--efi-directory=/efi".to_string()));
        assert!(at_efi.contains(&"--boot-directory=/boot".to_string()));
        assert!(at_efi.contains(&"--no-nvram".to_string()));
        assert!(at_efi.contains(&"--bootloader-id=Linexin".to_string()));
        let at_boot = grub_efi_args(EspMount::Boot);
        assert!(at_boot.contains(&"--efi-directory=/boot".to_string()));
    }

    #[test]
    fn only_a_shared_efi_partition_at_boot_is_someone_elses() {
        assert!(boot_is_ours(&esp(EspMount::Boot, false)));
        assert!(boot_is_ours(&esp(EspMount::Efi, true)));
        assert!(!boot_is_ours(&esp(EspMount::Boot, true)));
    }

    #[test]
    fn other_systems_are_only_looked_for_when_they_change_something() {
        assert!(other_os_matters(Bootloader::Automatic));
        assert!(other_os_matters(Bootloader::Grub));
        assert!(!other_os_matters(Bootloader::SystemdBoot));
        assert!(!other_os_matters(Bootloader::Refind));
    }

    #[test]
    fn another_systems_efi_directory_is_found() {
        assert!(other_systems_on_esp(&strings(&[
            "BOOT", "Linexin", "systemd", "refind", "Linux", "grub", "arch"
        ]))
        .is_empty());
        assert!(other_systems_on_esp(&strings(&["boot", "SYSTEMD", "Refind"])).is_empty());
        assert_eq!(
            other_systems_on_esp(&strings(&["Boot", "Microsoft", "ubuntu", "fedora"])),
            ["Microsoft", "ubuntu", "fedora"]
        );
    }

    #[test]
    fn os_prober_finds_what_is_not_this_installation() {
        let output = "\
/dev/vdz1@/efi/Microsoft/Boot/bootmgfw.efi:Windows Boot Manager:Windows:efi
/dev/vdz1@/EFI/Linexin/grubx64.efi:Linexin:Linexin:efi
/dev/vdz1@/EFI/BOOT/BOOTX64.EFI:Fallback:Fallback:efi
/dev/vdz3:Example Linux 1.0:Example:linux
/dev/vdz2:Linexin:Linexin:linux
/dev/vdy1:Windows 10:Windows:chain
";
        let own = ["/dev/vdz2", "/dev/vdz1"];
        assert_eq!(
            os_prober_others(output, &own),
            [
                "/dev/vdz1@/efi/Microsoft/Boot/bootmgfw.efi:Windows Boot Manager:Windows:efi",
                "/dev/vdz3:Example Linux 1.0:Example:linux",
                "/dev/vdy1:Windows 10:Windows:chain",
            ]
        );
        assert!(os_prober_others("\n", &own).is_empty());
    }

    #[test]
    fn kernel_options_name_the_root_and_its_subvolume() {
        assert_eq!(kernel_options("1111-2222", None), "root=UUID=1111-2222 rw");
        assert_eq!(
            kernel_options("1111-2222", Some("@")),
            "root=UUID=1111-2222 rw rootflags=subvol=@"
        );
    }

    #[test]
    fn systemd_boot_entries_and_loader_conf() {
        let (main, fallback) =
            systemd_boot_entries("linux", "root=UUID=1111-2222 rw rootflags=subvol=@");
        assert_eq!(
            main,
            "title   Linexin\nlinux   /vmlinuz-linux\ninitrd  /initramfs-linux.img\n\
             options root=UUID=1111-2222 rw rootflags=subvol=@ quiet splash\n"
        );
        assert_eq!(
            fallback,
            "title   Linexin (fallback)\nlinux   /vmlinuz-linux\ninitrd  /initramfs-linux-fallback.img\n\
             options root=UUID=1111-2222 rw rootflags=subvol=@\n"
        );
        assert!(LOADER_CONF.contains("default  linexin.conf\n"));
        assert!(LOADER_CONF.contains("timeout  0\n"));
        assert!(LOADER_CONF.contains("console-mode max\n"));
        assert!(LOADER_CONF.contains("editor   no\n"));
    }

    #[test]
    fn refind_files() {
        let conf = refind_conf("linux", false, true);
        assert!(conf.starts_with("include themes/refind-theme-regular/theme.conf\n"));
        assert!(conf.contains("timeout 5\n"));
        assert!(conf.contains("default_selection \"vmlinuz-linux\"\n"));
        assert!(conf.contains("also_scan_dirs boot,EFI/manjaro"));
        let btrfs = refind_conf("linux-zen", true, false);
        assert!(!btrfs.contains("include"));
        assert!(btrfs.contains("default_selection \"vmlinuz-linux-zen\"\n"));
        assert!(btrfs.contains("also_scan_dirs boot,@/boot,EFI/manjaro"));

        let linux = refind_linux_conf("root=UUID=1111-2222 rw");
        let lines: Vec<&str> = linux.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(
            lines[0].starts_with("\"Boot Linexin\"") && lines[0].ends_with("rw quiet splash\"")
        );
        assert!(lines[1].ends_with("rw systemd.unit=multi-user.target\""));
        assert!(lines[2].ends_with("rw single\""));
        assert!(!REFIND_PARTS.iter().any(|p| p.starts_with("drivers")));
    }

    #[test]
    fn default_grub_is_edited_not_replaced() {
        let image = "\
# GRUB boot loader configuration
GRUB_DEFAULT=0
GRUB_TIMEOUT=5
GRUB_DISTRIBUTOR=\"Arch\"
GRUB_CMDLINE_LINUX_DEFAULT=\"loglevel=3 quiet\"
GRUB_CMDLINE_LINUX=\"\"
#GRUB_DISABLE_OS_PROBER=false
";
        let alone = default_grub(image, false, false);
        assert_eq!(
            alone,
            image
                .replace("\"Arch\"", "\"Linexin\"")
                .replace("loglevel=3 quiet\"", "loglevel=3 quiet splash\"")
        );
        let beside = default_grub(image, true, false);
        assert!(beside.contains("\nGRUB_DISABLE_OS_PROBER=false\n"));
        assert!(!beside.contains("#GRUB_DISABLE_OS_PROBER"));
        // Applying it again changes nothing.
        assert_eq!(default_grub(&beside, true, false), beside);
        // A file without the settings gets them added.
        assert_eq!(
            default_grub("GRUB_TIMEOUT=5", false, false),
            "GRUB_TIMEOUT=5\nGRUB_DISTRIBUTOR=\"Linexin\"\nGRUB_CMDLINE_LINUX_DEFAULT=\"quiet splash\"\n"
        );
    }

    /// `options_for` as it was before a layout made by hand could turn GPT
    /// auto-discovery off (Unit Q, Q5).
    fn options_before(mounted: &Mounted) -> String {
        let subvolume = match mounted.filesystem {
            Filesystem::Btrfs => mounted.root_subvolume.as_deref(),
            Filesystem::Ext4 => None,
        };
        kernel_options(&mounted.root_uuid, subvolume)
    }

    /// `default_grub` as it was before Q5, word for word.
    fn default_grub_before(text: &str, other_os: bool) -> String {
        let mut text = set_variable(text, "GRUB_DISTRIBUTOR", "\"Linexin\"");
        let current = variable(&text, "GRUB_CMDLINE_LINUX_DEFAULT").unwrap_or_default();
        let mut words: Vec<&str> = current.split_whitespace().collect();
        for word in ["quiet", "splash"] {
            if !words.contains(&word) {
                words.push(word);
            }
        }
        text = set_variable(
            &text,
            "GRUB_CMDLINE_LINUX_DEFAULT",
            &format!("\"{}\"", words.join(" ")),
        );
        if other_os {
            text = set_variable(&text, "GRUB_DISABLE_OS_PROBER", "false");
        }
        text
    }

    /// os-prober's list of this installation's own as `other_os` wrote it
    /// before Q5.
    fn own_before(mounted: &Mounted) -> Vec<&str> {
        let mut own = vec![mounted.root_device.as_str()];
        own.extend(mounted.home_device.as_deref());
        own.extend(mounted.esp.as_ref().map(|esp| esp.device.as_str()));
        own.extend(mounted.linux_boot.as_ref().map(|boot| boot.device.as_str()));
        own
    }

    /// The image's `/etc/default/grub` in the shapes the step meets: the
    /// image's own, one with options already on both lines, one with
    /// `GRUB_CMDLINE_LINUX` only commented out, and nothing at all.
    const GRUB_FILES: &[&str] = &[
        "# GRUB boot loader configuration\nGRUB_DEFAULT=0\nGRUB_TIMEOUT=5\n\
         GRUB_DISTRIBUTOR=\"Arch\"\nGRUB_CMDLINE_LINUX_DEFAULT=\"loglevel=3 quiet\"\n\
         GRUB_CMDLINE_LINUX=\"\"\n#GRUB_DISABLE_OS_PROBER=false\n",
        "GRUB_CMDLINE_LINUX_DEFAULT='splash nowatchdog'\nGRUB_CMDLINE_LINUX=\"zswap.enabled=0\"\n",
        "#GRUB_CMDLINE_LINUX=\"\"\nGRUB_TIMEOUT=5",
        "",
    ];

    /// The [`Mounted`] the easy path's storage half builds for `plan` (with
    /// devices and a UUID made up): the shapes every fixture request comes
    /// to, which the boot loader's step is then given.
    fn easy_mounted(plan: &crate::worker::partition::Plan, shared: Option<EspMount>) -> Mounted {
        use crate::worker::layout::LinuxBoot;
        use crate::worker::partition::{EspPlan, Role};
        let esp = match &plan.esp {
            EspPlan::None => None,
            EspPlan::New => Some(Esp {
                device: "/dev/vdx1".into(),
                uuid: "AAAA-0001".into(),
                mount: EspMount::Boot,
                shared: false,
            }),
            EspPlan::Shared { path } => Some(Esp {
                device: path.clone(),
                uuid: "AAAA-0002".into(),
                mount: shared.unwrap_or(EspMount::Boot),
                shared: true,
            }),
        };
        Mounted {
            firmware: Firmware::Uefi,
            table: plan.table,
            disk: plan.disk.clone(),
            root: "/run/lxb-installer/stage-test/root".into(),
            root_device: "/dev/vdx7".into(),
            root_uuid: "11111111-2222-4333-8444-555555555555".into(),
            filesystem: plan.filesystem,
            root_subvolume: (plan.filesystem == Filesystem::Btrfs).then(|| "@".to_string()),
            esp,
            linux_boot: plan.created(Role::LinuxBoot).map(|_| LinuxBoot {
                device: "/dev/vdx8".into(),
                uuid: "AAAA-0003".into(),
            }),
            home_device: plan.home.as_ref().map(|home| home.path.clone()),
            swapfile: None,
            fstab: String::new(),
            gpt_auto_off: false,
            other_devices: Vec::new(),
        }
    }

    /// **The easy path's boot loader is told exactly what it was told
    /// before** a layout made by hand could turn GPT auto-discovery off and
    /// name partitions of its own to os-prober (Unit Q, Q5): for every
    /// fixture request — each fixture disk at 512 and 4096 bytes a sector,
    /// every place on it, ext4 and Btrfs, a shared EFI partition with room
    /// for the kernels and without, a /home of its own or none, on either
    /// firmware — the kernel's options (systemd-boot's entries and rEFInd's
    /// lines are made of them), `/etc/default/grub` for every shape of the
    /// image's file, beside another system or alone, and os-prober's list
    /// of this installation's own are each what the code before wrote.
    #[test]
    fn the_easy_paths_kernel_options_grub_file_and_os_probers_list_are_as_they_were() {
        use crate::model::config::Home;
        use crate::worker::partition::fixtures::{all, config, targets};
        use crate::worker::partition::{plan, SharedRoom};
        let mut checked = 0;
        for sector in [512, 4096] {
            for (name, disk) in all(sector) {
                for (place, target) in targets(&disk) {
                    for filesystem in [Filesystem::Ext4, Filesystem::Btrfs] {
                        for home in [None, Some(true), Some(false)] {
                            let mut easy = config(disk.clone(), target.clone());
                            easy.storage.filesystem = filesystem;
                            easy.storage.home = home.and_then(|format| {
                                let part = disk.partitions.last()?;
                                Some(Home {
                                    path: part.path.clone(),
                                    format,
                                    disk: None,
                                })
                            });
                            for firmware in [Firmware::Uefi, Firmware::Bios] {
                                for room in [SharedRoom::Kernels, SharedRoom::LoaderOnly] {
                                    let Ok(plan) = plan(&easy, firmware, room) else {
                                        continue;
                                    };
                                    let mut mounted = easy_mounted(&plan, Some(room.esp_mount()));
                                    mounted.firmware = firmware;
                                    let case = format!(
                                        "{name} {sector} {place} {filesystem:?} {home:?} \
                                         {firmware:?} {room:?}"
                                    );
                                    assert_eq!(
                                        options_for(&mounted),
                                        options_before(&mounted),
                                        "{case}"
                                    );
                                    assert_eq!(
                                        own_devices(&mounted),
                                        own_before(&mounted),
                                        "{case}"
                                    );
                                    checked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 400, "only {checked} easy requests were planned");
        for text in GRUB_FILES {
            for other_os in [false, true] {
                assert_eq!(
                    default_grub(text, other_os, false),
                    default_grub_before(text, other_os),
                    "{text:?} beside another system: {other_os}"
                );
            }
        }
    }

    /// **A layout made by hand turns GPT auto-discovery off in every entry
    /// of every loader** (Unit Q, Q5): `systemd.gpt_auto=0` after the root's
    /// options in systemd-boot's two entries and rEFInd's three lines, and
    /// in GRUB's `GRUB_CMDLINE_LINUX` — once, beside what the image's file
    /// already passes, wherever it stood, and `quiet splash` left to the
    /// default line alone — and os-prober takes every partition it formats
    /// or makes for this installation's own.
    #[test]
    fn a_hand_made_layout_turns_gpt_auto_discovery_off_in_every_loader() {
        let plan = {
            use crate::worker::partition::fixtures::{config, disk_empty};
            use crate::worker::partition::{plan, SharedRoom};
            let easy = config(disk_empty(512), crate::model::config::Target::WholeDisk);
            plan(&easy, Firmware::Uefi, SharedRoom::Kernels).unwrap()
        };
        let mut laid = easy_mounted(&plan, None);
        laid.gpt_auto_off = true;
        laid.other_devices = vec!["/dev/vdx3".into(), "/dev/vdx4".into()];
        let options = options_for(&laid);
        assert_eq!(
            options,
            "root=UUID=11111111-2222-4333-8444-555555555555 rw systemd.gpt_auto=0"
        );
        laid.filesystem = Filesystem::Btrfs;
        laid.root_subvolume = Some("@".into());
        let options = options_for(&laid);
        assert!(
            options.ends_with(" rootflags=subvol=@ systemd.gpt_auto=0"),
            "{options}"
        );
        let (main, fallback) = systemd_boot_entries("linux", &options);
        for entry in [&main, &fallback] {
            assert!(entry.contains("systemd.gpt_auto=0"), "{entry}");
        }
        for line in refind_linux_conf(&options).lines() {
            assert!(line.contains("systemd.gpt_auto=0"), "{line}");
        }
        for text in GRUB_FILES {
            let edited = default_grub(text, false, true);
            let linux = variable(&edited, "GRUB_CMDLINE_LINUX").unwrap();
            let words: Vec<&str> = linux.split_whitespace().collect();
            assert_eq!(
                words.iter().filter(|word| **word == GPT_AUTO_OFF).count(),
                1,
                "{text:?}: {linux:?}"
            );
            let before = variable(&default_grub(text, false, false), "GRUB_CMDLINE_LINUX")
                .unwrap_or_default();
            assert!(linux.starts_with(before.as_str()), "{text:?}: {linux:?}");
            let default = variable(&edited, "GRUB_CMDLINE_LINUX_DEFAULT").unwrap();
            assert!(!default.contains(GPT_AUTO_OFF), "{default:?}");
            // Applying it again changes nothing.
            assert_eq!(default_grub(&edited, false, true), edited);
        }
        assert_eq!(
            variable(
                &default_grub(GRUB_FILES[1], true, true),
                "GRUB_CMDLINE_LINUX"
            )
            .as_deref(),
            Some("zswap.enabled=0 systemd.gpt_auto=0")
        );
        let own = own_devices(&laid);
        assert_eq!(&own[own.len() - 2..], ["/dev/vdx3", "/dev/vdx4"]);
        assert_eq!(&own[..own.len() - 2], own_before(&laid).as_slice());
    }

    #[test]
    fn grub_variables_are_read_from_the_last_active_line() {
        let text = "#GRUB_CMDLINE_LINUX_DEFAULT=\"x\"\nGRUB_CMDLINE_LINUX_DEFAULT='a b'\n";
        assert_eq!(
            variable(text, "GRUB_CMDLINE_LINUX_DEFAULT").as_deref(),
            Some("a b")
        );
        assert_eq!(variable(text, "GRUB_TIMEOUT"), None);
        assert!(names_root(
            "linux /vmlinuz-linux root=UUID=1111-2222 rw quiet",
            "1111-2222"
        ));
        assert!(!names_root(
            "linux /vmlinuz-linux root=/dev/vdz2",
            "1111-2222"
        ));
    }

    #[test]
    fn the_firmware_entry_uses_the_kernels_partition_number() {
        assert_eq!(device_name("/dev/nvme0n1p1"), Some("nvme0n1p1"));
        assert_eq!(device_name("/dev/../etc"), None);
        assert_eq!(device_name("/dev/-x"), None);
        assert_eq!(device_name("vda1"), None);
        assert_eq!(
            parent_disk(
                "../../devices/pci0000:00/0000:00:04.0/nvme/nvme0/nvme0n1/nvme0n1p1",
                "nvme0n1p1"
            ),
            Some("nvme0n1")
        );
        assert_eq!(
            parent_disk("../../devices/virtual/block/vdz1", "vdz2"),
            None
        );
        assert_eq!(partition_number("12\n"), Some(12));
        assert_eq!(partition_number("0\n"), None);
        assert_eq!(partition_number("p1"), None);
        assert_eq!(
            efibootmgr_args("/dev/nvme0n1", 1, r"\EFI\refind\refind_x64.efi", "rEFInd"),
            [
                "--quiet",
                "--create",
                "--disk",
                "/dev/nvme0n1",
                "--part",
                "1",
                "--loader",
                r"\EFI\refind\refind_x64.efi",
                "--label",
                "rEFInd"
            ]
        );
    }

    /// The firmware entry's own readers of `/sys/class/block` — the link
    /// GRUB's BIOS target is judged by among them — refuse every test
    /// before they read anything: a test of the entry hands `entry_args`
    /// a kernel of its own, one of GRUB's `grub_bios_args`.
    #[test]
    fn no_test_reads_this_computers_block_devices_for_the_firmware_entry() {
        for refused in [
            run::fence::refusal(|| block_link("vda1").is_ok()),
            run::fence::refusal(|| partition_file("vda1").is_some()),
        ] {
            let words = refused.expect("this computer's /sys/class/block was read");
            assert!(words.contains("test build:"), "{words}");
        }
    }

    #[test]
    fn a_refused_entry_stops_only_on_a_shared_partition() {
        assert_eq!(unregistered(false), Ok(Warning::BootEntryNotRegistered));
        assert_eq!(unregistered(true), Err(Failure::CommandFailed));
    }

    #[test]
    fn only_whole_disk_paths_reach_grub_install() {
        assert!(disk_path_ok("/dev/sda"));
        assert!(disk_path_ok("/dev/nvme0n1"));
        assert!(!disk_path_ok("/dev/disk/by-id/x"));
        assert!(!disk_path_ok("sda"));
        assert!(!disk_path_ok("/dev/-sda"));
    }

    /// **GRUB goes on the disk the kernel says holds `/`** (Unit Q, Q7): on
    /// a machine a BIOS starts, grub-install is given `--target=i386-pc` and
    /// the disk storage named, once `/`'s partition as it was read back is
    /// on that disk by its link in `/sys/class/block` — a virtio disk, an
    /// NVMe drive — and refused (before anything starts) where the kernel
    /// puts `/` on another disk, has no such partition, or the disk is no
    /// whole-disk path. The link is a kernel written by hand.
    #[test]
    fn grub_on_a_bios_goes_on_the_disk_the_kernel_says_holds_root() {
        let kernel = |name: &str| -> Result<String, String> {
            let disk = match name {
                "vda2" => "virtio2/block/vda",
                "vdb3" => "virtio3/block/vdb",
                "nvme0n1p2" => "nvme/nvme0/nvme0n1",
                _ => return Err(format!("/sys/class/block/{name}: no such device")),
            };
            Ok(format!(
                "../../devices/pci0000:00/0000:00:05.0/{disk}/{name}"
            ))
        };
        let bios = |disk: &str, root: &str| Mounted {
            firmware: Firmware::Bios,
            table: Table::Msdos,
            disk: disk.into(),
            root: "/run/lxb-installer/stage-test/root".into(),
            root_device: root.into(),
            root_uuid: "11111111-2222-4333-8444-555555555555".into(),
            filesystem: Filesystem::Ext4,
            root_subvolume: None,
            esp: None,
            linux_boot: None,
            home_device: None,
            swapfile: None,
            fstab: String::new(),
            gpt_auto_off: true,
            other_devices: Vec::new(),
        };
        let args = |disk: &str, root: &str| grub_bios_args(&bios(disk, root), &kernel);
        assert_eq!(
            args("/dev/vda", "/dev/vda2"),
            Ok(strings(&["--target=i386-pc", "/dev/vda"]))
        );
        assert_eq!(
            args("/dev/nvme0n1", "/dev/nvme0n1p2"),
            Ok(strings(&["--target=i386-pc", "/dev/nvme0n1"]))
        );
        // / on another disk than the one named: never GRUB on that one.
        let elsewhere = args("/dev/vda", "/dev/vdb3").unwrap_err();
        assert!(
            elsewhere.contains("/dev/vdb3 is not on /dev/vda"),
            "{elsewhere}"
        );
        assert!(args("/dev/vdb", "/dev/vda2").is_err());
        // No such partition, or no usable name: nothing is asked of GRUB.
        assert!(args("/dev/vda", "/dev/vda9").is_err());
        assert!(args("/dev/vda", "vda2").is_err());
        let not_a_disk = args("/dev/disk/by-id/x", "/dev/vda2").unwrap_err();
        assert_eq!(not_a_disk, "not a disk: \"/dev/disk/by-id/x\"");
    }
}
