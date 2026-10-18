# Linexin Installer — German.
#
# Every message of en-GB.ftl, in German. The comments are en-GB's: they say
# where a message appears and what it means. The words the family already
# uses — Zurück, Weiter, Optionen, Schließen, Wählen, Jetzt neu starten,
# Rechner, Laufwerk — are the shell's and the toolkit's, and so is the polite
# "Sie". A name a person or a disk gave stands in German quotes, „ “.

## The application itself

app-name = Linexin-Installer

## The head of each screen: its title, and the one line under it

welcome-caption = Ein paar Fragen, und Linexin ist auf diesem Rechner.
language-title = Sprache
language-caption = Die Sprache, die dieser Rechner spricht.
timezone-title = Zeitzone
timezone-caption = Wo Sie sind, damit die Uhr richtig geht.
keyboard-title = Tastatur
keyboard-caption = Die Tasten, auf denen Sie tippen.
target-title = Wohin installieren
target-caption = Das Laufwerk, auf das Linexin kommt.
desktop-title = Desktop
desktop-caption = Wie Linexin aussieht. Jeder lässt sich später anpassen.
account-title = Über Sie
account-caption = Ihr Name und das Konto, mit dem Sie sich anmelden.
review-title = Bereit, Linexin zu installieren
review-caption = Sehen Sie alles noch einmal an. Noch wurde nichts verändert.
installing-title = Linexin wird installiert
installing-caption = Das dauert eine Weile. Sie müssen nichts drücken.
done-title = Linexin ist installiert
done-caption = Starten Sie den Rechner neu und ziehen Sie den USB-Stick ab, um loszulegen.
stopped-title = Die Installation wurde abgebrochen

## The verbs at the foot of a screen

button-continue = Weiter
button-install = Installieren
button-close = Schließen
button-restart = Jetzt neu starten
button-not-now = Nicht jetzt
button-restart-short = Neu starten

## What the buttons do, in the legend at the foot of the panel

hint-choose = Wählen
hint-back = Zurück
hint-quit = Beenden
hint-type = Tippen
hint-shift = Shift
hint-close = Schließen
hint-done = Fertig
hint-options = Optionen
hint-details = Details
hint-hide-details = Details ausblenden
hint-search = Suchen
hint-adjust = Anpassen
hint-keep = Übernehmen
hint-put-back = Zurücksetzen
hint-cancel = Abbrechen

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = äö

## Questions, asked with the safe answer first

quit-title = Installer beenden?
quit-body = Auf diesem Rechner wurde nichts verändert.
quit-stay = Bleiben
quit-leave = Beenden
erase-title = Löschen und installieren?
erase-title-short = Löschen?
install-title = Linexin installieren?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = Alles auf { $disk } wird gelöscht.
erase-target-part = { $part } · { $size } auf { $disk } wird gelöscht.
erase-target-free = Linexin kommt in den freien Platz auf { $disk }.
erase-home = { $part }, wo Ihre Dateien liegen werden, wird gelöscht.
erase-final = Das lässt sich nicht rückgängig machen.
erase-nothing = Darauf wird nichts gelöscht.
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = Diese Vorgänge verändern, was auf Ihren Laufwerken ist:
erase-plan-nothing = Auf Ihren Laufwerken wird nichts gelöscht.
erase-plan-erased = Löscht { $partition } ({ $filesystem }, { $size })
erase-plan-esp = { $what } · EFI-Systempartition
erase-plan-windows = { $what } · Windows
erase-plan-page = Seite { $page } von { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = „{ $label }“
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $part } auf { $disk }
erase-back = Zurück
erase-confirm = Löschen und installieren
erase-confirm-short = Löschen
button-next = Weiter

## A line beside the legend

demo-note = Ein erfundener Rechner: Nichts hier ist echt, und nichts wird verändert.
install-unavailable = Starten Sie diesen Rechner vom Linexin-USB-Stick, um Linexin zu installieren.
disks-unreadable = Die Laufwerke dieses Rechners konnten nicht gelesen werden.
install-reading = Die Laufwerke dieses Rechners werden noch gelesen. Versuchen Sie es gleich noch einmal.
installing-note = Lassen Sie den Rechner eingeschaltet, bis alles fertig ist.
done-all-well = Alles hat wie geplant geklappt.
done-warnings = { $count ->
    [one] Linexin ist bereit, aber eine Sache hat nicht wie geplant geklappt:
   *[other] Linexin ist bereit, aber einige Dinge haben nicht wie geplant geklappt:
}
done-restart-failed = Der Rechner konnte nicht neu gestartet werden. Starten Sie ihn über das Systemmenü neu.

