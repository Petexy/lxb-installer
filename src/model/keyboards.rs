//! Whether a keyboard somebody can type on is attached: what decides whether
//! the Keyboard step offers its field to try the layout in ("Test keyboard"
//! is not offered when there is no keyboard — an installation with only a
//! controller never sees it). Read-only, and the interface's alone.
//!
//! The kernel lists every input device in `/proc/bus/input/devices`, one file,
//! world-readable, rewritten on every read — so a keyboard plugged in or
//! pulled out shows at the next reading. It is read as text, so that the
//! reading can be tested on text written by hand (as LineXinBar reads its
//! controllers from the same file).
//!
//! **A device counts when it says it has every letter key, A to Z.** A power
//! or sleep button, a lid switch, the volume and brightness keys, a headset's
//! buttons, a mouse, a touchpad and a pad never do; a keyboard's own extra
//! interfaces (its media keys, its system keys) do not either, while the
//! interface that types does. Some devices that declare every letter are not
//! keyboards:
//!
//! * **Valve's controllers type in lizard mode.** A Steam Controller, the 2026
//!   controller's puck and the Steam Deck's built-in controls each bring a
//!   firmware keyboard (A is Enter, B Escape, the D-pad the arrows) that
//!   declares the whole of a keyboard's keys: measured on a puck, which lists
//!   four keyboards with all twenty-six letters. Valve makes no keyboard to
//!   type on, so nothing of Valve's counts.
//! * **A handheld's own controls are no keyboard, and nor is its PS/2 one.**
//!   A handheld with no keyboard of its own is known by its controls' ids,
//!   the ones the kernel's own driver for it is bound by (`HANDHELDS`): a
//!   Steam Deck, an ROG Ally or Ally X, a Legion Go or a Legion Go S. The
//!   firmware keyboards its controls bring do not count, and nor does the
//!   PS/2 keyboard, which on a machine with no keyboard of its own can only
//!   be its buttons: a Steam Deck sends its volume buttons through an
//!   emulated "AT Translated Set 2 keyboard" — reported on Deck hardware
//!   (KDE's plasma-mobile issue 328), not measured here — and the PS/2
//!   driver declares every key of its table whatever is behind it. A
//!   keyboard plugged into the handheld counts. Everywhere else the PS/2
//!   keyboard is a laptop's own, or the one QEMU gives every guest, and
//!   counts.
//!
//! What this cannot tell apart is a device that declares a keyboard's letters
//! and has few keys or none. A gaming mouse whose buttons can send keys is
//! listed as one device with a keyboard's letters and a mouse's motion and
//! buttons (seen on a real one, on its wireless receiver) — and a keyboard
//! with a touchpad of its own can be listed just so, and is somebody's only
//! keyboard on a sofa. Nor can it know a handheld whose controls' ids no
//! driver names. With one of those the field is offered — which costs a field
//! nobody types in, never a field somebody needs.

/// Where the kernel lists every input device it has.
pub const DEVICES: &str = "/proc/bus/input/devices";

/// The letter keys' codes in the kernel's numbering (`input-event-codes.h`):
/// the rows Q to P, A to L and Z to M.
const LETTERS: [usize; 26] = [
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, // Q W E R T Y U I O P
    30, 31, 32, 33, 34, 35, 36, 37, 38, // A S D F G H J K L
    44, 45, 46, 47, 48, 49, 50, // Z X C V B N M
];

/// The kind of event a key press is (`EV_KEY`).
const EV_KEY: usize = 1;

/// The bus a PS/2 device is on (`BUS_I8042`).
const BUS_I8042: u16 = 0x0011;

/// Valve's vendor id.
const VALVE: u16 = 0x28de;

