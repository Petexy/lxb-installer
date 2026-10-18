//! The Accounts step: the computer's name, the person's account, the root
//! account and who may use sudo.
//!
//! Passwords reach `chpasswd` inside the target on its standard input and
//! nowhere else: never on a command line (which every user can read in
//! `/proc` and which the worker logs), never in a file, and the buffer that
//! carried one is overwritten as soon as it has been sent. Unlike the legacy
//! installer, every part of this step must succeed — an installation without
//! the person's account is not one.
use super::check::wipe;
use super::{Chroot, Install};
use crate::model::config::{Account, InstallConfig};
use crate::model::protocol::{Failure, Step};
use crate::worker::layout::Stop;
use crate::worker::{run, target};
use std::path::Path;

/// The groups the legacy installer gave the person, less `input` (which
/// would let every program the person runs read every keystroke).
pub const GROUPS: &[&str] = &["wheel", "audio", "video", "network", "storage", "power"];

/// Who may use sudo: the wheel group, asking for their own password.
pub const SUDOERS: &str = "%wheel ALL=(ALL:ALL) ALL\n";

/// The sudoers drop-in the installer writes.
pub const SUDOERS_FILE: &str = "etc/sudoers.d/10-installer";

/// How many parts the step reports progress in.
const PARTS: u64 = 5;

/// Whether a colon-separated account file (`/etc/passwd`, `/etc/group`) has
/// an entry named `name`.
pub fn has_entry(text: &str, name: &str) -> bool {
    text.lines()
        .any(|line| line.split(':').next() == Some(name))
}

/// `/etc/hosts` with this computer's name on the `127.0.1.1` line: any old
/// `127.0.1.1` line (the live medium's name) is replaced, everything else is
/// kept, and the localhost lines are added when missing.
pub fn hosts(existing: &str, hostname: &str) -> String {
    let first = |line: &str| line.split_whitespace().next().map(str::to_string);
    let mut lines: Vec<String> = existing
        .lines()
        .filter(|line| first(line).as_deref() != Some("127.0.1.1"))
        .map(str::to_string)
        .collect();
    let has_localhost = |lines: &[String], address: &str| {
        lines.iter().any(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some(address) && words.any(|w| w == "localhost")
        })
    };
    if !has_localhost(&lines, "127.0.0.1") {
        lines.push("127.0.0.1\tlocalhost".into());
    }
    if !has_localhost(&lines, "::1") {
        lines.push("::1\t\tlocalhost".into());
    }
    lines.push(format!("127.0.1.1\t{hostname}"));
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// The groups of [`GROUPS`] the target's `/etc/group` has, so that a group
/// an image does without does not stop `useradd`. `wheel` is required: it
/// is what lets the person administer the system.
pub fn present_groups(group_file: &str) -> Result<Vec<&'static str>, Stop> {
    if !has_entry(group_file, "wheel") {
        return Err(Stop::new(
            Failure::ImageInvalid,
            "the image has no wheel group",
        ));
    }
    Ok(GROUPS
        .iter()
        .copied()
        .filter(|group| has_entry(group_file, group))
        .collect())
}

/// `useradd`'s arguments for the person's account.
pub fn useradd_args(account: &Account, groups: &[&str]) -> Vec<String> {
    vec![
        "-m".into(),
        "-c".into(),
        account.full_name.clone(),
        "-G".into(),
        groups.join(","),
        "-s".into(),
        "/bin/bash".into(),
        account.username.clone(),
    ]
}

/// The line `chpasswd` reads, `user:password\n`, in a buffer made at its
/// final size (so no reallocation leaves a copy behind) that is overwritten
/// when it is dropped.
pub struct PasswordLine(Vec<u8>);

