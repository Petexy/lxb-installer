# Linexin Installer — English (UK).
#
# This is the whole catalog and the fallback every other language falls back
# to. A message missing from a translation is answered from here; a message
# missing from here is answered with its own identifier, which is a hole
# somebody can see and search for.
#
# en-US.ftl beside this file is an overlay of the handful of words America
# writes differently, and nothing else.
#
# Plain words only: what a person reads says what they can do, never names a
# device, a filesystem or a command. A disk is "Example SSD · 1 TB", a part of
# it holds "Windows", and a refusal says what to do about it.

## The application itself

app-name = Linexin Installer

## The head of each screen: its title, and the one line under it

welcome-caption = A few questions, and Linexin is on this computer.
language-title = Language
language-caption = The language this computer speaks.
timezone-title = Time zone
timezone-caption = Where you are, so the clock is right.
keyboard-title = Keyboard
keyboard-caption = The keys you type on.
target-title = Where to install
target-caption = The disk Linexin goes on.
desktop-title = Desktop
desktop-caption = How Linexin looks. Each can be personalised later.
account-title = You
account-caption = Your name, and the account you sign in with.
review-title = Ready to install Linexin
review-caption = Look it over. Nothing has been changed yet.
installing-title = Installing Linexin
installing-caption = This takes a while. Nothing needs to be pressed.
done-title = Linexin is installed
done-caption = Restart the computer and take out the USB stick to start using it.
stopped-title = The installation stopped

## The verbs at the foot of a screen

button-continue = Continue
button-install = Install
button-close = Close
button-restart = Restart now
button-not-now = Not now
# Restart now where Done's row of buttons has no room for the whole of it
# beside Not now (a display standing on its side): the verb alone.
button-restart-short = Restart

## What the buttons do, in the legend at the foot of the panel

hint-choose = Choose
hint-back = Back
hint-quit = Leave
hint-type = Type
hint-shift = Shift
hint-close = Close
hint-done = Done
hint-options = Options
hint-details = Details
hint-hide-details = Hide details
hint-search = Search
# On Partitions' sheets: take hold of a size, keep it, put it back, and
# leave a sheet changing nothing.
hint-adjust = Adjust
hint-keep = Keep
hint-put-back = Put back
hint-cancel = Cancel

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = àé

## Questions, asked with the safe answer first

quit-title = Leave the installer?
quit-body = Nothing has been changed on this computer.
quit-stay = Stay
quit-leave = Leave
erase-title = Erase and install?
# The erase question where the dialog has no room for the whole of
# erase-title (a display standing on its side): the verb alone, the body and
# the answers under it saying the rest.
erase-title-short = Erase?
install-title = Install Linexin now?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = Everything on { $disk } will be erased.
erase-target-part = { $part } · { $size } on { $disk } will be erased.
erase-target-free = Linexin goes into the free space on { $disk }.
erase-home = { $part }, where your files will be kept, will be erased.
erase-final = This cannot be undone.
erase-nothing = Nothing on it is erased.
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = These operations change what is on your disks:
erase-plan-nothing = Nothing on your disks is erased.
# Under a new partition table's line on the plan's last question: each
# partition it erases, in GParted's words.
erase-plan-erased = Erases { $partition } ({ $filesystem }, { $size })
# A line of the plan's last question about an EFI system partition, or
# about one of Windows' own, named so after it.
erase-plan-esp = { $what } · EFI system partition
erase-plan-windows = { $what } · Windows
# Under each page of the plan's last question where it takes more than one.
erase-plan-page = Page { $page } of { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = “{ $label }”
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $part } on { $disk }
erase-back = Go back
erase-confirm = Erase and install
# The erase answer where the dialog has no room for the whole of
# erase-confirm beside erase-back (a display standing on its side): the
# verb alone, the title over it saying the rest.
erase-confirm-short = Erase
# Partitions' plan's last question goes on over pages where its list
# is longer than the dialog holds: the answer that turns to the next.
button-next = Next

## A line beside the legend

demo-note = A made-up computer: nothing here is real, and nothing is changed.
install-unavailable = Start this computer from a Linexin USB stick to install Linexin on it.
disks-unreadable = The disks of this computer could not be read.
# Install, pressed while Partitions is still reading how full the disks
# are (it holds the installer's lock while it reads).
install-reading = This computer's disks are still being read. Try again in a moment.
installing-note = Keep the computer switched on until this is done.
done-all-well = Everything went as planned.
done-warnings = { $count ->
    [one] Linexin is ready to use, but one thing did not go as planned:
   *[other] Linexin is ready to use, but a few things did not go as planned:
}
done-restart-failed = The computer could not be restarted. Restart it from the system menu.

