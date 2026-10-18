# How the Linexin Installer is designed

[Project home](../README.md) · [Languages](localization.md)

This is the design in prose, for somebody reading the code or deciding whether
to trust it with a disk.

The installer ports the earlier GTK/Python Linexin installer feature for
feature, in Rust on lxb-toolkit, and fixes that installer's known defects: a
second EFI partition made on a disk that already had one, no legacy BIOS, no
Intel graphics in the unused-driver removal, and a confirmation that did not
say exactly what would be erased.

## One program, two processes

The binary is started twice, as two different people.

The **interface** (`src/ui`) runs as the person in the live session. It reads
the machine and nothing else: `lsblk` for the disks, `/sys` for the firmware,
the zoneinfo and XKB databases for time zones and keyboards, `pacman -Qei` for
the packages on the medium. From the answers it builds one `Request` and shows
the plan, then the progress. It never writes to a disk and never holds a
privilege.

The **worker** (`src/worker`) is the same program started as
`pkexec /usr/bin/lxb-installer --worker`. polkit finds the action
`io.github.petexy.lxb-installer.install` by that exact path and asks for the
password (on the live medium a rule lets the live user through without one).
The worker reads the request from its standard input, checks every part of it
again against the machine as it is now, writes, and reports what it is doing
as JSON lines on its standard output and into a progress file under
`/run/lxb-installer`. If the interface is closed and opened again while a
worker runs, it finds that worker and follows it rather than starting another.

`--worker` is accepted only alone: nothing else on the command line can change
what the root half does, and the interface refuses `--worker` among other
flags rather than guess.

**The measurement** (`src/worker/measure.rs`) is the program's second root
road, `pkexec /usr/bin/lxb-installer --measure`, under an action of its own,
`io.github.petexy.lxb-installer.measure`, whose question says nothing is
changed. Both actions name their flag as `exec.argv1` as well as the path, so
pkexec can never raise one mode under the other's question. It reads for
Partitions what a person's own process cannot: how full each ext4, NTFS and
EFI system partition is and how small it could be made. It reads one
request (the disks as they were listed), finds each disk again and reads
nothing of one that changed, which its answer names, nor of one nothing may
be made on (the installation medium, a read-only disk), which it names only
once it is not as listed any more; asks of every partition whether it is mounted,
swap or held, and reads nothing of one that is; then reads in a mount
namespace of its own, with only a few command lines, each checked word for
word as it starts: `dumpe2fs -h` and `resize2fs -P` for ext4, `ntfsresize
--info` for NTFS (whose refusal says a check is due), the EFI partition
mounted read-only for a moment, and `du` of the image. ntfsresize cannot say
whether Windows is hibernated, was shut down by Fast Startup or left its
journal unclean — it opens every volume "forensic", and libntfs-3g then
skips both checks — so an NTFS it reads cleanly is then asked for one
read-only sign of hibernation, and only that: `ntfscat <partition> hiberfil.sys`,
which opens the volume read-only, its output cut at 4096 bytes and the
program then ended, judged by libntfs-3g's own rule (a first word other
than `hibr`/`HIBR` is not hibernated; a file shorter than its header, or one
beginning so, is; and no such file is what libntfs-3g calls not hibernated,
leaving the rest to its journal check — ntfscat's words for that counted
only when they are all it said, since it says them for any failure of its
lookup). Signed not hibernated by a `hiberfil.sys` of its own it is read;
hibernated, or a sign it cannot read, it is never offered to be shrunk;
with no `hiberfil.sys` at all, what it uses is read but it is never shrunk.
A Windows keeps that file on its own system volume only, so its data
volumes (a "D:") hold none, and whether a Windows hibernated or shut down
by Fast Startup still keeps their metadata only their journal says; a
Windows with hibernation switched off holds none either. And one read is
offered to be shrunk only while, across every disk the measurement was
asked about, no NTFS is signed hibernated, none could not be read whole
and no volume is BitLocker (`Measures::shrink_of`, the rule set's one
reading of a measurement for a shrink): a hibernated Windows may hold
another Windows' partition as its data volume. So the interface asks about
every disk listed, and the measure entry answers the Windows volumes of a
disk it does not read unread. Its journal is not looked at: no read-only sign of an unclean
journal was shown to be libntfs-3g's own rule. Never e2fsck, mkfs, sfdisk, wipefs, a resize, or
a mount of ext4, Btrfs or NTFS, which can replay a journal even read-only;
BitLocker and LUKS are not touched at all. It answers with one JSON line, logs only to standard error,
takes the worker's lock (so it never runs beside an installation) and never
opens the worker's progress file or log. Like `--worker` it is accepted only
alone and refuses off the live medium before anything else, and nothing it
answers is ever trusted with a write: the worker measures for itself —
before a shrink, with these very commands and rules, on an allowlist of its
own that adds only the dry runs (`manual::shrinking`), every Windows volume
of every disk it finds then read again so the rule across every disk is
applied once more, just before anything is written.

**Only on the live medium.** Both halves ask one hard guard
(`model::live::guard`) before anything else: whether `/run/archiso/airootfs`,
the live system's read-only image, is there — which only root can make. The
interface asks it in `launch::start`, the one place a worker is started, and
in `launch::measure`, the one place a measurement is, before any thread or
program, and answers a refusal at once; the worker asks
it first of all, before it even looks at whether it is root, makes its
directory or takes its lock. So on a development machine or an installed
system no installation starts and `--worker` does nothing, whoever runs it;
nor does `--measure`, which asks the same guard first.
The tests hand the guard their own answer and never read the machine, and in
a test build polkit's program is a path that cannot exist. `--demo` and
`--shot` are the made-up computer, whose Install only rehearses and never
reaches `launch::start`, and whose Partitions never asks for a measurement;
tests walk it from every scene with every press to show both.

## The request, and the list of what will be erased

A request is the installation the person chose plus the **consent list**: the
devices the last question showed as erased or formatted. The worker computes
the same list from the chosen installation and refuses unless the two are
equal, so nothing is erased that the person was not shown. For a layout made
by hand on Partitions (`Target::Layout`), the list is every line of what it
destroys, named by path, number, start and sizes (`layout::destroys`) — and
the worker of this build refuses any layout outright. The worker reads at most
1 MiB of request (`MAX_REQUEST`): the largest the interface allows, four disks
of 128 entries at their longest, is some 880 KiB, and an easy request two or
three.

