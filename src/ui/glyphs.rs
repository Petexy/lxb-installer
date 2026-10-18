//! The installer's own marks, and which mark each disk, place and field of
//! Where to install and You wears.
//!
//! The toolkit carries the shell's marks; what the shell has no mark for —
//! a kind of disk, a place on one, a password typed twice, a computer's name
//! — the installer draws itself, in `data/glyphs/`, the way every mark of the
//! set is drawn: a white shape saying `lxb:shape`, its openings cut out of it,
//! two units of its thirty-two clear all round. The toolkit measures each one
//! into the same atlas as its own (`App::add_mark`) and shades it with the
//! same material, so a row's mark here stands beside the shell's as one of the
//! set — as Review's rows do with the shell's own.
//!
//! Where the shell does have a mark for a row (the name card, the at sign, the
//! key, the shield, the magnifier), the row wears the shell's, from the
//! toolkit.

use lxb_app::App;

use crate::model::disks::{Disk, Transport};

/// An NVMe drive: the M.2 stick.
pub const DISK_NVME: &str = "disk-nvme";
/// A SATA SSD: the 2.5-inch case.
pub const DISK_SSD: &str = "disk-ssd";
/// A hard disk: the drive with its lid off, the platter and the arm.
pub const DISK_HDD: &str = "disk-hdd";
/// A USB stick.
pub const DISK_USB: &str = "disk-usb";
/// A memory card (SD, or the eMMC a small machine keeps its system on).
pub const DISK_CARD: &str = "disk-card";
/// The disk Linexin goes onto: Where to install's own mark, and Review's for
/// the row that goes back to it.
pub const DISK_TARGET: &str = "disk-target";
/// Partitions, Where to install's Advanced page: a disk's map, three tiles
/// with their bands.
pub const DISK_PARTITIONS: &str = "disk-partitions";
/// Erase this disk.
pub const PLACE_ERASE: &str = "place-erase";
/// The free space on a disk.
pub const PLACE_FREE: &str = "place-free";
/// A part of a disk replaced.
pub const PLACE_REPLACE: &str = "place-replace";
/// Something still missing before Continue: the mark in front of what
/// Partitions' foot says.
pub const WARNING: &str = "warning";
/// A password typed a second time.
pub const PASSWORD_AGAIN: &str = "field-password-again";
/// The computer's name.
pub const COMPUTER: &str = "field-computer";

/// The shell's marks You's rows wear, from the toolkit: the name card, the
/// at sign every machine means "the name you sign in as" by, the key, and the
/// shield of an account that may change the system.
pub const FULL_NAME: &str = "setting-name";
pub const USERNAME: &str = "setting-username";
pub const PASSWORD: &str = "setting-password";
pub const ADMINISTRATOR: &str = "setting-account-type";

/// The shell's magnifier, which its own searches wear at their head (the
/// library's, Settings', the file picker's), from the toolkit: the search
/// field of Language, Time zone and Keyboard.
pub const SEARCH: &str = "search";

/// Every mark of the installer's own, by name, with its drawing.
pub const OWN: [(&str, &[u8]); 13] = [
    (DISK_NVME, include_bytes!("../../data/glyphs/disk-nvme.svg")),
    (DISK_SSD, include_bytes!("../../data/glyphs/disk-ssd.svg")),
    (DISK_HDD, include_bytes!("../../data/glyphs/disk-hdd.svg")),
    (DISK_USB, include_bytes!("../../data/glyphs/disk-usb.svg")),
    (DISK_CARD, include_bytes!("../../data/glyphs/disk-card.svg")),
    (
        DISK_TARGET,
        include_bytes!("../../data/glyphs/disk-target.svg"),
    ),
    (
        DISK_PARTITIONS,
        include_bytes!("../../data/glyphs/disk-partitions.svg"),
    ),
    (
        PLACE_ERASE,
        include_bytes!("../../data/glyphs/place-erase.svg"),
    ),
    (
        PLACE_FREE,
        include_bytes!("../../data/glyphs/place-free.svg"),
    ),
    (
        PLACE_REPLACE,
        include_bytes!("../../data/glyphs/place-replace.svg"),
    ),
    (WARNING, include_bytes!("../../data/glyphs/warning.svg")),
    (
        PASSWORD_AGAIN,
        include_bytes!("../../data/glyphs/field-password-again.svg"),
    ),
    (
        COMPUTER,
        include_bytes!("../../data/glyphs/field-computer.svg"),
    ),
];

/// The application with the installer's own marks added to it, for the
/// window and for every picture alike.
///
/// A mark the toolkit refuses is said on the error stream and left out
/// rather than stopping the installer: where it was refused because the
/// toolkit has come to carry a mark by that name, the row wears the toolkit's
/// drawing, which is the set's own; anything else leaves the row without a
/// mark, which loses nothing it needs. The tests hold every drawing here to
/// being one the toolkit takes.
pub fn added(mut app: App) -> App {
    for (name, svg) in OWN {
        if let Err(refused) = app.add_mark(name, svg) {
            eprintln!("lxb-installer: the {name} mark is not drawn: {refused}");
        }
    }
    app
}