## The fields

field-full-name = Full name
field-full-name-placeholder = Your name, as others will see it
field-username = Username
field-username-placeholder = What you sign in with
field-password = Password
field-password-placeholder = Choose a password
field-repeat = Confirm password
field-repeat-placeholder = The same password again
field-hostname = Computer name
field-hostname-placeholder = This computer's name on a network
field-root-password = Administrator password
field-root-password-placeholder = A password for system changes
field-root-repeat = Confirm administrator password
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = Weak
strength-fair = Fair
strength-good = Good
strength-strong = Strong

## You

# A separate administrator (root) account, off unless asked for.
account-root = Administrator account
account-root-off = Off: your own password makes changes
account-root-on = With its own password, below

## Sizes, as a box writes them

size-tb = { $amount } TB
size-gb = { $amount } GB
size-mb = { $amount } MB
size-kb = { $amount } kB
size-bytes = { $amount } bytes

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = Fast solid-state drive
disk-kind-sata = Internal drive
disk-kind-usb = USB drive
disk-kind-card = Memory card
disk-kind-virtual = Virtual disk
disk-kind-other = Disk
disk-empty = Empty
disk-holds-more = { $first } and { $count ->
    [one] one other part
   *[other] { $count } other parts
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what }, filling the whole disk

## What a part of a disk appears to hold

content-efi = Start-up files
content-bios-boot = Start-up space
content-reserved = Space Windows keeps
content-recovery = Windows recovery
content-windows = Windows
content-linux = Linux
content-swap = Spare memory
content-data = Files
content-empty = Empty
content-other = Something else

## Why a disk cannot be chosen, said under it

blocked-installation-media = The installer is running from this
blocked-in-use = In use right now
blocked-swap = In use as spare memory
blocked-holder = Used by something else on this computer
blocked-read-only = Can be read but not written to
blocked-too-small = Too small for Linexin
blocked-unknown = Cannot be used

## The review

# Each row names a step; under it, that step's answer.
review-where = Where
review-desktop = Desktop
review-region-name = Language and region
review-account-name = Account
review-extras = Extras
review-where-disk = { $disk } · everything on it is erased
review-where-free = { $disk } · in its free space
review-where-part = { $disk } · in place of { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = your files on { $part }, kept
review-where-home-erased = your files on { $part }, erased
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = Not chosen yet
review-extra-updates = Updates
review-extra-apps = { $count ->
    [one] One app
   *[other] { $count } apps
}
review-extra-snapshots = Restore points
review-extra-swap = { $size } of spare memory
review-extra-none = Nothing more
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size } on { $disk }
review-part-formatted = { $partition } · { $filesystem } · { $size } · formatted
review-part-kept = { $partition } · { $filesystem } · { $size } · kept
review-operations = { $count ->
    [one] One operation will be applied
   *[other] { $count } operations will be applied
}

## The installation, while it runs

installing-waiting = Getting ready…
installing-count = Step { $step } of { $steps } · { $percent }%
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = Where it stopped: { $step }
step-check = Checking the computer
# A layout made by hand on Partitions that keeps Windows, or another part, smaller than it was: that part is shrunk before anything else is changed.
step-resize = Shrinking what stays on the disk
step-partition = Making room on the disk
step-format = Preparing the disk
step-copy = Copying Linexin
step-swap = Setting aside spare memory
step-system = Setting up the system
step-region = Setting the language, time zone and keyboard
step-accounts = Creating your account
step-desktop = Setting up the desktop
step-updates = Installing updates
step-packages = Adding and removing software
step-flatpaks = Installing apps
step-drivers = Removing drivers this computer does not need
step-snapshots = Setting up restore points
step-initramfs = Preparing the system to start
step-bootloader = Making the computer start Linexin
step-finish = Finishing up

## What did not work, said once Linexin is installed

warning-updates-offline = Updates were left out because there was no internet. They can be installed later.
warning-updates-failed = Updates could not be installed. They can be installed later.
warning-flatpaks-failed = Some apps could not be installed. They can be added later.
warning-packages-kept = Some software that was to be removed is still there.
warning-boot-entry = If Linexin does not start, choose it in the computer's start-up menu.
warning-snapshots = Restore points could not be set up.
warning-drivers-kept = Drivers this computer does not need were left in place.
# After Windows' part of the disk was made smaller. Windows' own mark asking for a check of its disk is left on, so Windows checks it the next time it starts, and Linexin does not open that part until it has.
warning-windows-check = Let Windows check its disk the next time it starts. Until then, Linexin leaves Windows' files alone.