## The fields

field-full-name = Vollständiger Name
field-full-name-placeholder = Ihr Name, wie andere ihn sehen
field-username = Benutzername
field-username-placeholder = Damit melden Sie sich an
field-password = Passwort
field-password-placeholder = Wählen Sie ein Passwort
field-repeat = Passwort bestätigen
field-repeat-placeholder = Dasselbe Passwort noch einmal
field-hostname = Rechnername
field-hostname-placeholder = So heißt dieser Rechner im Netzwerk
field-root-password = Administratorpasswort
field-root-password-placeholder = Ein Passwort für Systemänderungen
field-root-repeat = Administratorpasswort bestätigen
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = Schwach
strength-fair = Mittel
strength-good = Gut
strength-strong = Stark

## You

# A separate administrator (root) account, off unless asked for.
account-root = Administratorkonto
account-root-off = Aus: Ihr eigenes Passwort erlaubt Änderungen
account-root-on = Mit eigenem Passwort, unten

## Sizes, as a box writes them

size-tb = { $amount } TB
size-gb = { $amount } GB
size-mb = { $amount } MB
size-kb = { $amount } kB
size-bytes = { $amount } Byte

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = Schnelle SSD
disk-kind-sata = Internes Laufwerk
disk-kind-usb = USB-Laufwerk
disk-kind-card = Speicherkarte
disk-kind-virtual = Virtuelles Laufwerk
disk-kind-other = Laufwerk
disk-empty = Leer
disk-holds-more = { $first } und { $count ->
    [one] ein weiterer Bereich
   *[other] { $count } weitere Bereiche
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what }, auf dem ganzen Laufwerk

## What a part of a disk appears to hold

content-efi = Startdateien
content-bios-boot = Startbereich
content-reserved = Von Windows reserviert
content-recovery = Windows-Wiederherstellung
content-windows = Windows
content-linux = Linux
content-swap = Auslagerungsspeicher
content-data = Dateien
content-empty = Leer
content-other = Etwas anderes

## Why a disk cannot be chosen, said under it

blocked-installation-media = Davon läuft der Installer
blocked-in-use = Wird gerade verwendet
blocked-swap = Wird als Auslagerungsspeicher verwendet
blocked-holder = Wird von etwas anderem auf diesem Rechner verwendet
blocked-read-only = Lesbar, aber nicht beschreibbar
blocked-too-small = Zu klein für Linexin
blocked-unknown = Nicht verwendbar

## The review

# Each row names a step; under it, that step's answer.
review-where = Wo
review-desktop = Desktop
review-region-name = Sprache und Region
review-account-name = Konto
review-extras = Extras
review-where-disk = { $disk } · alles darauf wird gelöscht
review-where-free = { $disk } · im freien Platz
review-where-part = { $disk } · anstelle von { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = Ihre Dateien auf { $part }, behalten
review-where-home-erased = Ihre Dateien auf { $part }, gelöscht
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = Noch nicht gewählt
review-extra-updates = Aktualisierungen
review-extra-apps = { $count ->
    [one] Eine App
   *[other] { $count } Apps
}
review-extra-snapshots = Wiederherstellungspunkte
review-extra-swap = { $size } Auslagerungsspeicher
review-extra-none = Sonst nichts
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size } auf { $disk }
review-part-formatted = { $partition } · { $filesystem } · { $size } · formatiert
review-part-kept = { $partition } · { $filesystem } · { $size } · behalten
review-operations = { $count ->
    [one] Ein Vorgang wird angewendet
   *[other] { $count } Vorgänge werden angewendet
}

## The installation, while it runs

installing-waiting = Wird vorbereitet…
installing-count = Schritt { $step } von { $steps } · { $percent } %
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = Abgebrochen bei: { $step }
step-check = Rechner wird geprüft
step-resize = Was auf dem Laufwerk bleibt, wird verkleinert
step-partition = Platz auf dem Laufwerk wird geschaffen
step-format = Laufwerk wird vorbereitet
step-copy = Linexin wird kopiert
step-swap = Auslagerungsspeicher wird eingerichtet
step-system = System wird eingerichtet
step-region = Sprache, Zeitzone und Tastatur werden eingestellt
step-accounts = Ihr Konto wird angelegt
step-desktop = Desktop wird eingerichtet
step-updates = Aktualisierungen werden installiert
step-packages = Software wird hinzugefügt und entfernt
step-flatpaks = Apps werden installiert
step-drivers = Nicht benötigte Treiber werden entfernt
step-snapshots = Wiederherstellungspunkte werden eingerichtet
step-initramfs = Systemstart wird vorbereitet
step-bootloader = Rechner wird zum Starten von Linexin eingerichtet
step-finish = Wird abgeschlossen