/// The mark a disk's row wears: the kind of disk it is. By how it is
/// connected first — an NVMe drive, a stick on USB, a memory card are each
/// one thing whatever is inside them, and a virtual disk is solid state
/// whatever it says — and otherwise by whether it spins: a hard disk, or a
/// solid-state one (SATA, or an unknown disk, which is what most of them are
/// now).
///
/// A virtual disk's word on spinning is the hypervisor's default, not the
/// disk behind it: Linux's virtio_blk says every disk spins (it sets
/// `BLK_FEAT_ROTATIONAL`), so a QEMU guest's disk reads `ROTA` 1 even on an
/// NVMe drive. It wears the solid-state mark the docs promise it.
pub fn disk(disk: &Disk) -> &'static str {
    match disk.transport {
        Transport::Nvme => DISK_NVME,
        Transport::Usb => DISK_USB,
        Transport::Mmc => DISK_CARD,
        Transport::Virtio => DISK_SSD,
        Transport::Sata | Transport::Other if disk.rotational => DISK_HDD,
        Transport::Sata | Transport::Other => DISK_SSD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lxb_app::lxb_render::Mark;

    #[test]
    fn every_mark_of_the_installers_own_is_a_shape_the_toolkit_takes() {
        let mut app = App::new("io.github.petexy.lxb-installer", "Test");
        for (name, svg) in OWN {
            assert!(Mark::measure(svg).is_ok(), "{name} is not a shape");
            assert!(
                lxb_app::lxb_toolkit::assets::glyph(name).is_none(),
                "{name} is one of the toolkit's own marks now: draw the set's"
            );
            app.add_mark(name, svg)
                .unwrap_or_else(|refused| panic!("{name}: {refused}"));
        }
    }

    #[test]
    fn every_drawing_is_drawn_as_the_set_is() {
        for (name, svg) in OWN {
            let source = std::str::from_utf8(svg).expect("a drawing is text");
            assert!(source.contains("lxb:shape"), "{name} does not say so");
            assert!(
                source.contains(r#"viewBox="0 0 32 32""#),
                "{name} is not drawn on the set's thirty-two units"
            );
            for forbidden in [
                "<linearGradient",
                "<radialGradient",
                "<filter",
                "<text",
                "stop-color",
                "rgb(",
                "hsl(",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "{name} carries material of its own through {forbidden}"
                );
            }
            for attribute in ["fill=\"", "stroke=\""] {
                let mut rest = source;
                while let Some(index) = rest.find(attribute) {
                    rest = &rest[index + attribute.len()..];
                    let value = rest.split_once('"').expect("a closed attribute").0;
                    assert!(
                        matches!(value, "none" | "#fff" | "#ffffff" | "#000000"),
                        "{name} is coloured: {attribute}{value}"
                    );
                }
            }
        }
        let mut names: Vec<&str> = OWN.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), OWN.len(), "two marks share a name");
    }

    #[test]
    fn the_shells_marks_the_installer_wears_are_in_the_toolkit() {
        for name in [FULL_NAME, USERNAME, PASSWORD, ADMINISTRATOR, SEARCH] {
            assert!(
                lxb_app::lxb_toolkit::assets::glyph(name).is_some(),
                "{name}"
            );
        }
    }

    #[test]
    fn a_disk_wears_the_kind_of_disk_it_is() {
        let mut disks = crate::ui::demo::disks();
        let [nvme, sata, stick] = [disks[0].clone(), disks[1].clone(), disks[2].clone()];
        assert_eq!(disk(&nvme), DISK_NVME);
        assert_eq!(disk(&sata), DISK_HDD, "the made-up SATA disk spins");
        assert_eq!(disk(&stick), DISK_USB);
        let mut ssd = sata.clone();
        ssd.rotational = false;
        assert_eq!(disk(&ssd), DISK_SSD);
        let card = &mut disks[0];
        card.transport = Transport::Mmc;
        assert_eq!(disk(card), DISK_CARD);
        card.transport = Transport::Virtio;
        assert_eq!(disk(card), DISK_SSD);
        // virtio_blk says every disk spins: a virtual disk is still solid
        // state, as a QEMU guest's is.
        card.rotational = true;
        assert_eq!(disk(card), DISK_SSD);
        card.transport = Transport::Other;
        assert_eq!(disk(card), DISK_HDD);
        // A stick that says it spins, as many do, is still a stick.
        let mut spinning = stick;
        spinning.rotational = true;
        assert_eq!(disk(&spinning), DISK_USB);
    }
}