## Why the installation stopped, and what can be done about it

failure-invalid-request = The installer asked for something it cannot do
failure-invalid-request-do = Close the installer, open it again and go through the steps once more.
failure-consent-mismatch = What was going to be erased changed
failure-consent-mismatch-do = Nothing was erased. Go through the steps again and check the disk.
failure-not-root = The installer was not given permission
failure-not-root-do = Open the installer again and allow it when you are asked.
failure-not-live = This computer was not started from the installer
failure-not-live-do = Start the computer from the Linexin USB stick and try again.
failure-secure-boot = Secure Boot is on
failure-secure-boot-do = Turn Secure Boot off in the computer's start-up settings, then try again.
failure-already-running = Another installation is already running
failure-already-running-do = Wait for it to finish, or restart the computer and try again.
failure-missing-tool = Something the installer needs is missing
failure-missing-tool-do = Use a newer Linexin USB stick and try again.
failure-image-invalid = The copy of Linexin on the USB stick is damaged
failure-image-invalid-do = Write the USB stick again and try once more.
failure-disk-changed = The disk changed after it was chosen
failure-disk-changed-do = Some of the disk may already be erased. Go through the steps again and choose the disk once more.
failure-disk-changed-untouched-do = Nothing was erased. Go through the steps again and choose the disk once more.
failure-disk-busy = The disk is being used by something else
failure-disk-busy-do = Close whatever is using it, or restart the computer, and try again.
failure-disk-busy-untouched-do = Nothing was erased. Close whatever is using the disk, or restart the computer, and try again.
failure-no-space = There is not enough room
failure-no-space-do = Choose a bigger disk or more free space and try again.
failure-esp-full = The disk's start-up space is full
failure-esp-full-do = Erase the whole disk instead, or make room in its start-up space first.
# A part of the disk that was to be kept smaller could not be shrunk. Untouched: found before anything was written. Otherwise: while it was being shrunk, before any part of the disk was added or removed.
failure-shrink-refused = Part of the disk could not be shrunk
failure-shrink-refused-do = The disk's parts are all still there, but the one being shrunk may have changed. Let the system it belongs to check it, then try again.
failure-shrink-refused-untouched-do = Nothing was changed. Open the installer again and leave that part more room, or shrink it from the system it belongs to.
# Windows is hibernated, or was shut down by Fast Startup (Windows' own name for it), on this computer: none of its parts is shrunk. Found before anything was written.
failure-windows-hibernated = Windows is not fully shut down
failure-windows-hibernated-do = Nothing was changed. Start Windows, turn off Fast Startup, shut it down, then try again.
# A part of the disk that was to be shrunk must be checked by its own system first.
failure-needs-check = Part of the disk needs to be checked first
failure-needs-check-do = The disk's parts are all still there, but the one being shrunk may have changed. Start the system it belongs to, let it check its disk, then try again.
failure-needs-check-untouched-do = Nothing was changed. Start the system that part belongs to, let it check its disk, then try again.
# Another Windows partition on this computer than the one being shrunk is in use in this session (opened in the file manager, say), so whether Windows keeps it as it left it can't be read: no Windows partition is shrunk. Found before anything was written; each of the four below too.
failure-windows-in-use = Part of a disk with Windows is in use
failure-windows-in-use-do = Nothing was changed. Close whatever has Windows' files open, or restart the computer, then try again.
# Another Windows partition is marked for Windows' own check of its disk.
failure-windows-needs-check = Windows needs to check its disk first
failure-windows-needs-check-do = Nothing was changed. Start Windows, let it check its disk, shut it down, then try again.
# Another Windows partition could not be read (on another disk, or on the USB stick the installer started from), so whether Windows keeps it isn't known.
failure-windows-unread = Part of a disk with Windows couldn't be read
failure-windows-unread-do = Nothing was changed. Unplug any disk you aren't installing on and try again, or shrink Windows from Windows itself.
# A part of a disk on this computer is locked by BitLocker (Windows' own name for it), so whether its Windows is hibernated can't be read.
failure-bitlocker = Part of a disk is locked by BitLocker
failure-bitlocker-do = Nothing was changed. Turn off BitLocker in Windows and try again, or shrink Windows from Windows itself.
failure-needs-internet = This choice needs the internet
failure-needs-internet-do = Connect to the internet, or choose the Linexin desktop, which needs none.
failure-needs-internet-untouched-do = Nothing was erased. Connect to the internet, or choose the Linexin desktop, which needs none.
failure-command = Something went wrong while installing
failure-command-do = Look at the details, then try again. The disk may have to be erased.
failure-internal = The installer ran into a problem
failure-internal-do = Close it and try again. If it happens again, keep the details for a bug report.