## What did not work, said once Linexin is installed

warning-updates-offline = Aktualisierungen wurden mangels Internet ausgelassen. Sie lassen sich später installieren.
warning-updates-failed = Aktualisierungen konnten nicht installiert werden. Sie lassen sich später installieren.
warning-flatpaks-failed = Einige Apps konnten nicht installiert werden. Sie lassen sich später hinzufügen.
warning-packages-kept = Einige Software, die entfernt werden sollte, ist noch vorhanden.
warning-boot-entry = Falls Linexin nicht startet, wählen Sie es in der Startauswahl des Rechners.
warning-snapshots = Wiederherstellungspunkte konnten nicht eingerichtet werden.
warning-drivers-kept = Nicht benötigte Treiber sind noch vorhanden.
warning-windows-check = Lassen Sie Windows beim nächsten Start sein Laufwerk prüfen. Bis dahin lässt Linexin es ruhen.

## Why the installation stopped, and what can be done about it

failure-invalid-request = Der Installer hat etwas verlangt, das er nicht kann
failure-invalid-request-do = Schließen Sie den Installer, öffnen Sie ihn erneut und gehen Sie die Schritte noch einmal durch.
failure-consent-mismatch = Was gelöscht werden sollte, hat sich geändert
failure-consent-mismatch-do = Es wurde nichts gelöscht. Gehen Sie die Schritte erneut durch und prüfen Sie das Laufwerk.
failure-not-root = Der Installer hat keine Erlaubnis bekommen
failure-not-root-do = Öffnen Sie den Installer erneut und erlauben Sie es, wenn Sie gefragt werden.
failure-not-live = Dieser Rechner wurde nicht vom Installer gestartet
failure-not-live-do = Starten Sie den Rechner vom Linexin-USB-Stick und versuchen Sie es erneut.
failure-secure-boot = Secure Boot ist eingeschaltet
failure-secure-boot-do = Schalten Sie Secure Boot in den Starteinstellungen des Rechners aus und versuchen Sie es dann erneut.
failure-already-running = Es läuft bereits eine andere Installation
failure-already-running-do = Warten Sie, bis sie fertig ist, oder starten Sie den Rechner neu und versuchen Sie es erneut.
failure-missing-tool = Dem Installer fehlt etwas, das er braucht
failure-missing-tool-do = Verwenden Sie einen neueren Linexin-USB-Stick und versuchen Sie es erneut.
failure-image-invalid = Die Linexin-Kopie auf dem USB-Stick ist beschädigt
failure-image-invalid-do = Beschreiben Sie den USB-Stick neu und versuchen Sie es noch einmal.
failure-disk-changed = Das Laufwerk hat sich verändert, nachdem es gewählt wurde
failure-disk-changed-do = Ein Teil des Laufwerks ist womöglich schon gelöscht. Gehen Sie die Schritte erneut durch und wählen Sie das Laufwerk noch einmal.
failure-disk-changed-untouched-do = Es wurde nichts gelöscht. Gehen Sie die Schritte erneut durch und wählen Sie das Laufwerk noch einmal.
failure-disk-busy = Das Laufwerk wird von etwas anderem verwendet
failure-disk-busy-do = Schließen Sie, was es verwendet, oder starten Sie den Rechner neu, und versuchen Sie es erneut.
failure-disk-busy-untouched-do = Es wurde nichts gelöscht. Schließen Sie, was das Laufwerk verwendet, oder starten Sie den Rechner neu, und versuchen Sie es erneut.
failure-no-space = Es ist nicht genug Platz
failure-no-space-do = Wählen Sie ein größeres Laufwerk oder mehr freien Platz und versuchen Sie es erneut.
failure-esp-full = Der Startbereich des Laufwerks ist voll
failure-esp-full-do = Löschen Sie stattdessen das ganze Laufwerk, oder schaffen Sie zuerst Platz in seinem Startbereich.
failure-shrink-refused = Ein Bereich des Laufwerks ließ sich nicht verkleinern
failure-shrink-refused-do = Alle Bereiche des Laufwerks sind noch da, aber der zu verkleinernde hat sich womöglich verändert. Lassen Sie ihn von seinem System prüfen und versuchen Sie es dann erneut.
failure-shrink-refused-untouched-do = Es wurde nichts verändert. Öffnen Sie den Installer erneut und lassen Sie dem Bereich mehr Platz, oder verkleinern Sie ihn in seinem eigenen System.
failure-windows-hibernated = Windows ist nicht ganz heruntergefahren
failure-windows-hibernated-do = Es wurde nichts verändert. Starten Sie Windows, schalten Sie den Schnellstart aus, fahren Sie es herunter und versuchen Sie es dann erneut.
failure-needs-check = Ein Bereich des Laufwerks muss zuerst geprüft werden
failure-needs-check-do = Alle Bereiche des Laufwerks sind noch da, aber der zu verkleinernde hat sich womöglich verändert. Starten Sie sein System, lassen Sie es sein Laufwerk prüfen und versuchen Sie es dann erneut.
failure-needs-check-untouched-do = Es wurde nichts verändert. Starten Sie das System, zu dem der Bereich gehört, lassen Sie es sein Laufwerk prüfen und versuchen Sie es dann erneut.
failure-windows-in-use = Ein Bereich eines Laufwerks mit Windows wird verwendet
failure-windows-in-use-do = Es wurde nichts verändert. Schließen Sie, was Dateien von Windows geöffnet hat, oder starten Sie den Rechner neu, und versuchen Sie es dann erneut.
failure-windows-needs-check = Windows muss zuerst sein Laufwerk prüfen
failure-windows-needs-check-do = Es wurde nichts verändert. Starten Sie Windows, lassen Sie es sein Laufwerk prüfen, fahren Sie es herunter und versuchen Sie es dann erneut.
failure-windows-unread = Ein Bereich eines Laufwerks mit Windows ließ sich nicht lesen
failure-windows-unread-do = Es wurde nichts verändert. Trennen Sie jedes Laufwerk, auf dem Sie nicht installieren, und versuchen Sie es erneut, oder verkleinern Sie Windows in Windows selbst.
failure-bitlocker = Ein Bereich eines Laufwerks ist mit BitLocker gesperrt
failure-bitlocker-do = Es wurde nichts verändert. Schalten Sie BitLocker in Windows aus und versuchen Sie es erneut, oder verkleinern Sie Windows in Windows selbst.
failure-needs-internet = Diese Wahl braucht Internet
failure-needs-internet-do = Verbinden Sie sich mit dem Internet, oder wählen Sie den Linexin-Desktop, der keines braucht.
failure-needs-internet-untouched-do = Es wurde nichts gelöscht. Verbinden Sie sich mit dem Internet, oder wählen Sie den Linexin-Desktop, der keines braucht.
failure-command = Beim Installieren ist etwas schiefgegangen
failure-command-do = Sehen Sie sich die Details an und versuchen Sie es dann erneut. Das Laufwerk muss womöglich gelöscht werden.
failure-internal = Der Installer ist auf ein Problem gestoßen
failure-internal-do = Schließen Sie ihn und versuchen Sie es erneut. Passiert es wieder, bewahren Sie die Details für einen Fehlerbericht auf.