/// The built-in controls of handhelds that have no keyboard of their own, as
/// vendor and product: the ids the kernel's own driver for each is bound by,
/// which `modinfo` lists as its aliases. OneXPlayer's (`hid-oxp`) are not
/// among them: some of its machines take a keyboard that clips on, which may
/// answer to the same ids, and a PS/2 keyboard left out there could be one.
const HANDHELDS: [(u16, u16); 9] = [
    // A Steam Deck's (`hid-steam`), and LineXinBar's stand-in for them,
    // which keeps their ids.
    (VALVE, 0x1205),
    // An ROG Ally's and an Ally X's (`hid-asus`, which names them
    // `ROG_NKEY_ALLY` and `ROG_NKEY_ALLY_X`).
    (0x0b05, 0x1abe),
    (0x0b05, 0x1b4c),
    // The Legion Go's (`hid-lenovo-go`, "Lenovo Legion Go Series Gamepads").
    (0x17ef, 0x61eb),
    (0x17ef, 0x61ec),
    (0x17ef, 0x61ed),
    (0x17ef, 0x61ee),
    // The Legion Go S's (`hid-lenovo-go-s`).
    (0x1a86, 0xe310),
    (0x1a86, 0xe311),
];

/// Whether a keyboard with letter keys is attached now. Where the list cannot
/// be read, yes: the field is offered as it always was, rather than taken
/// from somebody who may well have a keyboard.
pub fn attached() -> bool {
    answer(std::fs::read_to_string(DEVICES))
}

/// [`attached`] on what reading the list gave: the listing judged, or yes
/// where it could not be read. Apart from the reading, so a test holds it
/// without reading this machine.
fn answer(read: std::io::Result<String>) -> bool {
    match read {
        Ok(listing) => typing(&listing),
        Err(_) => true,
    }
}

/// [`attached`] on the text it reads: whether `listing`, in the shape of
/// `/proc/bus/input/devices`, holds a keyboard to type on.
pub fn typing(listing: &str) -> bool {
    let devices: Vec<Device> = listing.split("\n\n").map(Device::of).collect();
    let handheld = devices.iter().any(Device::is_handhelds);
    devices.iter().any(|device| {
        device.has_letters()
            && device.vendor != VALVE
            && !device.is_handhelds()
            && !(handheld && device.is_ps2())
    })
}

/// One device of the listing: what it is on and who made it, and the event
/// kinds and keys it says it has, as the kernel printed them.
#[derive(Debug, Default)]
struct Device<'a> {
    bus: u16,
    vendor: u16,
    product: u16,
    events: &'a str,
    keys: &'a str,
}

impl<'a> Device<'a> {
    /// The device a block of the listing describes: its `I:` line's ids and
    /// its `B: EV=` and `B: KEY=` bitmaps. What a block does not say is left
    /// at nought or empty, which no test below passes.
    fn of(block: &'a str) -> Self {
        let mut device = Device::default();
        for line in block.lines() {
            let line = line.trim();
            if let Some(ids) = line.strip_prefix("I:") {
                for field in ids.split_whitespace() {
                    let Some((key, value)) = field.split_once('=') else {
                        continue;
                    };
                    let value = u16::from_str_radix(value, 16).unwrap_or(0);
                    match key {
                        "Bus" => device.bus = value,
                        "Vendor" => device.vendor = value,
                        "Product" => device.product = value,
                        _ => {}
                    }
                }
            } else if let Some(bitmap) = line.strip_prefix("B:") {
                let bitmap = bitmap.trim();
                if let Some(events) = bitmap.strip_prefix("EV=") {
                    device.events = events;
                } else if let Some(keys) = bitmap.strip_prefix("KEY=") {
                    device.keys = keys;
                }
            }
        }
        device
    }

    /// Whether it sends key presses and says it has every letter.
    fn has_letters(&self) -> bool {
        bit(self.events, EV_KEY) && LETTERS.iter().all(|code| bit(self.keys, *code))
    }

    /// Whether it is a handheld's own controls: what says this machine is
    /// one.
    fn is_handhelds(&self) -> bool {
        HANDHELDS.contains(&(self.vendor, self.product))
    }

    /// Whether it is on the PS/2 port.
    fn is_ps2(&self) -> bool {
        self.bus == BUS_I8042
    }
}