## Why a choice cannot be installed, said beside it

invalid-username-empty = Choose a username
invalid-username-shape = Small letters, digits, - and _, starting with a letter
invalid-username-reserved = That name belongs to the system
invalid-full-name = A name cannot have : or , in it
invalid-hostname = Letters, digits and -, at most 63 of them
invalid-password-empty = Choose a password
invalid-password-mismatch = The two passwords are not the same
invalid-password-shape = At most 128 characters, with no line breaks
invalid-root-password-empty = Choose a password for the administrator account
invalid-locale = Choose a language
invalid-timezone = Choose a time zone
invalid-keyboard = Choose a keyboard
invalid-disk-blocked = This disk cannot be used
invalid-target-missing = Choose where to install
invalid-target-small = There is not enough room for Linexin there
table-full = This disk has no room for another part. Replace one instead.
invalid-target-esp = That part is what starts the computer
target-protected = Windows keeps that part for itself
invalid-home = That part cannot hold your files
invalid-swap = The spare memory is bigger than the room there
invalid-bootloader = That start-up manager cannot be used on this computer
invalid-source = The copy of Linexin to install cannot be found
invalid-layout = Your partitions cannot be used as planned

## Welcome

# Welcome's greeting and its button are not here: they take turns in the
# earlier installer's twenty languages, each in its own words, whatever
# catalog is in force (src/ui/screens/welcome.rs). Its head is app-name and
# welcome-caption.

## The time zone step

timezone-heading-region = Part of the world
timezone-heading-city = Nearest city
# Over the places a search found, from every part of the world.
timezone-heading-found = Places found
# Under a place a search found: its country (as the time zone database names it, in English) and its part of the world.
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] One place
   *[other] { $count } places
}
zone-region-africa = Africa
zone-region-america = The Americas
zone-region-antarctica = Antarctica
zone-region-arctic = The Arctic
zone-region-asia = Asia
zone-region-atlantic = Atlantic Ocean
zone-region-australia = Australia
zone-region-europe = Europe
zone-region-indian = Indian Ocean
zone-region-pacific = Pacific Ocean
zone-region-utc = Universal time
# The one place under Universal time: the clock no country keeps.
zone-utc = Coordinated Universal Time

## The keyboard step

keyboard-heading-layout = Layout
keyboard-heading-variant = Kind
# Over the kinds a search found, of every layout.
keyboard-heading-found = Kinds found
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] Standard only
    [one] Standard and one other
   *[other] Standard and { $count } others
}
# Under the plain layout, first among its kinds.
keyboard-plain = Standard
keyboard-test = Try your keyboard
keyboard-test-placeholder = Type something to try the keys

## The search over a long list (Language, Time zone, Keyboard)

# The search field's heading.
search-field = Search
search-language-placeholder = A language, also in English
search-timezone-placeholder = A city, a country or a part of the world
search-keyboard-placeholder = A layout, or a kind of one
# Where a search finds nothing; the query is passed as it was typed.
search-nothing = Nothing found for “{ $query }”

## The step of where to install

button-options = Options
target-heading-disks = Disks
target-erase = Erase this disk
target-erase-note = Everything on it is erased
target-free = Free space · { $size }
target-free-note = Nothing on the disk is erased
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = Replace { $what } · { $size }
target-replace-note = What is on it is erased
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = Replace “{ $label }” · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · erased
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = “{ $label }” · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = Free space
target-no-disk = There is no disk Linexin can be installed on.
target-options = Options
# Each option with the state it is in.
target-option-storage = Storage · { $value }
target-option-swap = Spare memory · { $value }
target-option-home = Your files · { $value }
target-storage = Storage
target-storage-standard = Standard
target-storage-snapshots = With restore points
# The Options row says it where the menu has no room for the whole of
# target-storage-snapshots beside "Storage" (a 16:9 display standing on
# its side): shorter words for the same.
target-storage-snapshots-short = Restore points
target-swap = Spare memory
target-swap-off = Off
target-home = Your files
target-home-system = With the system
target-home-kept = { $part }, kept
target-home-erased = { $part }, erased
target-home-keep = Keep what is on it
target-home-erase = Erase it
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = Using your partitions · { $count ->
    [one] one operation
   *[other] { $count } operations
}
target-partitions-let-go = A disk changed, so your partitions were cleared. Choose again.