## Why a choice cannot be installed, said beside it

invalid-username-empty = Wählen Sie einen Benutzernamen
invalid-username-shape = Kleinbuchstaben, Ziffern, - und _, zuerst ein Buchstabe
invalid-username-reserved = Dieser Name gehört dem System
invalid-full-name = Ein Name darf kein : oder , enthalten
invalid-hostname = Buchstaben, Ziffern und -, höchstens 63 Zeichen
invalid-password-empty = Wählen Sie ein Passwort
invalid-password-mismatch = Die beiden Passwörter sind nicht gleich
invalid-password-shape = Höchstens 128 Zeichen, ohne Zeilenumbrüche
invalid-root-password-empty = Wählen Sie ein Passwort für das Administratorkonto
invalid-locale = Wählen Sie eine Sprache
invalid-timezone = Wählen Sie eine Zeitzone
invalid-keyboard = Wählen Sie eine Tastatur
invalid-disk-blocked = Dieses Laufwerk ist nicht verwendbar
invalid-target-missing = Wählen Sie, wohin installiert wird
invalid-target-small = Dort ist nicht genug Platz für Linexin
table-full = Dieses Laufwerk hat keinen Platz für einen weiteren Bereich. Ersetzen Sie einen.
invalid-target-esp = Dieser Bereich startet den Rechner
target-protected = Diesen Bereich behält Windows für sich
invalid-home = Dieser Bereich kann Ihre Dateien nicht aufnehmen
invalid-swap = Der Auslagerungsspeicher ist größer als der Platz dort
invalid-bootloader = Dieser Startmanager lässt sich auf diesem Rechner nicht verwenden
invalid-source = Die Linexin-Kopie zum Installieren wurde nicht gefunden
invalid-layout = Ihre Partitionen lassen sich so nicht verwenden