/// Whether bit `code` is set in a bitmap as the kernel prints one: a row of
/// machine words in hexadecimal, the highest first, nothing padded and the
/// words above the last set bit left out — so the last word is always the
/// lowest, and counting back from the end is the only way to read it. A word
/// is as wide as this machine's, which is the width the kernel wrote it at.
fn bit(bitmap: &str, code: usize) -> bool {
    let width = usize::BITS as usize;
    let words: Vec<&str> = bitmap.split_whitespace().collect();
    let Some(at) = words.len().checked_sub(1 + code / width) else {
        return false;
    };
    u64::from_str_radix(words[at], 16).is_ok_and(|word| (word >> (code % width)) & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every listing below is written by hand in the kernel's shape, from the
    // shapes real machines report, with made-up places and serials: none of
    // them is any one machine's. The bitmaps are the kernel's own for each
    // kind of device — the PS/2 driver's full table, a USB keyboard's boot
    // set, a headset's few media keys.

    /// The PS/2 keyboard's lines, as the kernel lists a laptop's own and the
    /// one QEMU gives a guest, and as a Steam Deck lists its volume buttons.
    const PS2_KEYBOARD: &str = "\
I: Bus=0011 Vendor=0001 Product=0001 Version=ab41
N: Name=\"AT Translated Set 2 keyboard\"
P: Phys=isa0060/serio0/input0
S: Sysfs=/devices/platform/i8042/serio0/input/input3
U: Uniq=
H: Handlers=sysrq kbd leds event3
B: PROP=0
B: EV=120013
B: KEY=402000000 3803078f800d001 feffffdfffefffff fffffffffffffffe
B: MSC=10
B: LED=7
";

    const POWER_BUTTON: &str = "\
I: Bus=0019 Vendor=0000 Product=0001 Version=0000
N: Name=\"Power Button\"
P: Phys=LNXPWRBN/button/input0
S: Sysfs=/devices/LNXSYSTM:00/LNXPWRBN:00/input/input1
U: Uniq=
H: Handlers=kbd event1
B: PROP=0
B: EV=3
B: KEY=10000000000000 0
";

    const LID_SWITCH: &str = "\
I: Bus=0019 Vendor=0000 Product=0005 Version=0000
N: Name=\"Lid Switch\"
P: Phys=PNP0C0D/button/input0
S: Sysfs=/devices/LNXSYSTM:00/LNXSYBUS:00/PNP0C0D:00/input/input0
U: Uniq=
H: Handlers=event0
B: PROP=0
B: EV=21
B: SW=1
";

    /// The brightness keys of a laptop's display.
    const VIDEO_BUS: &str = "\
I: Bus=0019 Vendor=0000 Product=0006 Version=0000
N: Name=\"Video Bus\"
P: Phys=LNXVIDEO/video/input0
S: Sysfs=/devices/LNXSYSTM:00/LNXSYBUS:00/PNP0A08:00/LNXVIDEO:00/input/input4
U: Uniq=
H: Handlers=kbd event4
B: PROP=0
B: EV=3
B: KEY=3e000b00000000 0 0 0
";

    const TOUCHPAD: &str = "\
I: Bus=0018 Vendor=0000 Product=0000 Version=0000
N: Name=\"Example Touchpad\"
P: Phys=i2c-TEST0001:00
S: Sysfs=/devices/platform/TEST0001:00/input/input6
U: Uniq=
H: Handlers=mouse0 event6
B: PROP=5
B: EV=1b
B: KEY=e520 10000 0 0 0 0
B: ABS=2e0800000000003
B: MSC=20
";

    /// A headset's volume and play keys.
    const HEADSET: &str = "\
I: Bus=0003 Vendor=f00d Product=0001 Version=0100
N: Name=\"Example Wireless Headset\"
P: Phys=usb-0000:00:14.0-2/input3
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-2/1-2:1.3/0003:F00D:0001.0004/input/input8
U: Uniq=
H: Handlers=kbd event8
B: PROP=0
B: EV=13
B: KEY=1000000000 c000000000000 0
B: MSC=10
";

    /// A pad as xpad lists one: buttons, never a key.
    const PAD: &str = "\
I: Bus=0003 Vendor=045e Product=028e Version=0114
N: Name=\"Microsoft X-Box 360 pad\"
P: Phys=usb-0000:00:14.0-3/input0
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-3/1-3:1.0/input/input9
U: Uniq=
H: Handlers=event9 js0
B: PROP=0
B: EV=20000b
B: KEY=7cdb000000000000 0 0 0 0
B: ABS=3003f
B: FF=107030000 0
";

    /// A USB keyboard of several interfaces, the shape an analogue gaming
    /// keyboard has: the one that types, its system keys, its media keys and
    /// a mouse of its own — and only the first has the letters.
    const USB_KEYBOARD: &str = "\
I: Bus=0003 Vendor=beef Product=0060 Version=0111
N: Name=\"Example Analog 60\"
P: Phys=usb-0000:00:14.0-4/input0
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-4/1-4:1.0/0003:BEEF:0060.0005/input/input10
U: Uniq=TEST0001
H: Handlers=sysrq kbd leds event10
B: PROP=0
B: EV=120013
B: KEY=1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe
B: MSC=10
B: LED=1f

I: Bus=0003 Vendor=beef Product=0060 Version=0111
N: Name=\"Example Analog 60 System Control\"
P: Phys=usb-0000:00:14.0-4/input2
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-4/1-4:1.2/0003:BEEF:0060.0006/input/input11
U: Uniq=TEST0001
H: Handlers=kbd event11
B: PROP=0
B: EV=13
B: KEY=c000 10000000000000 0
B: MSC=10

I: Bus=0003 Vendor=beef Product=0060 Version=0111
N: Name=\"Example Analog 60 Consumer Control\"
P: Phys=usb-0000:00:14.0-4/input2
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-4/1-4:1.2/0003:BEEF:0060.0006/input/input12
U: Uniq=TEST0001
H: Handlers=kbd event12
B: PROP=0
B: EV=1f
B: KEY=3f000303ff 0 0 483ffff17aff32d bfd4444600000000 1 130ff38b17d000 677bfad9415fed 9ed68000004400 10000002
B: REL=1040
B: ABS=100000000
B: MSC=10

I: Bus=0003 Vendor=beef Product=0060 Version=0111
N: Name=\"Example Analog 60 Mouse\"
P: Phys=usb-0000:00:14.0-4/input2
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-4/1-4:1.2/0003:BEEF:0060.0006/input/input13
U: Uniq=TEST0001
H: Handlers=event13 mouse1
B: PROP=0
B: EV=17
B: KEY=1f0000 0 0 0 0
B: REL=903
B: MSC=10
";

    /// The Steam Deck's own controls: the firmware keyboard and mouse of
    /// lizard mode, and the kernel's gamepad.
    const STEAM_DECK_CONTROLS: &str = "\
I: Bus=0003 Vendor=28de Product=1205 Version=0111
N: Name=\"Valve Software Steam Deck Controller\"
P: Phys=usb-0000:04:00.4-3/input0
S: Sysfs=/devices/pci0000:00/0000:00:08.1/0000:04:00.4/usb3/3-3/3-3:1.0/0003:28DE:1205.0001/input/input5
U: Uniq=
H: Handlers=sysrq kbd leds event5
B: PROP=0
B: EV=120013
B: KEY=1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe
B: MSC=10
B: LED=1f

I: Bus=0003 Vendor=28de Product=1205 Version=0111
N: Name=\"Valve Software Steam Deck Controller\"
P: Phys=usb-0000:04:00.4-3/input1
S: Sysfs=/devices/pci0000:00/0000:00:08.1/0000:04:00.4/usb3/3-3/3-3:1.1/0003:28DE:1205.0002/input/input6
U: Uniq=
H: Handlers=event6 mouse0
B: PROP=0
B: EV=17
B: KEY=30000 0 0 0 0
B: REL=903
B: MSC=10

I: Bus=0003 Vendor=28de Product=1205 Version=0111
N: Name=\"Steam Deck\"
P: Phys=usb-0000:04:00.4-3/input2
S: Sysfs=/devices/pci0000:00/0000:00:08.1/0000:04:00.4/usb3/3-3/3-3:1.2/0003:28DE:1205.0003/input/input7
U: Uniq=
H: Handlers=event7 js0
B: PROP=0
B: EV=b
B: KEY=7fdb000000000000 0 0 0 0
B: ABS=3273f
";

    /// One of the four keyboards a Steam Controller's puck lists, each with
    /// a keyboard's every key.
    const PUCK_KEYBOARD: &str = "\
I: Bus=0003 Vendor=28de Product=1304 Version=0111
N: Name=\"Valve Software Steam Controller Puck Keyboard\"
P: Phys=usb-0000:00:14.0-5/input1
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-5/1-5:1.1/0003:28DE:1304.0008/input/input20
U: Uniq=
H: Handlers=sysrq kbd event20
B: PROP=0
B: EV=100013
B: KEY=1000000000007 ff800000000007ff febeffdfffefffff fffffffffffffffe
B: MSC=10
";

    /// What QEMU gives a guest besides its PS/2 keyboard.
    const QEMU_POINTERS: &str = "\
I: Bus=0003 Vendor=0627 Product=0001 Version=0001
N: Name=\"QEMU QEMU USB Tablet\"
P: Phys=usb-0000:00:04.0-1/input0
S: Sysfs=/devices/pci0000:00/0000:00:04.0/usb1/1-1/1-1:1.0/0003:0627:0001.0001/input/input7
U: Uniq=28754-0000:00:04.0-1
H: Handlers=mouse1 event7
B: PROP=0
B: EV=1f
B: KEY=70000 0 0 0 0
B: REL=900
B: ABS=3
B: MSC=10

I: Bus=0011 Vendor=0002 Product=0013 Version=0006
N: Name=\"VirtualPS/2 VMware VMMouse\"
P: Phys=isa0060/serio1/input1
S: Sysfs=/devices/platform/i8042/serio1/input/input6
U: Uniq=
H: Handlers=mouse0 event6
B: PROP=0
B: EV=b
B: KEY=70000 0 0 0 0
B: ABS=3
";

    /// One device with a keyboard's letters and a mouse's motion and buttons:
    /// how a gaming mouse whose buttons can send keys is listed on its
    /// wireless receiver, and how a keyboard with a touchpad of its own can
    /// be. The keys are the USB keyboard's above, and a mouse's eight
    /// buttons beside them.
    const KEYS_AND_POINTER: &str = "\
I: Bus=0003 Vendor=cafe Product=0042 Version=0111
N: Name=\"Example Wireless Device\"
P: Phys=usb-0000:00:14.0-6/input2:1
S: Sysfs=/devices/pci0000:00/0000:00:14.0/usb1/1-6/1-6:1.2/0003:CAFE:0001.0009/0003:CAFE:0042.000A/input/input21
U: Uniq=
H: Handlers=sysrq kbd leds event21 mouse2
B: PROP=0
B: EV=12001f
B: KEY=ff0000 1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe
B: REL=1943
B: MSC=10
B: LED=1f
";

    /// A handheld's controls as `HANDHELDS` knows them by: a firmware
    /// keyboard declaring every key, under `vendor` and `product`.
    fn handhelds_keyboard(vendor: u16, product: u16) -> String {
        format!(
            "\
I: Bus=0003 Vendor={vendor:04x} Product={product:04x} Version=0111
N: Name=\"Example Handheld Controls Keyboard\"
P: Phys=usb-0000:05:00.3-2/input0
S: Sysfs=/devices/pci0000:00/0000:00:08.1/0000:05:00.3/usb3/3-2/3-2:1.0/input/input15
U: Uniq=
H: Handlers=sysrq kbd leds event15
B: PROP=0
B: EV=120013
B: KEY=1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe
B: MSC=10
B: LED=1f
"
        )
    }

    fn listing(devices: &[&str]) -> String {
        devices.join("\n")
    }

    #[test]
    fn a_laptops_own_keyboard_counts_and_its_buttons_and_switches_do_not() {
        let laptop = listing(&[LID_SWITCH, POWER_BUTTON, PS2_KEYBOARD, VIDEO_BUS, TOUCHPAD]);
        assert!(typing(&laptop));
        let without = listing(&[LID_SWITCH, POWER_BUTTON, VIDEO_BUS, TOUCHPAD]);
        assert!(!typing(&without), "a button or a touchpad counted");
    }

    #[test]
    fn a_computer_with_only_a_controller_has_no_keyboard() {
        let desk = listing(&[POWER_BUTTON, POWER_BUTTON, HEADSET, PAD]);
        assert!(!typing(&desk));
    }

    #[test]
    fn a_keyboard_of_several_interfaces_counts_by_the_one_that_types() {
        assert!(typing(&listing(&[POWER_BUTTON, USB_KEYBOARD, PAD])));
        // Its media keys, system keys and mouse alone are no keyboard.
        let extras = USB_KEYBOARD.split("\n\n").skip(1).collect::<Vec<_>>();
        assert_eq!(extras.len(), 3);
        assert!(!typing(&listing(&extras)));
    }

    #[test]
    fn the_keyboard_qemu_gives_a_guest_counts() {
        assert!(typing(&listing(&[
            POWER_BUTTON,
            PS2_KEYBOARD,
            QEMU_POINTERS
        ])));
        assert!(!typing(&listing(&[POWER_BUTTON, QEMU_POINTERS])));
    }

    #[test]
    fn a_valve_controllers_lizard_keyboard_is_no_keyboard() {
        let puck = listing(&[POWER_BUTTON, PUCK_KEYBOARD, PUCK_KEYBOARD, PAD]);
        assert!(!typing(&puck));
    }

    #[test]
    fn a_steam_deck_alone_has_no_keyboard_and_one_plugged_into_it_counts() {
        let deck = listing(&[
            POWER_BUTTON,
            PS2_KEYBOARD,
            VIDEO_BUS,
            STEAM_DECK_CONTROLS,
            HEADSET,
        ]);
        assert!(!typing(&deck), "the Deck's volume buttons counted");
        let docked = listing(&[&deck, USB_KEYBOARD]);
        assert!(typing(&docked));
    }

    #[test]
    fn every_known_handheld_alone_has_no_keyboard_and_one_plugged_into_it_counts() {
        for (vendor, product) in HANDHELDS {
            let controls = handhelds_keyboard(vendor, product);
            let alone = listing(&[POWER_BUTTON, PS2_KEYBOARD, &controls, PAD]);
            assert!(
                !typing(&alone),
                "{vendor:04x}:{product:04x}'s controls or buttons counted"
            );
            assert!(typing(&listing(&[&alone, USB_KEYBOARD])));
        }
        // The same firmware keyboard under ids no handheld has is a
        // keyboard like any other: nothing is left out by its shape alone.
        let unknown = handhelds_keyboard(0xbeef, 0x0001);
        assert!(typing(&listing(&[POWER_BUTTON, &unknown, PAD])));
    }

    #[test]
    fn a_device_with_letters_and_a_pointer_counts() {
        // A mouse whose buttons send keys is not told apart from a keyboard
        // with a touchpad of its own: the one costs a field nobody types in,
        // and leaving out the other would take it from somebody who needs it.
        assert!(typing(&listing(&[POWER_BUTTON, KEYS_AND_POINTER, PAD])));
    }

    #[test]
    fn an_empty_or_garbled_listing_has_no_keyboard() {
        // A file read whole that says nothing; one that cannot be read at
        // all is the next test's.
        assert!(!typing(""));
        assert!(!typing("I: Bus=0003\nB: EV=120013\nB: KEY=nonsense\n"));
    }

    #[test]
    fn a_list_that_cannot_be_read_offers_the_field_as_it_always_was() {
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(answer(Err(denied)), "the field was taken away unasked");
        assert!(!answer(Ok(listing(&[POWER_BUTTON, PAD]))));
        assert!(answer(Ok(listing(&[POWER_BUTTON, PS2_KEYBOARD]))));
    }

    #[test]
    fn a_bitmap_is_read_from_its_lowest_word_at_the_end() {
        // KEY_A (30) in the last word, KEY_POWER (116) in the one before.
        assert!(bit("10000000000000 40000000", 30));
        assert!(bit("10000000000000 40000000", 116));
        assert!(!bit("10000000000000 40000000", 31));
        assert!(!bit("40000000", 116), "a word that is not there was read");
        assert!(!bit("", 0));
        // A keyboard short of one letter (Z, 44) is not one.
        let mut keys = u64::MAX;
        keys &= !(1 << 44);
        let short = format!("B: EV=120013\nB: KEY={keys:x}\n");
        assert!(!typing(&short));
        let whole = format!("B: EV=120013\nB: KEY={:x}\n", u64::MAX);
        assert!(typing(&whole));
    }
}