## Partitions: Where to install's Advanced page, in GParted's words.
## Filesystem names (ntfs, ext4, linux-swap, extended), device paths, flags
## and units (MiB, GiB) are written by the program and never translated.

# The button on Where to install that opens the partitioning tool.
button-advanced = Advanced
partitions-title = Partitions
# Said in the foot when Continue is pressed on Partitions on a live system
# and the plan is not one this version carries out: it mixes kinds of work
# no single test of this version did together, or asks something none did
# at all (a disk of 4096-byte sectors, for one). The same words stop it at
# Install.
partitions-not-yet = Partitions can't be applied in this version yet.
# What Continue would say were the first thing the plan asks something this
# version carries out in no plan at all, the first of: a new partition
# table, making or deleting partitions, formatting one, keeping a /home,
# several disks, a computer started by BIOS, shrinking. This version carries
# each of them out in some plan, so on a live system the line above is said
# instead.
partitions-not-yet-fresh-table = A new partition table can't be applied in this version yet.
partitions-not-yet-keep-table = Making or deleting partitions can't be applied in this version yet.
partitions-not-yet-change-entries = Formatting a partition can't be applied in this version yet.
partitions-not-yet-kept-home = Keeping a /home can't be applied in this version yet.
partitions-not-yet-several-disks = A plan over several disks can't be applied in this version yet.
partitions-not-yet-bios = Partitions on a computer started by BIOS can't be applied in this version yet.
partitions-not-yet-shrink = Shrinking a partition can't be applied in this version yet.
# A run of space on a disk no partition uses, as GParted calls it.
partitions-unallocated = unallocated
# A partition whose filesystem could not be read.
partitions-unknown = unknown
# In the head, after a disk's size, for a disk with no partition table.
partitions-no-table = no partition table
# The names in the details under the map.
partitions-partition = Partition
partitions-filesystem = File system
partitions-label = Label
partitions-size = Size
partitions-used = Used
partitions-mount-point = Mount point
partitions-flags = Flags
# What will be done to the partition when the plan is applied.
partitions-pending = Pending
# Where a run of unallocated space starts and ends on the disk.
partitions-start = Start
partitions-end = End
# What the details say as Used while a live system is still reading how
# full the disks are.
partitions-reading = Reading…

## Partitions' plan: its menus, its pending operations and what Continue
## needs. Devices (/dev/sda4), filesystems (ext4, fat32, linux-swap), tables
## (gpt, msdos), mount points (/, /home, /efi, /boot, swap), sizes and
## units are passed as values and never translated.