## The time zone step

timezone-heading-region = Weltregion
timezone-heading-city = Nächste Stadt
timezone-heading-found = Gefundene Orte
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] Ein Ort
   *[other] { $count } Orte
}
zone-region-africa = Afrika
zone-region-america = Amerika
zone-region-antarctica = Antarktis
zone-region-arctic = Arktis
zone-region-asia = Asien
zone-region-atlantic = Atlantik
zone-region-australia = Australien
zone-region-europe = Europa
zone-region-indian = Indischer Ozean
zone-region-pacific = Pazifik
zone-region-utc = Weltzeit
# The one place under Universal time: the clock no country keeps.
zone-utc = Koordinierte Weltzeit

## The keyboard step

keyboard-heading-layout = Belegung
keyboard-heading-variant = Variante
keyboard-heading-found = Gefundene Varianten
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] Nur Standard
    [one] Standard und eine weitere
   *[other] Standard und { $count } weitere
}
# Under the plain layout, first among its kinds.
keyboard-plain = Standard
keyboard-test = Tastatur ausprobieren
keyboard-test-placeholder = Tippen Sie hier zum Ausprobieren

## The search over a long list (Language, Time zone, Keyboard)

search-field = Suche
search-language-placeholder = Eine Sprache, auch auf Englisch
search-timezone-placeholder = Eine Stadt, ein Land oder ein Erdteil
search-keyboard-placeholder = Eine Belegung oder eine Variante
search-nothing = Nichts gefunden für „{ $query }“

## The step of where to install

button-options = Optionen
target-heading-disks = Laufwerke
target-erase = Dieses Laufwerk löschen
target-erase-note = Alles darauf wird gelöscht
target-free = Freier Platz · { $size }
target-free-note = Auf dem Laufwerk wird nichts gelöscht
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = { $what } ersetzen · { $size }
target-replace-note = Was darauf ist, wird gelöscht
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = „{ $label }“ ersetzen · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · wird gelöscht
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = „{ $label }“ · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = Freier Platz
target-no-disk = Es gibt kein Laufwerk, auf dem Linexin installiert werden kann.
target-options = Optionen
# Each option with the state it is in.
target-option-storage = Speicher · { $value }
target-option-swap = Auslagerungsspeicher · { $value }
target-option-home = Ihre Dateien · { $value }
target-storage = Speicher
target-storage-standard = Standard
target-storage-snapshots = Mit Wiederherstellungspunkten
target-storage-snapshots-short = Wiederherstellungspunkte
target-swap = Auslagerungsspeicher
target-swap-off = Aus
target-home = Ihre Dateien
target-home-system = Beim System
target-home-kept = { $part }, behalten
target-home-erased = { $part }, gelöscht
target-home-keep = Inhalt behalten
target-home-erase = Inhalt löschen
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = Ihre Partitionen werden verwendet · { $count ->
    [one] ein Vorgang
   *[other] { $count } Vorgänge
}
target-partitions-let-go = Ein Laufwerk hat sich geändert, daher wurden Ihre Partitionen verworfen. Wählen Sie erneut.

## Partitionen