Before it writes, the worker finds the disk again and refuses if anything that
identifies it has changed — its sequence number, model, serial, size, device
numbers, sector size, or its partitions — telling a different disk apart from
the same disk repartitioned. It refuses if anything on it is mounted or in use,
opens every device it will write exclusively just before writing, and gives
itself a mount namespace of its own so nothing it mounts reaches the live
session. A separate /home on another disk is carried in the request as
reviewed and checked the same way.

## What the worker says back

First a `Plan` of the steps, then `Begin`, `Progress`, `Log` and `Warning`
events, and exactly one `Finished` or `Failed` at the end. No event carries
words for the face of the panel: steps, warnings and failures are identifiers,
and the interface words them from its catalogs in the person's language.
`Log` lines and a failure's technical detail are kept for Details and for the
log file, `/var/log/lxb-installer.log` on the installed system.

## Where Linexin goes on the disk

Three ways, as before: **erase a whole disk**, use **a gap of free space**, or
**replace a partition**.

On a **UEFI** computer, erasing a disk makes a GPT table with a 2 GiB EFI
partition (FAT32, mounted at `/boot`) and the rest for Linexin. Free space and
a replaced partition **reuse the disk's EFI partition** — never formatted, and
nothing of another system's on it touched. With at least 600 MiB free on it,
it is mounted at `/boot` and holds the kernels too. With less — Windows makes
100 MiB ones — it is mounted at `/efi` and holds only the boot loader, and the
kernels go on a new 2 GiB FAT32 **Linux boot partition** (the XBOOTLDR type,
never a second EFI partition) at `/boot`, carved from the start of the space
used. The firmware reads the kernels through its own FAT support, so no
filesystem driver stands between the menu and Linexin. A disk with no EFI
partition gets one, at the start of the space used. A replaced partition keeps
its place unless one of those partitions has to come out of it.

On a **BIOS** computer, erasing a disk makes an msdos table with one partition
and GRUB written to the disk — or GPT with a 1 MiB BIOS boot partition when
the disk is larger than an msdos table can address. A GPT disk gets a BIOS boot
partition in the space used if it has none, and an msdos disk with no free
primary slot is refused before anything is asked.

**Btrfs** puts `/` and `/home` in subvolumes `@` and `@home` (and the swap file
in `@swap`), mounted with zstd compression; **ext4** is the default. The
**swap file** is 4 GB unless changed, or none. A **separate /home** is an
existing partition of at least 1 GB, on this disk or another, kept as it is or
formatted.

Sizes are read in bytes and the disk's own sector size, never assuming 512.
Partition device names are read back from the kernel after partitioning, never
built from strings.

## The boot loader

Chosen by the computer and the disk unless the person overrides it in Options:

- **GRUB** on a BIOS computer, or on an msdos disk;
- **systemd-boot**, with no menu, when Linexin is alone on the computer;
- **rEFInd** when another system is found — on the EFI partition, or by
  os-prober.

Another system's files on a shared EFI partition are never removed, the
fallback loader is written only to an EFI partition the installer made, and the
firmware entry is made on the right disk, NVMe included; on a BIOS, GRUB goes
on the disk the kernel says holds `/`. A loader the medium does not carry can
only be downloaded, which is why the package depends on both GRUB and rEFInd:
a medium that carries the installer carries every loader it may choose.

Some choices cannot be installed at all without the internet — Kinexin, and
LineXinBar on a medium that does not carry it — and those stop the
installation before the disk is touched when it does not answer. Updates,
Flatpaks and the snapshot tools only use it when it is there, and turn its
absence into a warning on Done.

## What the system steps do

The live system's own image is copied to the new root with rsync, its progress
shown as it goes. Then, in order: the kernel put in place, `fstab` written from
what was mounted, a keyring of the new system's own, the live session's residue
removed (its user, autologins, password-less rules, live services, archiso's
own networking, so NetworkManager alone runs the network), the language,
keyboard and clock, the accounts (the person's, with the groups the earlier
installer gave, and root locked unless a password was set for it), the desktop,
updates when chosen, the packages left out (one transaction each, so one that
cannot be removed does not stop the others), Flatpaks, unused microcode and
graphics drivers, Btrfs snapshots before each update when chosen, the initramfs
and the boot loader. The log is copied in and everything is unmounted.

The three desktops: **Linexin** is GNOME as the medium carries it, no internet
needed. **Kinexin** is Plasma, downloaded. **LineXinBar** is lxb and its login
screen CEDM, from the medium when it carries them and downloaded otherwise; it
keeps GNOME installed and makes CEDM the login screen with the LineXinBar
session as its default.

## The interface

The page draws its whole frame every time: the toolkit's wallpaper fills the
window, and the installer is one glass panel centred on it, **scaled by the
window's height** — about 1280 × 860 at 1080p, never more than the window less
a margin. Under LineXinBar the window is full screen; on the GNOME live desktop
it is an ordinary window, and the panel is the same panel. A narrow or portrait
window re-flows: columns slide with the light instead of shrinking. Where to
install's disks then leave the places' column peeking in at the panel's edge
by the toolkit's `layout::PEEK` of it, as LineXinBar's Home menu leaves its
cards, so Right is seen to lead somewhere; a disk's name may take a second
line there.