# A tile's menu: the mount point in force (a path, or partitions-none), and
# what the partition is formatted as (a filesystem, or partitions-keep).
partitions-menu-mount = Mount point · { $value }
partitions-menu-format = Format as · { $value }
partitions-format-as = Format as
partitions-none = none
partitions-keep = keep
partitions-delete = Delete
# Takes away every pending operation on one partition.
partitions-undo-part = Undo changes to { $partition }
# Undo changes to a partition, where the menu has no room for its device
# (its title names it).
partitions-undo = Undo changes
# A disk's menu, and the menu of tables it opens.
partitions-create-table = Create partition table
partitions-create-table-kind = Create { $table } partition table
partitions-table = Partition table
# Options on Partitions.
partitions-undo-last = Undo last operation
partitions-clear = Clear all operations
partitions-pending-list = Pending operations ({ $count })…
partitions-swap-file = Swap file · { $value }
partitions-swap-file-title = Swap file
# The list of pending operations under the details, and the whole list.
partitions-pending-title = Pending operations
# The title of the whole list where the dialog has no room for
# partitions-pending-title (a display standing on its side).
partitions-pending-short = Pending
partitions-pending-none = No pending operations
# Said over the list where the oldest operations are left out of it.
partitions-pending-earlier = { $count ->
    [one] One earlier operation
   *[other] { $count } earlier operations
}
# On a tile of the map: a pending format, and the partitions a run of
# unallocated space was (sda4, or sda4, sda5; count is how many).
partitions-tile-format = format
partitions-tile-deleted = { $count ->
   *[other] { $partitions } deleted
}
# What the details say is pending for a partition.
partitions-do-format = Format as { $filesystem }
partitions-do-mount = Mount at { $path }
partitions-do-delete = Delete { $partition }
# The pending operations, in GParted's words.
partitions-op-delete = Delete { $partition } ({ $filesystem }, { $size })
partitions-op-format = Format { $partition } as { $filesystem }
partitions-op-mount = Set mount point of { $partition } to { $path }
partitions-op-table = Create { $table } partition table on { $disk }
partitions-op-create = Create Primary Partition #{ $number } ({ $filesystem }, { $size }) on { $disk }
partitions-op-create-at = Create Primary Partition #{ $number } ({ $filesystem }, { $size }, { $path }) on { $disk }
partitions-op-shrink = Shrink { $partition } from { $from } to { $to }
partitions-op-grow = Grow { $partition } from { $from } to { $to }
partitions-op-label = Set file system label “{ $label }” on { $partition }
partitions-op-label-clear = Clear file system label on { $partition }
# What Continue still needs, said in the foot and when Continue is refused.
partitions-missing-root = Missing: a partition mounted at /
partitions-missing-efi = Missing: an EFI partition at /boot or /efi
partitions-missing-boot = Add a /boot, or mount the EFI partition at /boot
partitions-missing-bios-grub = Missing: a bios_grub partition on { $disk }
partitions-too-small = { $path } needs at least { $size }
partitions-must-format = Format { $partition } to use it at { $path }
partitions-needs = { $path } needs { $filesystem }
partitions-needs-linux = { $path } needs ext4, btrfs or xfs
# / is made only as ext4 or btrfs.
partitions-needs-root = { $path } needs ext4 or btrfs
partitions-twice = { $path } is set on more than one partition
partitions-blocked = Nothing can be changed on { $disk }
# What the plan says of a disk is not the disk as it was read (never
# made by Partitions itself).
partitions-refused-mismatch = The plan for { $disk } doesn't match the disk
# A partition made or resized off a whole MiB.
partitions-refused-unaligned = { $partition } must start and end on a whole MiB
# Past what the disk's partition table reaches.
partitions-refused-outside = { $partition } reaches past the room on its disk
partitions-refused-overlap = { $partition } overlaps another partition
# Something made on an msdos disk that keeps an extended partition.
partitions-refused-beside-extended = { $disk }: no new partitions beside an extended one
# A logical partition deleted while one after it is kept.
partitions-refused-logical = Delete the logical partitions after { $partition } first
partitions-refused-entries = { $disk } would hold more than 128 partitions
# A partition whose file system cannot be shrunk.
partitions-refused-shrink = { $partition } can't be shrunk
# A kept partition made larger: kept partitions only shrink.
partitions-refused-grow = { $partition } is kept, so it can't grow
# A bios_grub partition anywhere but a gpt disk a BIOS starts, of
# anything but 1 MiB.
partitions-refused-bios-grub = { $partition } can't be a bios_grub partition here
# A label its file system cannot hold.
partitions-refused-label = { $partition } has a label its file system can't hold
# A kept file system the installer cannot mount (BitLocker, LUKS, LVM,
# RAID members).
partitions-refused-unmountable = { $filesystem } on { $partition } can't be mounted
# Something mounted inside the folder a kept partition is mounted at (the
# folder named): nothing is made inside a kept partition's folders, and the
# folder for the mount would have to be made in it.
partitions-refused-inside-kept = { $path } is kept, so nothing can be mounted inside it
# A file system made on a partition smaller than its tools make one.
partitions-refused-fs-small = { $partition } is too small for { $filesystem }
# Restore points on a Btrfs / beside a separate /var.
partitions-refused-restore-points = Restore points can't be kept beside a separate /var
# /boot on another disk than /efi (UEFI) or / (BIOS), the one named.
partitions-refused-boot-disk = /boot must be on the same disk as { $path }
# Something mounted at /efi on a computer a BIOS starts.
partitions-refused-efi-bios = /efi is used only where UEFI starts the computer
# On BIOS, a partition in the first MiB of the msdos disk / is on.
partitions-refused-grub-gap = GRUB needs the first MiB of { $disk } free
# The start-up manager chosen cannot start Linexin from the partitions
# planned (its name, never translated but for Automatic).
partitions-refused-bootloader = { $loader } can't start Linexin from these partitions
# How much of a partition is used has not been read (a shrink, a kept
# EFI partition at /boot).
partitions-refused-not-read = How much of { $partition } is used isn't known yet
# Shrunk below what it uses and a margin.
partitions-refused-shrink-below = { $partition } can't be smaller than { $size }
# Why a partition cannot be shrunk, as reading it said.
partitions-refused-hibernated = { $partition } can't be shrunk: Windows is hibernated
partitions-refused-needs-check = { $partition } can't be shrunk until it is checked
partitions-refused-encrypted = { $partition } is encrypted, so it can't be shrunk
partitions-refused-in-use = { $partition } is in use, so it can't be shrunk
partitions-refused-unreadable = { $partition } can't be read, so it can't be shrunk
# Of an NTFS that holds no hiberfil.sys of its own ($partition, its
# device): a Windows' data partition, or the partition of a Windows whose
# hibernation is switched off, which the installer cannot tell apart. It
# is never shrunk here, but Windows itself can shrink it.
partitions-refused-no-hiberfile = { $partition } can't be shrunk here: shrink it in Windows
# Why no Windows partition can be shrunk, as reading another Windows
# partition said ($other, its device): it could not be read, or it is
# locked by BitLocker. (Hibernated says partitions-refused-hibernated.)
partitions-refused-windows-unread = Can't shrink: Windows on { $other } couldn't be read
partitions-refused-bitlocker = Can't shrink: { $other } is locked by BitLocker
# The same, under the partition's own details, with what to do — the
# partition itself is the one the light is on: Windows on $other
# hibernated (or shut down by Fast Startup, which hibernates it), Windows
# on $other not read, BitLocker on $other.
partitions-why-hibernated = Windows on { $other }: shut it down with Fast Startup off
partitions-why-windows-unread = Can't shrink until Windows on { $other } can be read
partitions-why-bitlocker = To shrink, turn off BitLocker on { $other }
# The same of an NTFS with no hiberfil.sys of its own (as
# partitions-refused-no-hiberfile): Windows' own tool for shrinking a
# partition, by the name Windows gives it in this language.
partitions-why-no-hiberfile = To shrink it, use Disk Management in Windows
# A kept EFI partition with less free than it must have.
partitions-refused-esp-full = { $partition } needs { $size } free
# A partition formatted for a folder the system's own files go in (/var,
# /opt…), when how much goes there is not known.
partitions-refused-image-unknown = How much Linexin puts in { $path } isn't known yet
# partitions-refused-not-read and partitions-refused-image-unknown once
# reading the disks has ended without the number: refused, failed, or
# answered without it.
partitions-refused-unread = How much of { $partition } is used couldn't be read
partitions-refused-image-unread = How much Linexin puts in { $path } couldn't be read