button-advanced = Erweitert
partitions-title = Partitionen
partitions-not-yet = Partitionen können in dieser Version noch nicht angewendet werden.
partitions-not-yet-fresh-table = Eine neue Partitionstabelle kann in dieser Version noch nicht angewendet werden.
partitions-not-yet-keep-table = Partitionen erstellen oder löschen kann in dieser Version noch nicht angewendet werden.
partitions-not-yet-change-entries = Eine Partition formatieren kann in dieser Version noch nicht angewendet werden.
partitions-not-yet-kept-home = Ein erhaltenes /home kann in dieser Version noch nicht angewendet werden.
partitions-not-yet-several-disks = Ein Plan über mehrere Datenträger kann in dieser Version noch nicht angewendet werden.
partitions-not-yet-bios = Partitionen auf einem BIOS-Computer können in dieser Version noch nicht angewendet werden.
partitions-not-yet-shrink = Eine Partition verkleinern kann in dieser Version noch nicht angewendet werden.
partitions-unallocated = nicht zugeteilt
partitions-unknown = unbekannt
partitions-no-table = keine Partitionstabelle
partitions-partition = Partition
partitions-filesystem = Dateisystem
partitions-label = Bezeichnung
partitions-size = Größe
partitions-used = Belegt
partitions-mount-point = Einhängepunkt
partitions-flags = Markierungen
partitions-pending = Ausstehend
partitions-start = Anfang
partitions-end = Ende
partitions-reading = Wird gelesen…
partitions-menu-mount = Einhängepunkt · { $value }
partitions-menu-format = Formatieren als · { $value }
partitions-format-as = Formatieren als
partitions-none = keiner
partitions-keep = behalten
partitions-delete = Löschen
partitions-undo-part = Änderungen an { $partition } zurücknehmen
partitions-undo = Änderungen zurücknehmen
partitions-create-table = Partitionstabelle erstellen
partitions-create-table-kind = { $table }-Partitionstabelle erstellen
partitions-table = Partitionstabelle
partitions-undo-last = Letzten Vorgang zurücknehmen
partitions-clear = Alle Vorgänge verwerfen
partitions-pending-list = Ausstehende Vorgänge ({ $count })…
partitions-swap-file = Auslagerungsdatei · { $value }
partitions-swap-file-title = Auslagerungsdatei
partitions-pending-title = Ausstehende Vorgänge
partitions-pending-short = Ausstehend
partitions-pending-none = Keine ausstehenden Vorgänge
partitions-pending-earlier = { $count ->
    [one] Ein früherer Vorgang
   *[other] { $count } frühere Vorgänge
}
partitions-tile-format = formatieren
partitions-tile-deleted = { $count ->
   *[other] { $partitions } gelöscht
}
partitions-do-format = Formatieren als { $filesystem }
partitions-do-mount = Einhängen in { $path }
partitions-do-delete = { $partition } löschen
partitions-op-delete = { $partition } löschen ({ $filesystem }, { $size })
partitions-op-format = { $partition } als { $filesystem } formatieren
partitions-op-mount = Einhängepunkt von { $partition } auf { $path } setzen
partitions-op-table = Partitionstabelle { $table } auf { $disk } erstellen
partitions-op-create = Primäre Partition #{ $number } ({ $filesystem }, { $size }) auf { $disk } erstellen
partitions-op-create-at = Primäre Partition #{ $number } ({ $filesystem }, { $size }, { $path }) auf { $disk } erstellen
partitions-op-shrink = { $partition } von { $from } auf { $to } verkleinern
partitions-op-grow = { $partition } von { $from } auf { $to } vergrößern
partitions-op-label = Dateisystembezeichnung „{ $label }“ für { $partition } setzen
partitions-op-label-clear = Dateisystembezeichnung von { $partition } entfernen
partitions-missing-root = Es fehlt eine Partition, eingehängt in /
partitions-missing-efi = Es fehlt eine EFI-Partition in /boot oder /efi
partitions-missing-boot = /boot hinzufügen oder EFI-Partition in /boot einhängen
partitions-missing-bios-grub = Es fehlt eine bios_grub-Partition auf { $disk }
partitions-too-small = { $path } braucht mindestens { $size }
partitions-must-format = Formatieren Sie { $partition } für { $path }
partitions-needs = { $path } braucht { $filesystem }
partitions-needs-linux = { $path } braucht ext4, btrfs oder xfs
partitions-needs-root = { $path } braucht ext4 oder btrfs
partitions-twice = { $path } ist mehr als einer Partition zugewiesen
partitions-blocked = Auf { $disk } kann nichts geändert werden
partitions-refused-mismatch = Der Plan für { $disk } passt nicht zum Datenträger
partitions-refused-unaligned = { $partition } muss auf ganzen MiB beginnen und enden
partitions-refused-outside = { $partition } ragt über den Datenträger hinaus
partitions-refused-overlap = { $partition } überlappt eine andere Partition
partitions-refused-beside-extended = { $disk }: nichts neben der erweiterten Partition
partitions-refused-logical = Erst die logischen Partitionen nach { $partition } löschen
partitions-refused-entries = { $disk } hätte mehr als 128 Partitionen
partitions-refused-shrink = { $partition } kann nicht verkleinert werden
partitions-refused-grow = { $partition } bleibt erhalten und kann nicht wachsen
partitions-refused-bios-grub = { $partition } kann hier keine bios_grub-Partition sein
partitions-refused-label = { $partition }: ungültige Bezeichnung
partitions-refused-unmountable = { $partition }: { $filesystem } nicht einhängbar
partitions-refused-inside-kept = { $path } bleibt erhalten: darin ist nichts einhängbar
partitions-refused-fs-small = { $partition } ist zu klein für { $filesystem }
partitions-refused-restore-points = Wiederherstellungspunkte gehen nicht neben eigenem /var
partitions-refused-boot-disk = /boot muss auf demselben Datenträger wie { $path } liegen
partitions-refused-efi-bios = /efi gibt es nur, wenn UEFI den Computer startet
partitions-refused-grub-gap = GRUB braucht das erste MiB von { $disk } frei
partitions-refused-bootloader = { $loader } kann Linexin von hier nicht starten
partitions-refused-not-read = Die Belegung von { $partition } ist noch nicht bekannt
partitions-refused-shrink-below = { $partition } kann nicht kleiner als { $size } werden
partitions-refused-hibernated = { $partition } lässt sich nicht verkleinern: Windows ruht
partitions-refused-needs-check = { $partition }: erst prüfen, dann verkleinern
partitions-refused-encrypted = { $partition } ist verschlüsselt und nicht verkleinerbar
partitions-refused-in-use = { $partition } wird verwendet und ist nicht verkleinerbar
partitions-refused-unreadable = { $partition } ist nicht lesbar und nicht verkleinerbar
partitions-refused-no-hiberfile = { $partition }: in Windows verkleinern, nicht hier
partitions-refused-windows-unread = Nicht verkleinerbar: Windows auf { $other } unlesbar
partitions-refused-bitlocker = Nicht verkleinerbar: { $other } hat BitLocker
partitions-why-hibernated = Windows auf { $other }: ohne Schnellstart herunterfahren
partitions-why-windows-unread = Verkleinern erst, wenn Windows auf { $other } lesbar ist
partitions-why-bitlocker = Zum Verkleinern BitLocker auf { $other } abschalten
partitions-why-no-hiberfile = Mit der Datenträgerverwaltung von Windows verkleinern
partitions-refused-esp-full = { $partition } braucht { $size } freien Platz
partitions-refused-image-unknown = Wie viel Linexin in { $path } ablegt, ist noch unbekannt
partitions-refused-unread = Die Belegung von { $partition } ließ sich nicht lesen
partitions-refused-image-unread = Wie viel Linexin in { $path } ablegt, ließ sich nicht lesen
partitions-new = Neu…
partitions-resize = Größe ändern…
partitions-relabel = Bezeichnung…
partitions-other = Anderer…
partitions-new-name = Neue Partition #{ $number }
partitions-tile-new = neu
partitions-new-title = Neue Partition
partitions-resize-title = Größe von { $partition } ändern
partitions-label-title = Bezeichnung von { $partition }
partitions-other-title = Einhängepunkt von { $partition }
partitions-new-size = Neue Größe
partitions-position = Lage
partitions-position-start = Anfang des Bereichs
partitions-position-end = Ende des Bereichs
partitions-bios-grub = bios_grub (unformatiert)
partitions-cancel = Abbrechen
partitions-add = Hinzufügen
partitions-resize-button = Größe ändern
partitions-set = Festlegen
partitions-leaves = Danach bleiben { $size } nicht zugeteilt
partitions-leaves-none = Danach bleibt nichts nicht zugeteilt
partitions-takes = Nimmt { $size } des nicht zugeteilten Bereichs dahinter
partitions-takes-all = Nimmt den ganzen nicht zugeteilten Bereich dahinter
partitions-at-least = Braucht mindestens { $size }
partitions-path-placeholder = Zum Beispiel /data
partitions-path-whole = Ein Einhängepunkt ist ein ganzer Pfad ab /, etwa /data
partitions-path-twice = { $path } ist schon ein Einhängepunkt
partitions-path-system = { $path } gehört zum System
partitions-path-long = Ein Einhängepunkt hat höchstens 255 Bytes
partitions-path-spaces = Ein Einhängepunkt enthält keine Leerzeichen oder Backslashes
partitions-path-under = Innerhalb von { $path } kann nichts eingehängt werden
partitions-label-bytes = { $filesystem }-Bezeichnungen fassen höchstens { $count ->
    [one] ein Byte
   *[other] { $count } Bytes
}
partitions-label-chars = { $filesystem }-Bezeichnungen fassen höchstens { $count ->
    [one] ein Zeichen
   *[other] { $count } Zeichen
}
partitions-label-slash = Eine Bezeichnung kann kein „/“ enthalten
partitions-why-table = Noch keine Partitionstabelle: diesen Bereich wählen, um eine zu erstellen
partitions-why-primaries = Ein msdos-Datenträger fasst höchstens vier primäre Partitionen
partitions-why-logical = Logische Partitionen können hier nicht erstellt werden
partitions-why-beside-extended = Neben einer erweiterten Partition entsteht nichts
partitions-why-whole = Etwas füllt den ganzen Datenträger: keine Tabelle oder Partition möglich
partitions-why-unmountable = { $filesystem } ist nicht einhängbar: erst formatieren
partitions-why-unformatted = Kein Dateisystem: zum Verwenden formatieren
partitions-why-logical-delete = Erst die logischen Partitionen mit höherer Nummer löschen
partitions-primaries = { $disk } hätte mehr als vier primäre Partitionen
partitions-do-create = Erstellen
partitions-do-shrink = Verkleinern auf { $size }
partitions-do-grow = Vergrößern auf { $size }

