//! Which firmware started the machine: what the boot rules of both halves
//! hang on — the partition tables a disk may be given, whether an EFI system
//! partition or a `bios_grub` one is needed, which boot loaders can be used.
//!
//! Plain data, so the interface and the rule set (`model::layout`) can judge
//! by it without reaching into the worker. Reading it from the running
//! machine is the worker's own (`Firmware::detect`, in `worker::layout`),
//! which the interface asks once as it starts.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Firmware {
    Uefi,
    Bios,
}
