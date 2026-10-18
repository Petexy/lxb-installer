//! A plan as sfdisk input and as the exact command lines that write it.
//!
//! The worker runs these commands against the disk; the image tests run the
//! very same ones against sparse image files, so what is tested is what is
//! run. Every sfdisk call takes the device's lock (`--lock`), which keeps udev
//! from probing the disk halfway through a write.
use super::plan::{Change, Plan, Role};
use super::table::OnDisk;
use crate::model::disks::Table;

/// One program run with its arguments and, for sfdisk scripts, its input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub program: &'static str,
    pub args: Vec<String>,
    pub input: Option<String>,
}

impl Command {
    /// Every program the worker runs is named as a literal here or in a
    /// `run::run` call, which `packaging/check.sh` reads; the executor of a
    /// hand-made layout (`worker::manual`) builds its commands with it too.
    pub(crate) fn new(program: &'static str, args: &[&str]) -> Self {
        Self {
            program,
            args: args.iter().map(|a| a.to_string()).collect(),
            input: None,
        }
    }

    pub(crate) fn with_input(mut self, input: String) -> Self {
        self.input = Some(input);
        self
    }
}

/// The sfdisk script that creates the plan's new partitions, or `None` when
/// the plan creates none (a re-typed partition).
///
/// Starts and sizes are whole sectors, so libfdisk places them exactly where
/// the plan says and the worker can find them again by their start. The
/// `sector-size` header states the unit: were the device's sector size ever
/// different, sfdisk converts the numbers so the byte positions still hold
/// rather than placing everything eight times too far. Names are quoted,
/// because libfdisk ends an unquoted value at the first space.
pub fn script(plan: &Plan) -> Option<String> {
    if plan.create.is_empty() {
        return None;
    }
    let mut text = String::new();
    if plan.change == Change::Fresh {
        text.push_str(match plan.table {
            Table::Gpt => "label: gpt\n",
            Table::Msdos => "label: dos\n",
        });
    }
    text.push_str("unit: sectors\n");
    text.push_str(&format!("sector-size: {}\n", plan.sector));
    for part in &plan.create {
        text.push_str(&format!(
            "start={}, size={}, type={}",
            part.start,
            part.size,
            part.role.part_type(plan.table)
        ));
        match plan.table {
            Table::Gpt => text.push_str(&format!(", name=\"{}\"", part.role.name())),
            // A fresh msdos table marks root active in the same write. On a
            // table that is kept, another partition may already be active,
            // which `activation` decides once the partitions exist.
            Table::Msdos if part.role == Role::Root && plan.change == Change::Fresh => {
                text.push_str(", bootable")
            }
            Table::Msdos => {}
        }
        text.push('\n');
    }
    Some(text)
}

/// The commands that carry the table half of the plan out on `device` (the
/// plan's own disk when the worker runs them), in order.
pub fn table_commands(plan: &Plan, device: &str) -> Vec<Command> {
    let mut commands = Vec::new();
    match &plan.change {
        Change::Fresh => {
            // Every signature on the disk goes first (an old GPT's backup
            // header at the end included), so nothing that was there can be
            // found again by a tool that probes the new table.
            commands.push(Command::new("wipefs", &["--all", "--lock", device]));
            if let Some(input) = script(plan) {
                commands.push(
                    Command::new(
                        "sfdisk",
                        &[
                            "--lock",
                            "--wipe",
                            "always",
                            "--wipe-partitions",
                            "always",
                            device,
                        ],
                    )
                    .with_input(input),
                );
            }
        }
        Change::Add => {
            if let Some(input) = script(plan) {
                commands.push(append(device).with_input(input));
            }
        }
        Change::Retype { number, .. } => {
            let number = number.to_string();
            let root = Role::Root;
            commands.push(Command::new(
                "sfdisk",
                &[
                    "--lock",
                    "--part-type",
                    device,
                    &number,
                    root.part_type(plan.table),
                ],
            ));
            if plan.table == Table::Gpt {
                commands.push(Command::new(
                    "sfdisk",
                    &["--lock", "--part-label", device, &number, root.name()],
                ));
            }
        }
        Change::Replace { number, .. } => {
            let number = number.to_string();
            commands.push(Command::new(
                "sfdisk",
                &["--lock", "--delete", device, &number],
            ));
            if let Some(input) = script(plan) {
                commands.push(append(device).with_input(input));
            }
        }
    }
    commands
}

/// New partitions added to a table that is kept. Old signatures inside the
/// new partitions are wiped, so a filesystem that once lived there cannot
/// be found beside the new one.
fn append(device: &str) -> Command {
    Command::new(
        "sfdisk",
        &["--lock", "--append", "--wipe-partitions", "always", device],
    )
}

/// On a kept msdos table, the command that marks root active, or `None`.
///
/// Only when no partition is active already: GRUB in the MBR does not look
/// at the flag, so moving it away from another system (Windows keeps it on
/// its own partition) would only matter if the installation stopped before
/// GRUB was written, and then that system could no longer start. A logical
/// partition (5 and up) cannot be started by the flag, so it is left alone.
pub fn activation(
    plan: &Plan,
    on_disk: &OnDisk,
    root_number: u32,
    device: &str,
) -> Option<Command> {
    let wanted = plan.wants_boot_flag()
        && plan.change != Change::Fresh
        && (1..=4).contains(&root_number)
        && !on_disk.partitions.iter().any(|p| p.bootable);
    wanted.then(|| {
        Command::new(
            "sfdisk",
            &["--lock", "--activate", device, &root_number.to_string()],
        )
    })
}