## The desktop step

button-done = Fertig
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = Basiert auf GNOME
desktop-about-kinexin = Basiert auf KDE Plasma
desktop-about-linexinbar = Für den Controller gemacht
# Under a desktop: whether it has to be downloaded.
desktop-ready = Kein Internet nötig
desktop-needs-internet = Braucht Internet
desktop-refused = { $desktop } braucht Internet. Verbinden Sie sich, oder wählen Sie Linexin.
desktop-checking = Internetverbindung wird geprüft…
desktop-checking-short = Internet wird geprüft…
desktop-offline = Es gibt kein Internet, daher wird ausgelassen, was es braucht.
desktop-updates = Aktualisierungen installieren
desktop-updates-on = Alles auf den neuesten Stand
desktop-updates-off = So wie auf dem USB-Stick
updates-refused = Aktualisierungen brauchen Internet. Sie lassen sich später installieren.
desktop-options-title = Optionen
desktop-options-caption = Was noch installiert wird und wie der Rechner startet.
desktop-option-remove = Software entfernen
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = Software
desktop-flatpaks-title = Apps
desktop-bootloader-title = Start
desktop-remove-caption = Schalten Sie aus, was Sie auf dem installierten System nicht möchten.
desktop-remove-looking = Software wird geprüft…
desktop-remove-unlisted = Die Software konnte nicht aufgelistet werden
desktop-remove-none = Es gibt nichts zu entfernen
desktop-remove-nothing = Nichts wird entfernt
desktop-remove-count = { $count ->
    [one] Eines ausgelassen
   *[other] { $count } ausgelassen
}
desktop-option-flatpaks = Apps von Flathub
desktop-flatpaks-caption = Werden zusammen mit Linexin aus dem Internet installiert.
# How many of the apps offered are switched on.
desktop-flatpaks-count = { $count } von { $total } gewählt
desktop-flatpak-on = Wird mit Linexin installiert
desktop-flatpak-off = Ausgelassen
flatpaks-refused = Apps von Flathub brauchen Internet. Sie lassen sich später hinzufügen.
desktop-option-bootloader = Startmanager
desktop-bootloader-caption = Was Linexin startet, wenn der Rechner eingeschaltet wird.
desktop-option-drivers = Ungenutzte Treiber entfernen
desktop-drivers-on = Ungenutzte werden entfernt
desktop-drivers-off = Alle Treiber bleiben
desktop-option-snapshots = Wiederherstellungspunkte
desktop-snapshots-on = Vor jeder Aktualisierung gesichert
desktop-snapshots-off = Es werden keine gesichert
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = Unter „Wohin installieren“ einrichten
desktop-snapshots-var = Nicht neben eigenem /var
bootloader-automatic = Automatisch
bootloader-automatic-note = Passend zu diesem Rechner gewählt
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = Startet Linexin sofort, ohne Menü
bootloader-grub = GRUB
bootloader-grub-note = Ein Menü beim Start, auf jedem Rechner
bootloader-refind = rEFInd
bootloader-refind-note = Ein Menü mit allen Systemen auf dem Rechner
bootloader-unavailable = Auf diesem Rechner nicht verwendbar
# The groups the software to remove is listed in.
package-group-applications = Apps
package-group-multimedia = Ton und Video
package-group-network = Netzwerk
package-group-fonts = Schriften
package-group-libraries = Bibliotheken