impl PasswordLine {
    pub fn new(user: &str, password: &str) -> Self {
        let mut line = Vec::with_capacity(user.len() + password.len() + 2);
        line.extend_from_slice(user.as_bytes());
        line.push(b':');
        line.extend_from_slice(password.as_bytes());
        line.push(b'\n');
        Self(line)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for PasswordLine {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// The program and arguments that set a password inside the target, which
/// is everything the worker logs about the command; the password itself is
/// only ever in the [`PasswordLine`] sent on standard input.
pub fn chpasswd_command(root: &str) -> (&'static str, Vec<String>) {
    (
        "arch-chroot",
        vec![root.to_string(), "/usr/bin/chpasswd".to_string()],
    )
}

fn set_password(root: &Path, user: &str, password: &str) -> Result<(), Stop> {
    let line = PasswordLine::new(user, password);
    let (program, args) = chpasswd_command(run::utf8(root)?);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run::run_input(program, &args, line.bytes(), true).map(|_| ())
}

pub fn run(install: &mut Install) -> Result<(), Stop> {
    let root = install.root().to_path_buf();
    // The configuration is borrowed for the whole run, not from `install`,
    // so it stays readable while progress is reported.
    let config = install.config;
    let account = &config.account;

    target::write(
        &root,
        "etc/hostname",
        &format!("{}\n", account.hostname),
        0o644,
    )?;
    let existing = target::read(&root, "etc/hosts")?.unwrap_or_default();
    target::write(
        &root,
        "etc/hosts",
        &hosts(&existing, &account.hostname),
        0o644,
    )?;
    install.progress(Step::Accounts, 1, PARTS);

    let passwd = target::read(&root, "etc/passwd")?.unwrap_or_default();
    if has_entry(&passwd, &account.username) {
        return Err(Stop::new(
            Failure::InvalidRequest,
            format!(
                "the image already has an account named {}",
                account.username
            ),
        ));
    }
    let group_file = target::read(&root, "etc/group")?.unwrap_or_default();
    let groups = present_groups(&group_file)?;
    if groups.len() < GROUPS.len() {
        run::log(&format!(
            "accounts: the image has only the groups {}",
            groups.join(",")
        ));
    }
    let args = useradd_args(account, &groups);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run::chroot(&root, "useradd", &args)?;
    set_password(&root, &account.username, account.password.expose())?;
    install.progress(Step::Accounts, 2, PARTS);

    match &account.root_password {
        Some(password) => set_password(&root, "root", password.expose())?,
        None => {
            run::chroot(&root, "passwd", &["-l", "root"])?;
        }
    }
    install.progress(Step::Accounts, 3, PARTS);

    target::write(&root, SUDOERS_FILE, SUDOERS, 0o440)?;
    // The live image carries /etc/sudoers as 0644 (sudo's package makes it
    // 0440), and `visudo -c` refuses a sudoers file with any other mode, so
    // it gets its packaged mode back before the whole configuration, the
    // drop-in included, is checked.
    target::set_mode(&root, "etc/sudoers", 0o440)?;
    run::chroot(&root, "visudo", &["-c"])?;
    install.progress(Step::Accounts, 4, PARTS);

    user_directories(&root, &account.username, &config.region.locale);
    install.progress(Step::Accounts, 5, PARTS);
    Ok(())
}

/// The arguments that make the person's folders (Documents, Downloads…)
/// named in their language, run as them: a cleared environment with their
/// home and language, so neither the worker's `C` locale nor root's home
/// reaches the program.
pub fn user_dirs_args(user: &str, locale: &str) -> Vec<String> {
    vec![
        "-u".into(),
        user.into(),
        "--".into(),
        "/usr/bin/env".into(),
        "-i".into(),
        format!("HOME=/home/{user}"),
        format!("LANG={locale}"),
        "PATH=/usr/bin".into(),
        "/usr/bin/xdg-user-dirs-update".into(),
    ]
}

/// Makes the person's folders now, as the legacy installer did, when the
/// image has the tool. The session makes them at first login anyway, so a
/// failure here is only logged.
fn user_directories(root: &Path, user: &str, locale: &str) {
    let present = [
        "usr/bin/xdg-user-dirs-update",
        "usr/bin/runuser",
        "usr/bin/env",
    ]
    .iter()
    .all(|program| target::exists(root, program));
    if !present {
        run::log("accounts: the person's folders are made at first login");
        return;
    }
    let args = user_dirs_args(user, locale);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Err(stop) = run::chroot(root, "runuser", &args) {
        run::log(&format!(
            "accounts: the person's folders are made at first login: {}",
            stop.detail
        ));
    }
}

/// Whether the person's home is one the installer made, and so wholly
/// theirs: always, unless it is on a /home that was kept — the easy path's
/// separate one, or one a hand-made layout keeps (`home_kept`) — whose
/// files are the person's own from before and are left as they are.
pub fn home_is_new(config: &InstallConfig) -> bool {
    !config.home_kept()
}

/// `chown`'s arguments for giving `user` everything in their home, links
/// included as links and never followed out of it.
pub fn chown_home_args(user: &str) -> Vec<String> {
    vec![
        "-R".into(),
        "-h".into(),
        "--".into(),
        format!("{user}:"),
        format!("/home/{user}"),
    ]
}

/// At the end of the installation, everything in a home the installer made
/// is given to its person. Package scripts that run once the account exists
/// can leave files in it that belong to root — kinexin-desktop copies the
/// skeleton in as root and hands back only part of it (115 files in the
/// QEMU run) — and those cannot then be changed from the desktop. A failure
/// is logged, not fatal: the system is installed either way.
pub fn give_home_back(root: &Path, config: &InstallConfig) {
    give_home_back_by(config, &mut |program, args| {
        run::chroot(root, program, args).map(|_| ())
    });
}

/// [`give_home_back`], its command run inside the target by `chroot`
/// (`run::chroot`, which a test replaces with a recorder that runs
/// nothing): none at all for a home on a `/home` kept from before.
pub(crate) fn give_home_back_by(config: &InstallConfig, chroot: &mut Chroot) {
    if !home_is_new(config) {
        return;
    }
    let args = chown_home_args(&config.account.username);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Err(stop) = chroot("chown", &args) {
        run::log(&format!(
            "accounts: the home was not given back to its person: {}",
            stop.detail
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{Home, Target};
    use crate::model::secret::Secret;
    use crate::worker::partition::fixtures::{config, disk_empty};

    #[test]
    fn a_home_the_installer_made_is_given_back_but_a_kept_one_is_left() {
        let mut fresh = config(disk_empty(512), Target::WholeDisk);
        assert!(home_is_new(&fresh));
        fresh.storage.home = Some(Home {
            path: "/dev/sdb1".into(),
            format: true,
            disk: None,
        });
        assert!(home_is_new(&fresh));
        let mut kept = fresh.clone();
        kept.storage.home.as_mut().unwrap().format = false;
        assert!(!home_is_new(&kept));
        assert_eq!(
            chown_home_args("zoe"),
            ["-R", "-h", "--", "zoe:", "/home/zoe"]
        );
    }

    /// **The person's home, and the live account's, are judged by
    /// `home_kept()`** (Unit Q, Q5): on every request of the easy path —
    /// each fixture disk at 512 and 4096 bytes a sector, every place on it,
    /// with no separate /home, one formatted and one kept — exactly as the
    /// expression the System, Accounts and Desktop steps each wrote out
    /// before; and under a hand-made layout, a partition kept (or only
    /// shrunk) at `/home` is the person's from before — its `/home/liveuser`
    /// not removed, the new home not given away with `chown -R`, its Plasma
    /// launchers not rewritten — while one formatted there is new.
    #[test]
    fn a_kept_home_is_judged_alike_on_the_easy_path_and_under_a_layout() {
        use crate::model::layout::{DiskLayout, Fate, Fs, MountPoint, Part};
        use crate::worker::partition::fixtures::{all, disk_gpt_plain, targets};
        let before = |config: &InstallConfig| {
            !config
                .storage
                .home
                .as_ref()
                .is_some_and(|home| !home.format)
        };
        for sector in [512, 4096] {
            for (name, disk) in all(sector) {
                for (place, target) in targets(&disk) {
                    let mut easy = config(disk.clone(), target);
                    for home in [None, Some(true), Some(false)] {
                        easy.storage.home = home.map(|format| Home {
                            path: "/dev/vdy1".into(),
                            format,
                            disk: None,
                        });
                        assert_eq!(
                            home_is_new(&easy),
                            before(&easy),
                            "{name} {sector} {place} {home:?}"
                        );
                        assert_eq!(easy.home_kept(), !before(&easy));
                    }
                }
            }
        }
        let disk = disk_gpt_plain(512);
        let laid = |fate: Fate| {
            let was = &disk.partitions[0];
            let mut config = config(disk.clone(), Target::WholeDisk);
            config.target = Target::Layout {
                disks: vec![DiskLayout {
                    disk: disk.clone(),
                    table: None,
                    parts: vec![Part {
                        path: was.path.clone(),
                        number: was.number,
                        start: was.start,
                        size: was.size,
                        fate,
                    }],
                    create: Vec::new(),
                }],
            };
            config
        };
        let home = Some(MountPoint::Home);
        let kept = laid(Fate::Keep {
            mount: home.clone(),
        });
        assert!(kept.home_kept() && !home_is_new(&kept));
        let shrunk = laid(Fate::Shrink {
            to: 20 * crate::model::disks::GIB,
            mount: home.clone(),
        });
        assert!(shrunk.home_kept() && !home_is_new(&shrunk));
        let formatted = laid(Fate::Format {
            fs: Fs::Ext4,
            label: String::new(),
            size: disk.partitions[0].size,
            mount: home,
        });
        assert!(!formatted.home_kept() && home_is_new(&formatted));
        let elsewhere = laid(Fate::Keep {
            mount: Some(MountPoint::Other("/data".into())),
        });
        assert!(!elsewhere.home_kept() && home_is_new(&elsewhere));
    }

    fn account() -> Account {
        Account {
            full_name: "Zoë Example".into(),
            username: "zoe".into(),
            password: Secret::new("correct horse: battery"),
            hostname: "example-pc".into(),
            root_password: None,
        }
    }

    #[test]
    fn entries_are_matched_by_their_whole_name() {
        let passwd = "root:x:0:0::/root:/bin/bash\nliveuser:x:1000:1000::/home/liveuser:/bin/zsh\n";
        assert!(has_entry(passwd, "liveuser"));
        assert!(has_entry(passwd, "root"));
        assert!(!has_entry(passwd, "live"));
        assert!(!has_entry(passwd, "x"));
    }

    #[test]
    fn hosts_get_this_computers_name_and_keep_the_rest() {
        let live = "# Static table lookup for hostnames.\n127.0.0.1 localhost\n::1 localhost\n127.0.1.1 archiso.localdomain archiso\n";
        assert_eq!(
            hosts(live, "example-pc"),
            "# Static table lookup for hostnames.\n127.0.0.1 localhost\n::1 localhost\n127.0.1.1\texample-pc\n"
        );
        assert_eq!(
            hosts("# empty\n", "example-pc"),
            "# empty\n127.0.0.1\tlocalhost\n::1\t\tlocalhost\n127.0.1.1\texample-pc\n"
        );
        let again = hosts(&hosts(live, "example-pc"), "example-pc");
        assert_eq!(again.matches("127.0.1.1").count(), 1);
    }

    #[test]
    fn the_account_gets_the_legacy_groups_without_input() {
        let group = "root:x:0:\nwheel:x:998:\naudio:x:995:\nvideo:x:985:\ninput:x:994:\nstorage:x:988:\npower:x:989:\n";
        let groups = present_groups(group).unwrap();
        assert_eq!(groups, ["wheel", "audio", "video", "storage", "power"]);
        assert!(present_groups("root:x:0:\n").is_err());
        let args = useradd_args(&account(), &groups);
        assert_eq!(
            args,
            [
                "-m",
                "-c",
                "Zoë Example",
                "-G",
                "wheel,audio,video,storage,power",
                "-s",
                "/bin/bash",
                "zoe"
            ]
        );
        assert!(!args.iter().any(|a| a.contains("input")));
    }

    #[test]
    fn a_password_travels_only_on_standard_input() {
        let account = account();
        let line = PasswordLine::new(&account.username, account.password.expose());
        assert_eq!(line.bytes(), b"zoe:correct horse: battery\n");
        let (program, args) = chpasswd_command("/run/lxb-installer/stage-x/root");
        // What the worker logs for a command is its program and arguments.
        let logged = format!("$ {program} {}", args.join(" "));
        assert!(!logged.contains("correct horse"));
        assert!(!logged.contains("battery"));
        assert_eq!(
            logged,
            "$ arch-chroot /run/lxb-installer/stage-x/root /usr/bin/chpasswd"
        );
    }

    #[test]
    fn a_password_sent_to_a_program_never_reaches_the_log() {
        static LINES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
        run::set_log(Box::new(|line| {
            LINES
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(line.to_string())
        }));
        let line = PasswordLine::new("zoe", "unguessable-7f3a");
        // The worst case: a program that repeats its input on standard error
        // and fails.
        let result = run::run_input("sh", &["-c", "cat >&2; exit 3"], line.bytes(), true);
        let detail = result.expect_err("the program fails").detail;
        assert!(!detail.contains("unguessable"));
        let logged = LINES.lock().unwrap_or_else(|p| p.into_inner());
        assert!(logged.iter().any(|l| l.starts_with("$ sh -c")));
        assert!(!logged.iter().any(|l| l.contains("unguessable")));
    }

    #[test]
    fn the_password_buffer_is_made_once_at_its_final_size() {
        let line = PasswordLine::new("zoe", "secret");
        assert_eq!(line.0.capacity(), line.0.len());
    }

    #[test]
    fn folders_are_made_in_the_persons_language_as_the_person() {
        let args = user_dirs_args("zoe", "pl_PL.UTF-8");
        assert_eq!(&args[..3], ["-u", "zoe", "--"]);
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"HOME=/home/zoe".to_string()));
        assert!(args.contains(&"LANG=pl_PL.UTF-8".to_string()));
        assert_eq!(args.last().unwrap(), "/usr/bin/xdg-user-dirs-update");
    }
}
