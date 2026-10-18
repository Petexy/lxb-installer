//! The Linexin system installer.
//!
//! Two processes share this crate. The interface (`ui`) runs as the person
//! at the screen, reads the machine without changing it and builds one
//! [`model::protocol::Request`]. The worker (`worker`) runs as root through
//! polkit, checks that request again from the start, and is the only code
//! that writes to a disk. `docs/design.md` describes the design. The
//! sections, units, steps and decisions the comments cite (§7.0, Unit Q,
//! step Q9, decision 20) are those of the maintainers' notes,
//! `INSTALLER-HANDOFF.md`, and the QEMU harness they name (`tests/e2e`, its
//! scenarios and `test_unit_q.py`) is the maintainers' own too.
pub mod i18n;
pub mod launch;
pub mod model;
pub mod ui;
pub mod worker;