## Partitions' sheets: New partition, Resize, Label… and a mount point of
## one's own (Other…). Devices, filesystems, paths and sizes are passed as
## values and never translated.

# A tile's menu: New… on unallocated space, Resize… and Label… on a
# partition; Other… in Mount point, for a path typed in.
partitions-new = New…
partitions-resize = Resize…
partitions-relabel = Label…
partitions-other = Other…
# A partition the plan makes, named as GParted names it until it is made.
partitions-new-name = New Partition #{ $number }
# On its tile in the map.
partitions-tile-new = new
# The sheets' titles.
partitions-new-title = New partition
partitions-resize-title = Resize { $partition }
partitions-label-title = Label of { $partition }
partitions-other-title = Mount point of { $partition }
# The sheets' rows: the size held on Resize, where a new partition stands
# in the unallocated space it is made in, and GRUB's own partition in the
# list of filesystems.
partitions-new-size = New size
partitions-position = Position
partitions-position-start = start of the space
partitions-position-end = end of the space
partitions-bios-grub = bios_grub (unformatted)
# The sheets' buttons.
partitions-cancel = Cancel
partitions-add = Add
partitions-resize-button = Resize
partitions-set = Set
# Under a size held on Resize: what is left of the unallocated space after
# the partition, or what of it the partition takes.
partitions-leaves = Leaves { $size } unallocated after it
partitions-leaves-none = Leaves no unallocated space after it
partitions-takes = Takes { $size } of the unallocated space after it
partitions-takes-all = Takes all of the unallocated space after it
# Under a size where the space is smaller than the partition must be.
partitions-at-least = Needs at least { $size }
# A mount point of one's own: the field's hint, and why a path is refused.
partitions-path-placeholder = For example /data
partitions-path-whole = A mount point is a whole path from /, such as /data
partitions-path-twice = { $path } is already a mount point
partitions-path-system = { $path } belongs to the system
# A path longer than a mount point may be (255 bytes).
partitions-path-long = A mount point is 255 bytes at most
# A path with a space, a tab or a backslash in it, which is refused rather
# than escaped.
partitions-path-spaces = A mount point has no spaces or backslashes
# A path inside /home, /boot or /efi (the one named).
partitions-path-under = Nothing can be mounted inside { $path }
# How long a label each filesystem holds, said under the field.
partitions-label-bytes = { $filesystem } labels hold { $count ->
    [one] one byte
   *[other] { $count } bytes
} at most
partitions-label-chars = { $filesystem } labels hold { $count ->
    [one] one character
   *[other] { $count } characters
} at most
# A "/" typed or pasted into a label, which no label holds (the label goes
# onto the command line of the tool that makes the file system).
partitions-label-slash = A label can't hold “/”
# Why unallocated space offers no New…
partitions-why-table = No partition table yet: choose this space to create one
partitions-why-primaries = An msdos disk holds four primary partitions at most
partitions-why-logical = Logical partitions can't be made here
# On an msdos disk that keeps an extended partition: nothing new is made.
partitions-why-beside-extended = Nothing can be made beside an extended partition
# On a disk something is written over as a whole (a file system, LUKS, LVM
# or RAID on the disk itself, even beside a partition table): no table and no
# partition is made on it. Said under every tile of that disk.
partitions-why-whole = Something fills the whole disk: no table or partition can be made on it
# Why a kept partition is given no mount point: its file system (BitLocker,
# LUKS, an LVM or RAID member, swap) is none the installer mounts.
partitions-why-unmountable = { $filesystem } can't be mounted: format it to use it
# The same, for a partition holding no file system at all.
partitions-why-unformatted = No file system: format it to use it
# Why a logical partition offers no Delete: one numbered after it is kept,
# and the kernel would number that one anew. The kernel numbers logical
# partitions as their tables are chained, which need not be the order they
# stand in on the disk, so "after it" is by number, not by place.
partitions-why-logical-delete = Delete the logical partitions numbered after it first
# What Continue still needs: the tool makes no logical partitions.
partitions-primaries = { $disk } would hold more than four primary partitions
# What the details say is pending.
partitions-do-create = Create
partitions-do-shrink = Shrink to { $size }
partitions-do-grow = Grow to { $size }