Every screen asks one thing and has one primary button. Lists are columns that
scroll with a spring and dissolve at their ends; a scroll bar appears only when
no controller is in hand. You's form is one column too, a field with nothing
to say under it close under the one before, so on a landscape panel all of it
shows until the administrator's fields are asked for; where it goes on past an
end, it rests with the next field peeking in at its soft edge, and a controller
is shown an arrow just past that end, as the toolkit's and LineXinBar's menus
show one. A field tells its three kinds of words apart: its heading the
smallest (the shell's smallest words, a step under a caption) and white, the
hint in an empty one a caption in the palette's soft lavender, barely there,
and what is typed the body's size and white — only what a person typed reads
as written. Time zone and Keyboard walk their two columns as LineXinBar's file
picker walks a disk: the bar's rows rolling under a fixed line, and the second
column opening beside the first only once a row of it is pressed, the whole
trail sliding a step so the light stays where it is, the first stepping back
to that row, Back closing it again; the disc under the lit row rides it
across as the trail steps. There the pointer lights a row of the column at the
front where it stands — the column holds still, as every list does, rather
than rolling the row to the line under a resting pointer — a click on a row
presses it at once (a part of the world or a layout opens, a place or a kind is
chosen), and a click on the row the second column was opened from closes it.
LineXinBar's own file picker lights nothing on a hover and carries the light on
a first click; the installer keeps the same pointer rule on every screen, by
design. On both steps Continue keeps the guess only once its second column has
been seen: standing open on the place or the kind in force, marked as chosen,
for as long as the column takes to come in (0.45 s; 0.3 s in low-end mode,
where it is there at once), counted by the frames that draw it — from the
second of them, by which the first had reached the display, and only once
they have carried the column all the way in, so a frame or two drawn across a
stall never count. A frame that draws the column with the light on the guess
begins it — the column opened on it, or the light come onto it in the column
standing open. Until then the first press of Continue opens that column on
it instead — the light staying on Continue — and a press while it is coming
in does nothing, a second press in the same frame among them, since a frame
answers all its presses at one instant before it is drawn (a click, asked
after the drawing, finds the column as it stood before it); the first press
after a frame has counted the column goes on, so nobody goes on with a city
or a keyboard kind they never saw. Taking the guess in that column waits as
long after it opened on it, so Enter twice on the guessed part of the world
or layout never takes the guess undrawn; a row the light was walked to once
a frame had drawn the column, another column, and what a search found do
not wait. Choosing a language that guesses another one puts it out
of sight again. The legend in
the corner draws the button that does each thing — South on a pad, Enter on a
keyboard — and is itself a row of buttons a pointer can press. As in
LineXinBar, hovering is selecting: the light goes to whatever the pointer
moves onto, a click acts on what it lands on at once, a list never scrolls
under a resting pointer, and a pointer left still never takes the light back
from the keys or a pad. A click
that brings another page in under the pointer — the next screen, an Options
page, the second column — lets the second click of a double-click go, so it
never acts on what came in: double-clicking Continue answers one question (on
Time zone and Keyboard, with the second column unseen, the first click shows it
and a second as it comes in does nothing), and double-clicking a part of the
world opens it without choosing a place. A key pressed twice is pressed twice
— Enter held down presses once — but for Time zone's and Keyboard's Continue
while the column it showed comes in. A field typed in is scrolled into view,
wherever the pointer held the form when it lit it. The one thing
hovering does not move is the light in a field being typed in — one typed
into, one a key brought the light into, or one LineXinBar's board is typing
into — because the light is where the keys go: a mouse knocked between two
keystrokes must not send the rest of a password into the field it rests on,
nor a pointer crossing the page put the shell's board away mid-word. A key, a
pad or a click moves it on, and out of the fields hovering lights again. Move, press, back and error each
have a sound.

The screens, in order: **Welcome** (Welcome to Linexin taking turns in the
earlier installer's twenty languages, the button that begins with it, and
nothing else: Language is the very next step) → **Language** (sixty-five, in
their own names, each beside its country's flag, in a column as wide as the
longest needs, in the middle; one of the installer's ten switches its words at
once) → **Time zone** (a region, then a city, in the language chosen, guessed
from the language) → **Keyboard** (a layout, then its variant, with a field to
try it where a keyboard is attached) → **Where to install** (the disks, then their places:
Erase, a gap, a partition; Options for the filesystem, swap and /home; Advanced
for **Partitions**, every disk laid out as GParted lays one out, in GParted's
technical words, where a plan of operations is made: below) →
**Desktop** (three cards, Install updates; Options for packages, Flatpaks and
the boot loader) → **You** (one column: full name, username, the password
twice with its strength, the computer's name, a root account) → **Review**
(one headline, a few rows, Install) → the **question**, safe answer first,
naming what will be erased → **Installing** (a bar, the step, "Step 4 of 12 ·
40%", the log behind Details) → **Done** (Restart now first, Not now) or
**Stopped** (what to do, Details, Close).

Language, Time zone and Keyboard each have a search field, as LineXinBar's own
long lists keep one at their head. Options summons it — North on a pad, the
Menu key or F10, a right click; the legend says "Search" — with the light in
it, and with a pad a board at once. Every letter narrows the list: a language
by its own name or its English one, a place by its city or its country in the
language chosen or in the time zone database's English (Warszawa or Warsaw,
Polska or Poland) or by its part of the world, a keyboard by its layout and
kind, whatever the case and the Latin accents. Time zone and Keyboard then show what
was found in one column, from every part of the world or every layout at once,
keeping their two columns as they were for when the search is emptied. Enter,
Down or the board's Done lands the light on the first match; Accept chooses it.
Back empties the field, the light staying on what it was on, then closes it.
The letter Y summons it too wherever no field takes it as text, as the shell
reads it beside the Menu key and F10 (the toolkit's window does).

**Partitions** is Where to install's Advanced page, a page of the same
step: Advanced, at the other end of the
button band from Options and Continue (at the foot of the disks' column on a
display standing on its side), opens it on the disk the light was on, and
Back returns to Where to install with the light on Advanced. It is the one
page that talks like GParted, by design: device names (`/dev/sda3`),
filesystem types, flags, mount points and binary units (GiB, with the
language's decimal mark), never translated; its own words are the catalogs'.
A chip for each disk along the top (the one shown lit, a blocked one outlined
and passed over, how many operations the plan has on each in a bead), the
**map** of the disk shown under them — a tile for each partition and each run
of unallocated space, as wide as its share of the disk but never narrower
than a pointer can land on, a band along its top in its filesystem's GParted
colour (unallocated space dashed), an msdos extended partition a frame round
its logical ones, the words thinning as a tile narrows — and under the map
the lit tile's **details** in two columns (Partition, File system, Size, Used
where it has been read, Label, Flags, Mount point, Pending), and on a landscape
panel GParted's pending pane. Left and Right walk the map, Up and Down go
between the chips, the map and the buttons, LB (Shift+Tab) shows the disk
before and RB (Tab) the disk after, the pointer lights what it rests on.

**It plans.** A tile's menu (A, Enter, a click, growing out of the tile)
gives a partition a mount point (`/`, `/home`, `/efi`, `/boot`, swap, a path
of one's own, none), formats it, resizes it, labels it, deletes it, or
undoes what was asked of it; unallocated space offers New…, or on a disk
with no partition table the table to make; Options a new table over a disk
that has one (a disk's chip raises no menu) — but on a disk with something
written over the whole of it as well (a stick a hybrid ISO image was
written to, an old signature never wiped) neither a table nor a partition,
which the rules refuse, and the details say so under each of its tiles —
nor is a partition formatted there grown past the partitions deleted after
it, since discovery lists no free space on such a disk; and
on a disk whose logical sector is larger than 4096 bytes, which the rules
lay nothing out on, nothing at all — the details say "Nothing can be
changed on" it under every tile. The larger of those are **sheets** laid over the page
under the map — New partition (its size, its place in the space, its
filesystem, its mount point, its label), Resize, Label…, Mount point ·
Other… — whose size is held as LineXinBar's Settings bars are (A takes hold,
Left and Right step, LB and RB step ten, Y types a number, B puts it back;
the pointer drags the tile's edge on the map or the track), the map drawing
the plan with the sheet's values in it as they are chosen. Options undoes
the last operation, clears them all, lists them in GParted's words and sets
the swap file. Everything asked for is a list of operations folded as
GParted folds them (`ui::partitions::Plan`), drawn on the map as the disks
will be, and the foot says the first thing Continue still needs — a
partition at `/` large enough and formatted, on UEFI an EFI partition at
`/efi` and a `/boot` beside one too small for the kernels, on BIOS with gpt a
`bios_grub`, each mount point once, a filesystem that suits it — by the
worker's own numbers.

**What the disks hold** — how much of a partition is used, the least it
could be shrunk to, whether Windows is hibernated or a check is due, how
much room a kept EFI partition has, how much of the system goes in `/var`,
`/opt`, `/srv`, `/root` and `/tmp` — takes root to read. On a live system
Partitions asks the read-only measurement for it (`launch::measure`, the
live guard first, then `pkexec … --measure` under its own read-only
action) the first time it opens, about the disks it can show, and again
only once the disks are listed otherwise than they were; the answer is kept
by partition, named by its path, its UUID and its size, so it is never
taken for another's, and the frame takes it in whichever step is up. While
it reads, the details say "Reading…" as Used, and Partitions' plan is not
judged by it for longer than a minute (`partitions::READING_PATIENCE`):
then what it has not answered is unreadable, as though it had failed, the
plan is judged by that, and the measurement is let go of, never killed,
its answer counting for nothing should it come. But no installation is
started while its measure entry still runs, whatever the choice in use —
a place on Where to install as much as a plan, and also once it is no
longer waited for: it holds the installer's lock, and a worker started
beside it would be refused the lock and end on Stopped saying another
installation is running. Install says "This computer's disks are still
being read. Try again in a moment." instead, the person still on Review,
for as long as the measure entry runs (`Measuring::running`; a measurement
let go of is kept until its thread, which ends only once the measure entry
has, is done). Read, Used is what the partition holds and Resize shrinks
a kept ext4 or NTFS no further than what it uses and a margin, or its own
tool's least where that is more. Read as what cannot shrink — Windows
hibernated, a check due, in use, encrypted, unreadable, an NTFS with no
hibernation file of its own, or another Windows of the measurement
hibernated, unread or locked by BitLocker — Resize is not offered and the
details say why under themselves, and what to do where something can be
done ("Windows on /dev/sdc1: shut it down with Fast Startup off"; under an
NTFS with no hibernation file of its own, which may be a data partition or
a Windows whose hibernation is switched off, "To shrink it, use Disk
Management in Windows");
and where reading ended without a partition it would have read (refused,
failed, not waited for any longer, or not in the answer), it is
unreadable, and the foot's "isn't
known yet" for a kept EFI partition at `/boot` or a `/var` of its own
becomes "couldn't be read". An answer naming a disk that is not as listed
has the disks listed again (read-only). The demo never measures: its
numbers are its own (`demo::measures`), on two readings of its made-up
machine (`demo::Reading`): every Windows shut down — Windows' own
partitions read as the measurement reads one whose `hiberfil.sys` says it
is not hibernated, the Windows disk's Data partition a data volume — and,
for two pictures only, the old disk's Windows hibernated, holding every
Windows back. Off the live medium nothing is read, and nothing offered to be
shrunk.

**The choice in use is the one confirmed last.** Continue on Partitions,
once nothing is missing, makes the plan the choice in use — in the demo; and
goes on to Desktop. Choosing a place on Where to install afterwards makes
that place the choice and keeps the plan as a draft, shown again on
Advanced; Back changes nothing about it, and Back from Desktop returns to
Partitions while the plan is in use. Meanwhile Where to install ticks no disk
and no place, its bar marks none, its foot says "Using your partitions · 6
operations", and its Continue goes on with the plan as Partitions' does.
Review says the plan's rows in place of Where — one for each mount point, in
GParted's words ("New Partition #1 · ext4 · 160.0 GiB on /dev/sda",
"/dev/sda1 · fat32 · 100 MiB · kept"), the swap file, and how many operations
will be applied, each going back to Partitions — and the last question is
the plan's own, in pages: every line of what the plan destroys, in GParted's
words, a bullet to each — every disk it writes a new table over, with each
partition that erases under it, then every partition it shrinks, formats or
deletes, an EFI system partition or one of Windows' named so — as many as
the dialog holds, the head on every page and "Page 1 of 2" under it, "Go
back" (lit first) and Next on every page but the last, "Go back" and "Erase
and install" with "This cannot be undone." on the last, and never a count of
what was left out: a new table longer than a page goes on over the next,
its sentence said again at the top. Where it destroys nothing, one page asks
to install and says nothing is erased. Then the demo's rehearsal runs as for
any choice: it needs no request — but only once the last page was reached
(`State::begin`).

Desktop and Review judge by what the person has chosen as a request carries
it (`State::config`): with the plan in use, the start-up managers its EFI
system partition's table allows (systemd-boot is outlined where that is
msdos: `desktop-loaders-plan`), restore points by a Btrfs `/` of the plan's
— switched off beside a `/var` of its own, Restore points saying "Not kept
beside a separate /var" — and the swap file, none beside a swap partition.

**Only what QEMU proved reaches a disk.** With no plan in use, what the
person has chosen is the draft byte for byte. With one, `State::config` puts
the plan in the request as a layout (`Target::Layout`, the end state of each
disk, `model::layout`), its disk, filesystem and swap file pinned to it and
nothing of Where to install's place left in it, and the consent list is
every line of what it destroys. **Off the demo** — a live system — Continue
on Partitions goes on only with a plan whose every need lies within one set
of `layout::PROVEN`: the exact needs of one scenario of the QEMU
campaign that passed, its firmware and the kind of every table it writes or
keeps among them (the campaign wrote all ten, every one of its eleven
scenarios that carry a plan out having passed); anything else is refused in
the foot, with the first thing the plan asks that no set holds or
"Partitions can't be applied in this version yet." — a disk of the plan
whose logical sector is not 512 bytes among it (`LargeSectors`, which no
set holds: QEMU gave every scenario's disks 512-byte sectors, though the
rule set lays a plan out at 4096 too). Install judges it again
(`State::ready`); `State::begin` goes on only once the plan's question was
read to its last page and the lines its first page showed are still the
request's consent; and **the worker judges the layout again** before it
looks at anything else (`check::validate`): by the same rule set, the same
`PROVEN` and its consent (`check::judged`). (Until the campaign two outer
walls refused every plan at Install and every layout in the worker, whatever
was proven; the commit that wrote what passed into `PROVEN` took them away.)
Then its executor (`worker::manual`) carries it out: the udisks rule put in for the run and taken
out however it ends (`quiet::Quiet`), the disks found again and nothing on
them in use, phase A reading the disks, their tables and the numbers it
needs without writing, every destructive command given leave by the fence
and its claim, a kept ext4 or NTFS shrunk before anything else on any disk
(read again, the rule across every disk applied again, dry-run, then shrunk
by its own commands, which the fence judges by shape, place and size —
never to its filesystem's own size or more, which would grow it — and read
back inside its new entry before its entry is made smaller; Windows left
marked for its own check, which Done says), the disks read back, the filesystems, mounts, swap file and
fstab, on as many disks as the plan touches, each in path order — a disk it
only mounts from, such as a `/home` kept from before, given no command at
all and its filesystem held to the UUID it was reviewed with, the
firmware entry made on the EFI system partition's own disk wherever `/` is,
and on a BIOS GRUB on the disk that holds `/` — in its `bios_grub`
partition on a gpt, in its first MiB on msdos, whose logical partitions
are deleted only from the last and found by where they start, and whose
boot flag is set only where nothing is marked. It asks for nothing outside
`PROVEN` either, and is tested on a machine that runs nothing, a test
judging by sets pretended in place of `PROVEN` where it needs other ones
(`layout::PretendProven`, test builds only). The interface reads the disks
as it starts and, on a live system, once more just before the plan's
question under a plan and after a measurement names a disk not as listed
(read-only, as at the start); a plan is kept only while every disk it
touches is listed exactly as it was, and otherwise dropped whole, Install
and Where to install's foot saying so, and what was measured of disks
listed otherwise goes with them, read again (`partitions::disks_listed`).

On a display standing on its side, where the tiles would not keep their
words side by side, the map is laid out wider than the panel — each tile at
least as wide as its words need, a share of the room more by size — and
slides with the light as the Home menu's cards do: the lit tile always in
view with a peek of its neighbours, moved as little as keeps it so, on the
cards' spring; a tile the pointer lit is not slid under it, and a click on a
tile partly out of view brings it in. A sheet there says, between the map
and its card, as many of its tile's details as fit, those it is about first
(Resize: the used space). Landscape is laid out exactly as before. **Motion:**
a sheet grows out of its tile as the toolkit's menus grow out of what raised
them, its glass first and its words after, the light put down on its first
row as it unfolds rather than flown across the page from the menu; closed, it
folds back over the page into the tile, its words going first. When the plan
changes, each tile eases on the cards' spring from where the tile it was
stood — a partition shrunk narrowing from its right, the space after it
widening with that edge, a run made by a delete widening over where the
partition was — while a size moving on a sheet is drawn at once. In low-end
mode all of it is at once.

**Welcome takes turns**, as the earlier installer's welcome did: Welcome to
Linexin in its twenty languages, in its order, each with the name where the
language puts it (Linexinへようこそ, Linexin में आपका स्वागत है, Linexin’e hoş
geldiniz), the button saying the earlier Begin Installation in the same
language. It starts on the person's own language where it is one of the
twenty; the first change comes three seconds in, then each greeting stays five
seconds, goes in 0.4 s and the next comes up in 0.6 s, the button's word 0.15 s
behind it. The button stays where it stands, lit and pressable, as wide as the
widest of its twenty words; the greeting's room is as tall as the tallest
needs (one line on a landscape panel, up to three on one standing on its side,
broken evenly), so nothing moves but the words. On low-end hardware the words
change at once, one frame for each. The head names the program. Latin, Greek
and Cyrillic are drawn in the toolkit's Roboto, Devanagari and Han in its Noto
faces, and kana, Hangul and Arabic — right to left, shaped — in the machine's
fonts, which the live medium carries (noto-fonts, noto-fonts-cjk).

**Time zone speaks the language chosen** on the Language step, whichever of
its sixty-five, even where the installer's own words stay English: a place and
its country as CLDR names them there (Warszawa, Нью-Йорк, 纽约; Polska), each
part of the world's places sorted as that language sorts them (Chinese by
pinyin, Polish with Ł between L and M), and Review's row and the lit name say
the same. The names come from `data/places/names.txt`, compiled in: CLDR 48's
exemplar cities and territory names, and each language's order, for every
zone of tzdata's zone.tab, as ICU 78 gives them, made by
`scripts/cldr-places.py` (run by hand when ICU or tzdata moves on; nothing is
asked of ICU when the installer is built or run). A place the table does not
know keeps the database's own city. The names are under the Unicode licence,
`data/places/LICENSE`, which the package installs beside the program's.

Each language wears its country's flag at the start of its row, as the
earlier installer wrote a flag emoji before each name:
as tall as the row's two lines — its own name and the English one under it —
together, the way the search field's magnifier spans its heading and what is
typed, the names moved right of it. The flags are Noto Color Emoji's waved
flags, the ones GNOME drew for that emoji, cut out of the font by
`scripts/flags-from-noto.py` into `data/flags/` (under the font's SIL Open
Font License 1.1) and carried in the program, one for each country a locale
names; a locale naming none would wear the font's white flag, as the earlier
installer gave one. They are drawn as pictures (`Ui::picture`), not as letters
of the emoji font: the toolkit's renderer draws a colour emoji at full
strength whatever its words are faded to, so a flag stood whole on a page
still coming in, where a picture comes and goes with the words. They keep
their own colours on a row at rest. The column is measured with the flag in
it, so on a landscape panel every name stays on one line; on a display
standing on its side a long name or English name takes a second line.

Every row and field of the steps that ask something wears a mark, as Review's
rows do, in the glossy material every mark of the shell wears: a disk the kind
it is — an NVMe drive's M.2 stick, a SATA SSD's case, a hard disk with its lid
off, a USB stick, a memory card — by its bus, else by whether lsblk says it
spins (a virtual disk wears the SSD's case whatever it says: Linux's
virtio_blk says every disk spins); a place on it an eraser (the whole disk), room with a plus in it (free
space) or two arrows passing (a part replaced); You's fields the shell's name
card, at sign and key, a key with a tick cut through its bow for the same
password again, a display with a name on it for the computer's name, and the
shell's shield on the administrator switch; the search field of Language,
Time zone and Keyboard the shell's magnifier, as the shell's own searches do.
Where to install's own mark is a drive with an arrow coming down onto it. What
the shell has no mark for the installer draws itself (`data/glyphs/`), white
shapes saying `lxb:shape` that the toolkit measures and shades as its own
(`App::add_mark`). You's fields and switch wear their marks at any width, on
a display standing on its side too, as Review's rows do there, and the search
field, alone over its list, its magnifier: every heading and hint of every
language is held short enough to fit beside a mark in the narrowest form. The tick on what is chosen and the switch are the toolkit's
(`Ui::chosen`, `Ui::switch`), of the same material: the tick ringed in the
panel's dark glass so it reads on the light, the switch a dark capsule, the
accent's track while it is on, and a knob.

What a person reads is plain: "Samsung SSD · 1 TB", "Windows and 2 other parts",
never a device path, a filesystem name or a command's output on the face of a
panel. With a keyboard in hand a text field takes typing directly. With a
controller in hand South on a field brings up an on-screen keyboard: **under
LineXinBar the shell's own** — the field takes text (Wayland's text input),
which the shell answers by raising its board over it — and anywhere else the
installer's, because the GNOME live session has none for a pad. Under
LineXinBar is decided once, as the window opens: `XDG_CURRENT_DESKTOP` names
LineXinBar, as the session and its compositor set it for everything they
start, and the window is a Wayland one. A field the light only passes takes
nothing there, or the shell's board would rise over every field of You as the
D-pad walked it; the board's Start sends Enter, which is answered as Enter
from a keyboard (on to the next field, the light down to what a search found);
South again on a field whose board was put away asks for it again (the field
lets go of its text for a frame, since the shell offers its board once a
field), and so do the legend's Type pressed by a click or a tap, and a click
or a tap on the field it was put away from. The shell raises its board over
any field taking text while it has a controller in hand, however the field
was chosen — a pad's South, or a click or a tap, which the toolkit counts as
the pointer in hand and so types into the keyboard's way — draws it along the
foot of the display wherever the field is, and says nothing back about where
or when it goes. So the screen's room ends over the board for as long as the
toolkit knows it stands over the window (`Page::shells_board`: up as a field
takes text where the shell last wrote down a controller in hand, gone with
the field, with a pad's press, and the moment the board lets go of the
keyboard — its own hide key, B, Start, a key of the person's own), where the
shell settles it (`lxb_toolkit::board::rect`, the shell's own rule, for a
window filling the display), risen on the cards' spring as the board rises
and given back as it goes: Keyboard's test field stands above it, and You's
form scrolls the field typed into above it, wherever the pointer that clicked
it held the form, as they do over the installer's own board (`--shot
--shells-board` draws that). What the shell's board types reaches the window
as a keyboard's keys, so the pad is held in hand meanwhile — until a pad's own
press, or a key once no board stands over the window (the key that arrives as
it goes, Start's Enter or the person's own key the board passes on, is still
the board's). With a pad in hand the field the
light stands in takes a keyboard's typing quietly (`Page::taking_keys`:
nothing said to the compositor, so no board rises), so a keyboard reached for
types into it from its first letter, Space a space and Backspace rubbing out,
and that letter lets the pad go; a click does not, since the shell itself
goes on counting a controller in hand through a click and would raise its
board over a field that took text. A pad's own press is one arriving
with a pad the last thing the toolkit saw reached for, not the pad merely not
put down yet: the toolkit says a pad is in hand from South until the board's
first key, frames in which nothing is pressed at all.

Every change of page moves as LineXinBar's pages do — a step on or back, a
page of the Desktop step's Options, the installation begun and ended — the
way the shell steps into a subcategory of its bar and back: going on, the new
page comes in from a step to the right (a quarter of the panel, the shell's
own step at the panel's size) while the page left slides a step to the left
and steps back, smaller and hazier; going back, the page left goes back out
to the right while the one returned to comes forward out of that depth. Two
pages are never read over one another: as where the shell's search crosses
two sets of columns in one place, the page going has given up all of its ink
before half way, and the words and marks of the page coming in come up only
after it, while its glass comes up over the whole way so the panel is never
empty. A page's pictures (Desktop's screenshots, the bar of a disk) and what
belongs to its words (a field's caret, the ring round a tick, a switch) go with its
words, not with its glass. At the sides of the pane a page coming in or going fades into the
glass across the panel's padding rather than being cut off at a line. A press
dips the control it landed on, on the page it was on. A page carries its head — the
mark, the title, the caption — and everything in its room; what is the same
on both pages does not move: Continue stands where it stands, the light on it
too, a button that keeps its place but changes its word (Continue becoming
Done) keeps its glass while the old word goes and the new one comes, and so
does the legend pair by pair, Back in its corner standing still and a pair
nudged along by a longer or shorter word beside it going along to its new
place. A page steps on the shell's own depth spring, the rate its bar steps
into a subcategory at. Back pressed before the new page has come in turns the
change round where it stands, the page called back keeping its light and its
ink and coming up from what it had, its words straight away where the page it
gives way to had written none yet. The row of
chips in the head moves by itself, the long chip going over to its new place
leading end first and stretching over the chips between. Time zone's and
Keyboard's second column opens on the trail's own spring, the disc under the
lit row going across to the other column's row with the columns rather than
jumping there at half way; menus and the last question unfold as the
toolkit's always have. The on-screen keyboard rises out of the foot of the
buttons' band; the light stays on the field until the board's first letter
has risen into sight and then goes over to it, rising with its key, and
Keyboard's test field, beside Continue on a wide panel, goes up ahead of the
board so it is never typed into under the keys. A
press during a change lands on the new page, which is pressable where it is
drawn, and a change that has arrived is still. On low-end hardware — the
shell's low-end mode, or a window drawn without a graphics chip, as the
toolkit counts it — nothing of the installer's own moves: a page changes at
once, and so do the light, the columns and the board. The toolkit's own
lights stand on what is chosen there too: the window's clock stands still
with its wallpaper, and the last question's light, timed by that clock, once
stayed on "Go back" while Enter answered the answer chosen. A press still
dips what it presses, and menus and the last question still unfold, as the
toolkit's own do there.

## On the live medium

The installer ships inside the Linexin live ISO, which logs in to LineXinBar
and starts it. What the medium has to do for it: start it from the live user's
own configuration or an autostart entry, never from `/etc/skel` (which the
installed system would inherit); log in through CEDM with an initial session
the installer removes from the installed system; and keep the polkit rule that
lets the live user raise the worker without a password. The package's
dependencies are every program the worker runs, so a medium that carries the
package carries what an installation needs offline — see
[`packaging/README.md`](../packaging/README.md).

## Proven on virtual disks only

Nothing about an installation is tried on a real disk. The unit tests are pure
— no disk, no root, no network. Installations are proven in QEMU before a
release: the live ISO read-only, the installer's package — built in a plain
Arch container — on a payload disk, sparse image files as targets (an empty
UEFI disk, an empty BIOS disk, NVMe, and a disk with a small EFI partition,
Windows' partitions and canary files), QEMU inside bubblewrap with a private
`/dev`, a CPU model with no AVX-512 so an over-tuned build fails there first,
the installer walked with the keyboard, the result checked offline, and the
installed disk started alone until its login screen. Every kind of plan
Partitions carries out was installed and booted that way, on virtual disks,
before `layout::PROVEN` let it through.

On a development machine the installer is run only through
[`scripts/demo-sandbox.sh`](../scripts/demo-sandbox.sh), and only as the demo
(or a picture of it): the script refuses anything but `--demo` with a demo's
or a picture's flags, `--worker`, `--measure` and `--list-disks` wherever they
stand, and
runs the program in a bubblewrap sandbox where no disk of the computer can
even be seen. Its `/dev` has no block device, only bwrap's few nodes and the
GPU's render nodes; `/sys/block`, `/sys/class/block`, `/sys/dev/block` and
every disk's own directory are empty, and so are `/proc/partitions` and
`/proc/diskstats`, so `lsblk` finds nothing. There is no system D-Bus, so no
polkit, pkexec, udisks or logind; no `/run/archiso`, `/home`, `/mnt`, `/media`
or `/var`; no capabilities, no new privileges and no network. Everything is
read-only but a scratch HOME, `/tmp` and an `XDG_RUNTIME_DIR` holding the one
Wayland socket it is shown on, the desktop's or a nested LineXinBar's. The
session bus stays out unless asked for (`--session-bus`): only keeping the
screen awake while Installing is up uses it, and the shell's on-screen
keyboard and settings reach the installer through Wayland and the scratch
HOME — with the session's desktop name, let through as a name and nothing
more, which is how the installer knows its pad types with the shell's board. `--probe` runs `ls /dev`, `lsblk`, `busctl --system` and the rest
through the same sandbox, to see for oneself that they find nothing.

## The pictures

`--demo` is a made-up computer — an empty NVMe drive, a SATA disk with Windows
on it and room beside it, and the USB stick the live system started from,
which carries Linexin and LineXinBar as the Linexin 3.0 stick does, so only
Kinexin is downloaded — with invented models and serials, and time zones and keyboard layouts of its
own, so a picture is the same on every machine it is taken on. Install on it
only rehearses: the events a worker would send arrive on a timer, and no worker
is ever started. `--shot` writes one settled frame of that demo with no display
at all; `--scene` picks the screen (`welcome`, `language`, `timezone`,
`timezone-places` with a region's cities open, `keyboard-layouts` as it is
arrived at, `timezone-continue` and `keyboard-continue` as they are arrived
at with the light gone down to Continue, whose first press opens the second
column, `keyboard` with a kind chosen, `keyboard-none` with no keyboard
attached, `language-search`, `timezone-search` and `keyboard-search` with
the search up and something typed in it, `timezone-search-nothing` and
`keyboard-search-nothing` with something typed that finds nothing, `target`, `target-erase`,
`target-free`, `target-nothing`, `target-whole`, `target-kinds` with an SSD
and a memory card added so every kind of disk wears its mark,
`target-options` and `target-storage` with a menu up,
`target-refused`, `partitions` (Where to install's Advanced page, the light
on Windows), `partitions-nvme` (a disk with no partition table),
`partitions-msdos` (an extended partition's frame round its logical ones),
`partitions-tiny` (the light on a 1 MiB `bios_grub`),
`partitions-blocked` (the pointer resting on the installer's own stick),
`partitions-menu` (a partition's menu), `partitions-mount` (its Mount point),
`partitions-table` (an empty disk's unallocated space offering a table), `partitions-planned` (a plan in force:
a partition deleted, one formatted at `/`, the EFI partition at `/efi`),
`partitions-options` (that plan with Options up), `partitions-pending` (its
whole list), `partitions-missing` (Continue pressed while the partition at
`/` is still kept), `partitions-new` (New partition over the unallocated
space, 120 GiB of it), `partitions-new-held` (its size held and stepped, the
new tile grown on the map), `partitions-new-typed` (a number being typed for
it), `partitions-resize` (Windows shrunk to 250 GiB, which only shrinks),
`partitions-label` (Data formatted ext4, its label being set — with `--pad`,
on the installer's board; a kept partition takes no label),
`partitions-other` (a mount point of one's own refused, and why),
`partitions-new-planned` (New Partition #1 added, the pending operations
naming it), `partitions-continue-refused` (a whole plan, nothing missing,
Continue refused as a live system refuses it, with the first thing the plan
asks that this version cannot carry out yet — drawn by the very function
that says it there, since the demo itself goes on),
`partitions-portrait-slid` (the light on the last tile, which a display
standing on its side slides the map to), `partitions-unmountable` (the
light on an encrypted partition, which is given no mount point, its details
saying why), `partitions-measured` (a home ext4 read — what it uses in its
details — and Resize over it from the least its own tool names),
`partitions-hibernated` (Windows read as hibernated: no Used, and under the
details what to do), `partitions-hibernated-other` (the Windows disk's own
Windows, read, held back by that hibernated one) and `partitions-reading` (a live system still
reading, "Reading…" as Used), all with an old msdos disk and a Linux SSD
the pictures of Where to install leave out,
`target-partitions-in-use` (Where to install with that whole plan the choice
in use: nothing ticked, the foot saying so), `desktop`, `desktop-options`,
`desktop-software`, `account`, `account-empty` as it is arrived at,
`desktop-loaders-plan` (the start-up managers with a plan on the old msdos
disk in use, systemd-boot outlined), `account-administrator`, `board`,
`review`, `confirm`, `review-partitions` and `confirm-partitions` (Review and
the last question with that plan in use), `confirm-plan-pages` and
`confirm-plan-last` (the plan's question on its first page and on its last,
the plan also writing a new table over a made-up archive disk of forty
partitions that these two pictures alone add),
`installing`, `installing-details`, `done`, `stopped`, `stopped-details`,
and with the plan that keeps Windows smaller rehearsed: `installing-shrink`
(its Resize step, "Shrinking what stays on the disk"), `done-shrink` (Done
saying Windows will check its disk), `stopped-shrink` (stopped while
shrinking: every part of the disk still there, the one being shrunk may
have changed), `stopped-hibernated` and `stopped-needs-check` (stopped in
Check, nothing changed), and `stopped-windows-in-use`,
`stopped-windows-check`, `stopped-windows-unread` and `stopped-bitlocker`
(stopped in Check, nothing changed, by what another Windows partition was
read as: in use, marked for Windows' check, not read, or a BitLocker
volume), and `stopped-windows-shrunk` (stopped in Partition once Windows'
partition was shrunk, saying under what to do that Windows will check its
disk the next time it starts)),
`--pad` draws it as with a controller in hand (the legend's pictures of a
pad's buttons, no scroll bars) — and with `--demo` opens a window so, a pad
in hand that a click does not put down and the first key does, which is how a
window in the sandbox, with no pad to read, is driven as a pad drives it —,
`--shells-board` as under LineXinBar with its keyboard typing into the field
the light is in (the room ending over where the shell settles it; the
keyboard itself is the shell's and is not drawn), and `--pointer X,Y` as with
the pointer resting at X,Y — what it rests on lit, a ring where it is; with a menu or the
question up, which take the pointer, nothing of the page is lit and no ring is
drawn, as in the running program. `--greeting N` starts Welcome on the Nth
of its twenty greetings (1 to 20), and `--search WORDS` types those words into
the search of `language-search`, `timezone-search` or `keyboard-search` in
place of the scene's own. The made-up machine starts in the session's language
(`LC_ALL`, `LC_MESSAGES` or `LANG`, as the words are chosen), as a live system
starts in its own, so a picture taken in Polish has Polish places, guesses and
greeting. A scene stands as the walk to it would have left the steps before
it: a scene past Time zone or Keyboard came through their Continue,
`timezone-places` shows the places opened long enough ago, and every
Keyboard scene but `keyboard-continue` has a kind chosen (Polish), so each
step's guess counts as seen there, and Continue pressed in the picture, or
after Back to the step, goes on as it would in the installer; `timezone`,
`timezone-continue`, `timezone-search`, `timezone-search-nothing` and
`keyboard-continue` are the steps as arrived at, the guess unseen.

A settled frame is what `--shot` takes. `--shot-at SECONDS` takes the frame
that long into a change instead, the frames drawn a sixtieth of a second
apart as in a window (the toolkit's `App::shot_after`): with `--from NAME`
the picture starts settled on that scene and the installer then goes to
`--scene`, on or back by where the two stand in the walk; with `--press NAME`
(accept, back, up, down, left, right or options) the picture starts settled
on `--scene` and the press is answered between the first frame and the
second, handed to the window as a key is (the toolkit's `Page::act`) — Back
closing Time zone's places, Options raising its menu, Accept dipping what the
light is on, Right moving the last question's light, Back closing a menu —
and `--then NAME --then-at SECONDS` presses again so long after it (Continue
and then Back at once), given again in pairs for every further press
(Options walked into Spare memory and out again);
with neither, the seconds count from `--scene` being set up, so a menu or the
question it raises is caught unfolding. Nought seconds into a change is the
scene before it, settled. A change `--from` a scene draws the page going with
the answers `--scene` sets up, so where the two scenes' answers differ (a
note You shows, a field Review fills) the page going is not quite the
`--from` picture; a `--press` is the change a person makes. `--low-end` draws
as the window draws on low-end hardware (the toolkit's `App::low_end`): every
frame at the one instant the wallpaper is held at there, the plain glass, the
new page already in place on the same frame, and every light standing on what
is chosen — the last question after Right below:

```sh
for t in 0.017 0.033 0.05 0.067 0.1 0.2; do
    lxb-installer --shot on-$t.png --from timezone --scene keyboard-layouts --shot-at $t
    lxb-installer --shot back-$t.png --from keyboard-layouts --scene timezone --shot-at $t
done
lxb-installer --shot trail.png --from timezone --scene timezone-places --shot-at 0.083
lxb-installer --shot closing.png --scene timezone-places --press back --shot-at 0.083
lxb-installer --shot turned.png --scene language --press accept --then back --then-at 0.05 --shot-at 0.1
lxb-installer --shot menu.png --scene target-options --shot-at 0.1
lxb-installer --shot low.png --from timezone --scene keyboard-layouts --shot-at 0.05 --low-end
lxb-installer --shot erase.png --scene confirm --low-end --press up --then right --then-at 0.4 --shot-at 0.6
lxb-installer --shot back.png --scene target-options --press down --then accept --then-at 0.3 \
    --then down --then-at 0.6 --then accept --then-at 0.9 --shot-at 1.4
```

The pictures in the README are taken at the **Indigo** accent, which is not any
particular machine's, in British English, through a scratch settings file
rather than by changing anybody's desktop:

```sh
mkdir -p /tmp/lxb-shot/lxb
printf 'accent = "Indigo"\n' > /tmp/lxb-shot/lxb/shell.toml
export XDG_CONFIG_HOME=/tmp/lxb-shot LC_ALL=en_GB.UTF-8

lxb-installer --shot docs/welcome.png    --scene welcome    --width 1600 --height 900
lxb-installer --shot docs/target.png     --scene target     --width 1600 --height 900
lxb-installer --shot docs/desktop.png    --scene desktop    --width 1600 --height 900
lxb-installer --shot docs/account.png    --scene account    --width 1600 --height 900
lxb-installer --shot docs/review.png     --scene review     --width 1600 --height 900
lxb-installer --shot docs/installing.png --scene installing --width 1600 --height 900
lxb-installer --shot docs/done.png       --scene done       --width 1600 --height 900
```

`done` carries one warning on purpose — a firmware entry that could not be
made — so the picture shows how the finished screen says what did not work.