## The desktop step

button-done = Done
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = Built on GNOME
desktop-about-kinexin = Built on KDE Plasma
desktop-about-linexinbar = Made for a controller
# Under a desktop: whether it has to be downloaded.
desktop-ready = No internet needed
desktop-needs-internet = Needs the internet
desktop-refused = { $desktop } needs the internet. Connect, or choose Linexin.
desktop-checking = Checking the internet connection…
desktop-checking-short = Checking the internet…
desktop-offline = There is no internet, so what needs it is left out.
desktop-updates = Install updates
desktop-updates-on = Everything brought up to date
desktop-updates-off = As it is on the USB stick
updates-refused = Updates need the internet. They can be installed later.
desktop-options-title = Options
desktop-options-caption = What else is installed, and how the computer starts.
desktop-option-remove = Software to remove
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = Software
desktop-flatpaks-title = Apps
desktop-bootloader-title = Start-up
desktop-remove-caption = Switch off what you do not want on the installed system.
desktop-remove-looking = Looking at the software…
desktop-remove-unlisted = The software could not be listed
desktop-remove-none = There is nothing to remove
desktop-remove-nothing = Nothing is removed
desktop-remove-count = { $count ->
    [one] One left out
   *[other] { $count } left out
}
desktop-option-flatpaks = Apps from Flathub
desktop-flatpaks-caption = Installed from the internet along with Linexin.
# How many of the apps offered are switched on.
desktop-flatpaks-count = { $count } of { $total } chosen
desktop-flatpak-on = Installed with Linexin
desktop-flatpak-off = Left out
flatpaks-refused = Apps from Flathub need the internet. They can be added later.
desktop-option-bootloader = Start-up manager
desktop-bootloader-caption = What starts Linexin when the computer is switched on.
desktop-option-drivers = Remove unused drivers
desktop-drivers-on = Unused ones are removed
desktop-drivers-off = Every driver is kept
desktop-option-snapshots = Restore points
desktop-snapshots-on = Saved before every update
desktop-snapshots-off = None are saved
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = Set up under Where to install
# Under Restore points while the partitions made on Partitions put /var
# on a partition of its own, beside which they are switched off.
desktop-snapshots-var = Not kept beside a separate /var
bootloader-automatic = Automatic
bootloader-automatic-note = Chosen to suit this computer
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = Starts Linexin straight away, with no menu
bootloader-grub = GRUB
bootloader-grub-note = A menu at start-up, on any computer
bootloader-refind = rEFInd
bootloader-refind-note = A menu of every system on the computer
bootloader-unavailable = Cannot be used on this computer
# The groups the software to remove is listed in.
package-group-applications = Apps
package-group-multimedia = Sound and video
package-group-network = Network
package-group-fonts = Fonts
package-group-libraries = Libraries
