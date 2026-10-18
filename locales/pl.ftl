# Linexin Installer — Polish.
#
# Every message of en-GB.ftl, in Polish. The comments are en-GB's: they say
# where a message appears and what it means. The words the family already
# uses — Wstecz, Kontynuuj, Opcje, Zamknij, Wybierz, Gotowe, Uruchom ponownie
# teraz, komputer, dysk — are the shell's and the toolkit's, and so is the
# familiar "ty", with Twoje and Tobie capitalised as the shell writes them.
# A count takes Polish's own forms: one, few, many, and other behind them.
# Where a value of unknown gender would have to agree with a verb, the
# sentence is turned (with a colon, or "do wymazania") so that it never has
# to. A name a person or a disk gave stands in Polish quotes, „ ”.

## The application itself

app-name = Instalator Linexin

## The head of each screen: its title, and the one line under it

welcome-caption = Kilka pytań i Linexin będzie na tym komputerze.
language-title = Język
language-caption = Język, którym mówi ten komputer.
timezone-title = Strefa czasowa
timezone-caption = Gdzie jesteś, aby zegar pokazywał dobrą godzinę.
keyboard-title = Klawiatura
keyboard-caption = Klawisze, na których piszesz.
target-title = Gdzie zainstalować
target-caption = Dysk, na którym znajdzie się Linexin.
desktop-title = Pulpit
desktop-caption = Jak wygląda Linexin. Każdy można później dostosować.
account-title = O Tobie
account-caption = Twoje imię i konto, którym się logujesz.
review-title = Linexin jest gotowy do instalacji
review-caption = Sprawdź wszystko. Nic jeszcze nie zostało zmienione.
installing-title = Instalowanie Linexin
installing-caption = To chwilę potrwa. Nie trzeba nic naciskać.
done-title = Linexin jest zainstalowany
done-caption = Uruchom ponownie komputer i wyjmij pendrive, aby korzystać z Linexin.
stopped-title = Instalacja została przerwana

## The verbs at the foot of a screen

button-continue = Kontynuuj
button-install = Zainstaluj
button-close = Zamknij
button-restart = Uruchom ponownie teraz
button-not-now = Nie teraz
button-restart-short = Uruchom ponownie

## What the buttons do, in the legend at the foot of the panel

hint-choose = Wybierz
hint-back = Wstecz
hint-quit = Wyjdź
hint-type = Pisz
hint-shift = Shift
hint-close = Zamknij
hint-done = Gotowe
hint-options = Opcje
hint-details = Szczegóły
hint-hide-details = Ukryj szczegóły
hint-search = Szukaj
hint-adjust = Dostosuj
hint-keep = Zatwierdź
hint-put-back = Przywróć
hint-cancel = Anuluj

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = ąę

## Questions, asked with the safe answer first

quit-title = Wyjść z instalatora?
quit-body = Na tym komputerze nic nie zostało zmienione.
quit-stay = Zostań
quit-leave = Wyjdź
erase-title = Wymazać i zainstalować?
erase-title-short = Wymazać?
install-title = Zainstalować Linexin?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = Wszystko na { $disk } zostanie wymazane.
erase-target-part = Zostanie wymazane: { $part } · { $size } na { $disk }.
erase-target-free = Linexin zostanie zainstalowany w wolnym miejscu na { $disk }.
erase-home = Zostanie wymazane miejsce na Twoje pliki: { $part }.
erase-final = Tego nie można cofnąć.
erase-nothing = Nic na nim nie zostanie wymazane.
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = Te operacje zmienią zawartość Twoich dysków:
erase-plan-nothing = Nic na Twoich dyskach nie zostanie wymazane.
erase-plan-erased = Wymazuje { $partition } ({ $filesystem }, { $size })
erase-plan-esp = { $what } · partycja systemowa EFI
erase-plan-windows = { $what } · Windows
erase-plan-page = Strona { $page } z { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = „{ $label }”
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $part } na { $disk }
erase-back = Wróć
erase-confirm = Wymaż i zainstaluj
erase-confirm-short = Wymaż
button-next = Dalej

## A line beside the legend

demo-note = Zmyślony komputer: nic tu nie jest prawdziwe i nic nie zostanie zmienione.
install-unavailable = Uruchom ten komputer z pendrive’a z Linexin, aby zainstalować na nim Linexin.
disks-unreadable = Nie udało się odczytać dysków tego komputera.
install-reading = Dyski tego komputera są jeszcze odczytywane. Spróbuj ponownie za chwilę.
installing-note = Nie wyłączaj komputera, dopóki to się nie skończy.
done-all-well = Wszystko poszło zgodnie z planem.
done-warnings = { $count ->
    [one] Linexin jest gotowy, ale jedna rzecz nie poszła zgodnie z planem:
    [few] Linexin jest gotowy, ale kilka rzeczy nie poszło zgodnie z planem:
    [many] Linexin jest gotowy, ale kilka rzeczy nie poszło zgodnie z planem:
   *[other] Linexin jest gotowy, ale kilka rzeczy nie poszło zgodnie z planem:
}
done-restart-failed = Nie udało się ponownie uruchomić komputera. Uruchom go ponownie z menu systemu.

## The fields

field-full-name = Imię i nazwisko
field-full-name-placeholder = Tak, jak zobaczą Cię inni
field-username = Nazwa użytkownika
field-username-placeholder = Używana do logowania
field-password = Hasło
field-password-placeholder = Ustaw hasło
field-repeat = Potwierdź hasło
field-repeat-placeholder = To samo hasło jeszcze raz
field-hostname = Nazwa komputera
field-hostname-placeholder = Nazwa tego komputera w sieci
field-root-password = Hasło administratora
field-root-password-placeholder = Hasło do zmian w systemie
field-root-repeat = Potwierdź hasło administratora
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = Słabe
strength-fair = Średnie
strength-good = Dobre
strength-strong = Silne

## You

# A separate administrator (root) account, off unless asked for.
account-root = Konto administratora
account-root-off = Wyłączone: zmiany zatwierdza Twoje hasło
account-root-on = Z własnym hasłem, poniżej

## Sizes, as a box writes them

size-tb = { $amount } TB
size-gb = { $amount } GB
size-mb = { $amount } MB
size-kb = { $amount } kB
size-bytes = { $amount } B

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = Szybki dysk SSD
disk-kind-sata = Dysk wewnętrzny
disk-kind-usb = Dysk USB
disk-kind-card = Karta pamięci
disk-kind-virtual = Dysk wirtualny
disk-kind-other = Dysk
disk-empty = Pusty
disk-holds-more = { $first } i { $count ->
    [one] jeszcze jedna część
    [few] jeszcze { $count } części
    [many] jeszcze { $count } części
   *[other] jeszcze { $count } części
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what } na całym dysku

## What a part of a disk appears to hold

content-efi = Pliki startowe
content-bios-boot = Obszar startowy
content-reserved = Zarezerwowane przez Windows
content-recovery = Odzyskiwanie Windows
content-windows = Windows
content-linux = Linux
content-swap = Dodatkowa pamięć
content-data = Pliki
content-empty = Pusta
content-other = Coś innego

## Why a disk cannot be chosen, said under it

blocked-installation-media = Z niego działa instalator
blocked-in-use = Teraz w użyciu
blocked-swap = Używany jako dodatkowa pamięć
blocked-holder = Używany przez coś innego na tym komputerze
blocked-read-only = Tylko do odczytu
blocked-too-small = Za mały na Linexin
blocked-unknown = Nie można go użyć

## The review

# Each row names a step; under it, that step's answer.
review-where = Gdzie
review-desktop = Pulpit
review-region-name = Język i region
review-account-name = Konto
review-extras = Dodatki
review-where-disk = { $disk } · wszystko zostanie wymazane
review-where-free = { $disk } · w wolnym miejscu
review-where-part = { $disk } · zamiast: { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = Twoje pliki na { $part }, bez wymazywania
review-where-home-erased = Twoje pliki na { $part }, po wymazaniu
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = Jeszcze nie wybrano
review-extra-updates = Aktualizacje
review-extra-apps = { $count ->
    [one] Jedna aplikacja
    [few] { $count } aplikacje
    [many] { $count } aplikacji
   *[other] { $count } aplikacji
}
review-extra-snapshots = Punkty przywracania
review-extra-swap = { $size } dodatkowej pamięci
review-extra-none = Nic więcej
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size } na { $disk }
review-part-formatted = { $partition } · { $filesystem } · { $size } · sformatowana
review-part-kept = { $partition } · { $filesystem } · { $size } · zachowana
review-operations = { $count ->
    [one] Zostanie zastosowana jedna operacja
    [few] Zostaną zastosowane { $count } operacje
    [many] Zostanie zastosowanych { $count } operacji
   *[other] Zostanie zastosowanych { $count } operacji
}

## The installation, while it runs

installing-waiting = Przygotowywanie…
installing-count = Krok { $step } z { $steps } · { $percent }%
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = Przerwano na etapie: { $step }
step-check = Sprawdzanie komputera
step-resize = Zmniejszanie tego, co zostaje na dysku
step-partition = Robienie miejsca na dysku
step-format = Przygotowywanie dysku
step-copy = Kopiowanie Linexin
step-swap = Przygotowywanie dodatkowej pamięci
step-system = Konfigurowanie systemu
step-region = Ustawianie języka, strefy czasowej i klawiatury
step-accounts = Tworzenie Twojego konta
step-desktop = Konfigurowanie pulpitu
step-updates = Instalowanie aktualizacji
step-packages = Dodawanie i usuwanie oprogramowania
step-flatpaks = Instalowanie aplikacji
step-drivers = Usuwanie sterowników, których ten komputer nie potrzebuje
step-snapshots = Konfigurowanie punktów przywracania
step-initramfs = Przygotowywanie systemu do uruchomienia
step-bootloader = Ustawianie komputera, by uruchamiał Linexin
step-finish = Kończenie

## What did not work, said once Linexin is installed

warning-updates-offline = Aktualizacje pominięto, bo nie było internetu. Można je zainstalować później.
warning-updates-failed = Nie udało się zainstalować aktualizacji. Można je zainstalować później.
warning-flatpaks-failed = Nie udało się zainstalować niektórych aplikacji. Można je dodać później.
warning-packages-kept = Część oprogramowania, które miało zostać usunięte, nadal jest w systemie.
warning-boot-entry = Jeśli Linexin się nie uruchamia, wybierz go w menu startowym komputera.
warning-snapshots = Nie udało się skonfigurować punktów przywracania.
warning-drivers-kept = Sterowniki, których ten komputer nie potrzebuje, zostały na miejscu.
warning-windows-check = Pozwól Windows sprawdzić dysk przy następnym uruchomieniu. Do tego czasu Linexin go nie rusza.

## Why the installation stopped, and what can be done about it

failure-invalid-request = Instalator poprosił o coś, czego nie potrafi zrobić
failure-invalid-request-do = Zamknij instalator, otwórz go ponownie i jeszcze raz przejdź przez wszystkie kroki.
failure-consent-mismatch = Zmieniło się to, co miało zostać wymazane
failure-consent-mismatch-do = Nic nie zostało wymazane. Przejdź przez kroki ponownie i sprawdź dysk.
failure-not-root = Instalator nie dostał pozwolenia
failure-not-root-do = Otwórz instalator ponownie i zezwól na to, gdy pojawi się pytanie.
failure-not-live = Ten komputer nie został uruchomiony z instalatora
failure-not-live-do = Uruchom komputer z pendrive’a z Linexin i spróbuj ponownie.
failure-secure-boot = Secure Boot jest włączony
failure-secure-boot-do = Wyłącz Secure Boot w ustawieniach startowych komputera i spróbuj ponownie.
failure-already-running = Trwa już inna instalacja
failure-already-running-do = Poczekaj, aż się skończy, albo uruchom ponownie komputer i spróbuj jeszcze raz.
failure-missing-tool = Brakuje czegoś, czego potrzebuje instalator
failure-missing-tool-do = Użyj nowszego pendrive’a z Linexin i spróbuj ponownie.
failure-image-invalid = Kopia Linexin na pendrivie jest uszkodzona
failure-image-invalid-do = Nagraj Linexin na pendrive od nowa i spróbuj jeszcze raz.
failure-disk-changed = Dysk zmienił się po tym, jak został wybrany
failure-disk-changed-do = Część dysku mogła już zostać wymazana. Przejdź przez kroki ponownie i jeszcze raz wybierz dysk.
failure-disk-changed-untouched-do = Nic nie zostało wymazane. Przejdź przez kroki ponownie i jeszcze raz wybierz dysk.
failure-disk-busy = Dysk jest używany przez coś innego
failure-disk-busy-do = Zamknij to, co go używa, albo uruchom ponownie komputer i spróbuj jeszcze raz.
failure-disk-busy-untouched-do = Nic nie zostało wymazane. Zamknij to, co używa dysku, albo uruchom ponownie komputer i spróbuj jeszcze raz.
failure-no-space = Brakuje miejsca
failure-no-space-do = Wybierz większy dysk lub więcej wolnego miejsca i spróbuj ponownie.
failure-esp-full = Obszar startowy dysku jest pełny
failure-esp-full-do = Zamiast tego wymaż cały dysk albo najpierw zrób miejsce w jego obszarze startowym.
failure-shrink-refused = Nie udało się zmniejszyć części dysku
failure-shrink-refused-do = Wszystkie części dysku wciąż są na miejscu, ale zmniejszana część mogła się zmienić. Pozwól systemowi, do którego należy, ją sprawdzić, a potem spróbuj ponownie.
failure-shrink-refused-untouched-do = Nic nie zostało zmienione. Otwórz instalator ponownie i zostaw tej części więcej miejsca albo zmniejsz ją w systemie, do którego należy.
failure-windows-hibernated = Windows nie jest całkiem wyłączony
failure-windows-hibernated-do = Nic nie zostało zmienione. Uruchom Windows, wyłącz szybkie uruchamianie, zamknij system i spróbuj ponownie.
failure-needs-check = Część dysku trzeba najpierw sprawdzić
failure-needs-check-do = Wszystkie części dysku wciąż są na miejscu, ale zmniejszana część mogła się zmienić. Uruchom system, do którego należy, pozwól mu sprawdzić dysk i spróbuj ponownie.
failure-needs-check-untouched-do = Nic nie zostało zmienione. Uruchom system, do którego należy ta część, pozwól mu sprawdzić dysk i spróbuj ponownie.
failure-windows-in-use = Część dysku z Windows jest używana
failure-windows-in-use-do = Nic nie zostało zmienione. Zamknij to, co ma otwarte pliki Windows, albo uruchom ponownie komputer i spróbuj jeszcze raz.
failure-windows-needs-check = Windows musi najpierw sprawdzić dysk
failure-windows-needs-check-do = Nic nie zostało zmienione. Uruchom Windows, pozwól mu sprawdzić dysk, zamknij system i spróbuj ponownie.
failure-windows-unread = Nie udało się odczytać części dysku z Windows
failure-windows-unread-do = Nic nie zostało zmienione. Odłącz dyski, na których nie instalujesz, i spróbuj ponownie albo zmniejsz Windows w samym Windows.
failure-bitlocker = Część dysku jest zablokowana przez BitLockera
failure-bitlocker-do = Nic nie zostało zmienione. Wyłącz BitLockera w Windows i spróbuj ponownie albo zmniejsz Windows w samym Windows.
failure-needs-internet = Ten wybór wymaga internetu
failure-needs-internet-do = Połącz się z internetem albo wybierz pulpit Linexin, który go nie wymaga.
failure-needs-internet-untouched-do = Nic nie zostało wymazane. Połącz się z internetem albo wybierz pulpit Linexin, który go nie wymaga.
failure-command = Coś poszło nie tak podczas instalacji
failure-command-do = Sprawdź szczegóły i spróbuj ponownie. Może być konieczne wymazanie dysku.
failure-internal = Instalator napotkał problem
failure-internal-do = Zamknij go i spróbuj ponownie. Jeśli to się powtórzy, zachowaj szczegóły do zgłoszenia błędu.

## Why a choice cannot be installed, said beside it

invalid-username-empty = Podaj nazwę użytkownika
invalid-username-shape = Małe litery, cyfry, - i _, na początku litera
invalid-username-reserved = Ta nazwa należy do systemu
invalid-full-name = Imię nie może zawierać : ani ,
invalid-hostname = Litery, cyfry i -, najwyżej 63 znaki
invalid-password-empty = Ustaw hasło
invalid-password-mismatch = Hasła nie są takie same
invalid-password-shape = Najwyżej 128 znaków, bez podziału na wiersze
invalid-root-password-empty = Ustaw hasło dla konta administratora
invalid-locale = Wybierz język
invalid-timezone = Wybierz strefę czasową
invalid-keyboard = Wybierz klawiaturę
invalid-disk-blocked = Tego dysku nie można użyć
invalid-target-missing = Wybierz, gdzie zainstalować
invalid-target-small = Nie ma tam dość miejsca na Linexin
table-full = Na tym dysku nie ma miejsca na kolejną część. Zastąp którąś z obecnych.
invalid-target-esp = Ta część uruchamia komputer
target-protected = Windows zachowuje tę część dla siebie
invalid-home = Ta część nie może przechowywać Twoich plików
invalid-swap = Dodatkowa pamięć jest większa niż miejsce, które tam jest
invalid-bootloader = Tego menedżera startu nie można użyć na tym komputerze
invalid-source = Nie można znaleźć kopii Linexin do zainstalowania
invalid-layout = Twoich partycji nie da się użyć tak, jak je zaplanowano

## The time zone step

timezone-heading-region = Część świata
timezone-heading-city = Najbliższe miasto
timezone-heading-found = Znalezione miejsca
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] Jedno miejsce
    [few] { $count } miejsca
    [many] { $count } miejsc
   *[other] { $count } miejsca
}
zone-region-africa = Afryka
zone-region-america = Ameryki
zone-region-antarctica = Antarktyka
zone-region-arctic = Arktyka
zone-region-asia = Azja
zone-region-atlantic = Ocean Atlantycki
zone-region-australia = Australia
zone-region-europe = Europa
zone-region-indian = Ocean Indyjski
zone-region-pacific = Ocean Spokojny
zone-region-utc = Czas uniwersalny
# The one place under Universal time: the clock no country keeps.
zone-utc = Uniwersalny czas koordynowany

## The keyboard step

keyboard-heading-layout = Układ
keyboard-heading-variant = Wariant
keyboard-heading-found = Znalezione warianty
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] Tylko standardowy
    [one] Standardowy i jeden inny
    [few] Standardowy i { $count } inne
    [many] Standardowy i { $count } innych
   *[other] Standardowy i { $count } innych
}
# Under the plain layout, first among its kinds.
keyboard-plain = Standardowy
keyboard-test = Wypróbuj klawiaturę
keyboard-test-placeholder = Wpisz coś, aby sprawdzić klawisze

## The search over a long list (Language, Time zone, Keyboard)

search-field = Szukaj
search-language-placeholder = Język, również po angielsku
search-timezone-placeholder = Miasto, kraj lub część świata
search-keyboard-placeholder = Układ albo jego wariant
search-nothing = Nic nie znaleziono dla „{ $query }”

## The step of where to install

button-options = Opcje
target-heading-disks = Dyski
target-erase = Wymaż ten dysk
target-erase-note = Wszystko na nim zostanie wymazane
target-free = Wolne miejsce · { $size }
target-free-note = Nic na dysku nie zostanie wymazane
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = Zastąp: { $what } · { $size }
target-replace-note = Jej zawartość zostanie wymazana
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = Zastąp „{ $label }” · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · do wymazania
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = „{ $label }” · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = Wolne miejsce
target-no-disk = Nie ma dysku, na którym można zainstalować Linexin.
target-options = Opcje
# Each option with the state it is in.
target-option-storage = Przechowywanie · { $value }
target-option-swap = Dodatkowa pamięć · { $value }
target-option-home = Twoje pliki · { $value }
target-storage = Przechowywanie
target-storage-standard = Standardowe
target-storage-snapshots = Z punktami przywracania
target-storage-snapshots-short = Punkty przywracania
target-swap = Dodatkowa pamięć
target-swap-off = Wyłączona
target-home = Twoje pliki
target-home-system = Razem z systemem
target-home-kept = { $part }, bez wymazywania
target-home-erased = { $part }, po wymazaniu
target-home-keep = Zachowaj zawartość
target-home-erase = Wymaż zawartość
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = Używane są Twoje partycje · { $count ->
    [one] jedna operacja
    [few] { $count } operacje
    [many] { $count } operacji
   *[other] { $count } operacji
}
target-partitions-let-go = Dysk się zmienił, więc Twoje partycje zostały odrzucone. Wybierz ponownie.

## Partycje

button-advanced = Zaawansowane
partitions-title = Partycje
partitions-not-yet = Partycji nie można jeszcze zastosować w tej wersji.
partitions-not-yet-fresh-table = Nowej tablicy partycji nie można jeszcze zastosować w tej wersji.
partitions-not-yet-keep-table = Tworzenia ani usuwania partycji nie można jeszcze zastosować w tej wersji.
partitions-not-yet-change-entries = Formatowania partycji nie można jeszcze zastosować w tej wersji.
partitions-not-yet-kept-home = Zachowania /home nie można jeszcze zastosować w tej wersji.
partitions-not-yet-several-disks = Planu na kilku dyskach nie można jeszcze zastosować w tej wersji.
partitions-not-yet-bios = Partycji na komputerze z BIOS-em nie można jeszcze zastosować w tej wersji.
partitions-not-yet-shrink = Zmniejszania partycji nie można jeszcze zastosować w tej wersji.
partitions-unallocated = nieprzydzielone
partitions-unknown = nieznany
partitions-no-table = brak tablicy partycji
partitions-partition = Partycja
partitions-filesystem = System plików
partitions-label = Etykieta
partitions-size = Rozmiar
partitions-used = Użyte
partitions-mount-point = Punkt montowania
partitions-flags = Flagi
partitions-pending = Oczekujące
partitions-start = Początek
partitions-end = Koniec
partitions-reading = Odczytywanie…
partitions-menu-mount = Punkt montowania · { $value }
partitions-menu-format = Formatuj jako · { $value }
partitions-format-as = Formatuj jako
partitions-none = brak
partitions-keep = zachowaj
partitions-delete = Usuń
partitions-undo-part = Cofnij zmiany w { $partition }
partitions-undo = Cofnij zmiany
partitions-create-table = Utwórz tablicę partycji
partitions-create-table-kind = Utwórz tablicę partycji { $table }
partitions-table = Tablica partycji
partitions-undo-last = Cofnij ostatnią operację
partitions-clear = Wyczyść wszystkie operacje
partitions-pending-list = Oczekujące operacje ({ $count })…
partitions-swap-file = Plik wymiany · { $value }
partitions-swap-file-title = Plik wymiany
partitions-pending-title = Oczekujące operacje
partitions-pending-short = Oczekujące
partitions-pending-none = Brak oczekujących operacji
partitions-pending-earlier = { $count ->
    [one] Jedna wcześniejsza operacja
    [few] { $count } wcześniejsze operacje
    [many] { $count } wcześniejszych operacji
   *[other] { $count } wcześniejszych operacji
}
partitions-tile-format = formatuj
partitions-tile-deleted = { $count ->
   *[other] usunięto { $partitions }
}
partitions-do-format = Formatuj jako { $filesystem }
partitions-do-mount = Montuj w { $path }
partitions-do-delete = Usuń { $partition }
partitions-op-delete = Usuń { $partition } ({ $filesystem }, { $size })
partitions-op-format = Sformatuj { $partition } jako { $filesystem }
partitions-op-mount = Ustaw punkt montowania { $partition } na { $path }
partitions-op-table = Utwórz tablicę partycji { $table } na { $disk }
partitions-op-create = Utwórz partycję podstawową nr { $number } ({ $filesystem }, { $size }) na { $disk }
partitions-op-create-at = Utwórz partycję podstawową nr { $number } ({ $filesystem }, { $size }, { $path }) na { $disk }
partitions-op-shrink = Zmniejsz { $partition } z { $from } do { $to }
partitions-op-grow = Powiększ { $partition } z { $from } do { $to }
partitions-op-label = Ustaw etykietę systemu plików „{ $label }” na { $partition }
partitions-op-label-clear = Usuń etykietę systemu plików na { $partition }
partitions-missing-root = Brakuje partycji zamontowanej w /
partitions-missing-efi = Brak partycji EFI w /boot lub /efi
partitions-missing-boot = Dodaj /boot albo zamontuj partycję EFI w /boot
partitions-missing-bios-grub = Brakuje partycji bios_grub na { $disk }
partitions-too-small = { $path } potrzebuje co najmniej { $size }
partitions-must-format = Sformatuj { $partition }, aby użyć jej jako { $path }
partitions-needs = { $path } wymaga { $filesystem }
partitions-needs-linux = { $path } wymaga ext4, btrfs lub xfs
partitions-needs-root = { $path } wymaga ext4 lub btrfs
partitions-twice = { $path } ustawiono na więcej niż jednej partycji
partitions-blocked = Na { $disk } nie można niczego zmienić
partitions-refused-mismatch = Plan dla { $disk } nie pasuje do dysku
partitions-refused-unaligned = { $partition }: początek i koniec na pełnym MiB
partitions-refused-outside = { $partition } wychodzi poza miejsce na dysku
partitions-refused-overlap = { $partition } nachodzi na inną partycję
partitions-refused-beside-extended = Obok partycji rozszerzonej na { $disk } nic nie powstanie
partitions-refused-logical = { $partition }: najpierw usuń dalsze partycje logiczne
partitions-refused-entries = { $disk } miałby więcej niż 128 partycji
partitions-refused-shrink = { $partition }: nie można zmniejszyć
partitions-refused-grow = { $partition } jest zachowywana, więc nie urośnie
partitions-refused-bios-grub = { $partition } nie może tu być partycją bios_grub
partitions-refused-label = { $partition }: etykieta nie pasuje do systemu plików
partitions-refused-unmountable = { $partition }: { $filesystem } nie montuje się
partitions-refused-inside-kept = { $path }: jest zachowywana, nie można w niej montować
partitions-refused-fs-small = { $partition } jest za mała na { $filesystem }
partitions-refused-restore-points = Punkty przywracania nie działają obok osobnego /var
partitions-refused-boot-disk = /boot musi być na tym samym dysku co { $path }
partitions-refused-efi-bios = /efi działa tylko, gdy komputer uruchamia UEFI
partitions-refused-grub-gap = GRUB potrzebuje wolnego pierwszego MiB na { $disk }
partitions-refused-bootloader = { $loader }: nie można stąd uruchomić Linexin
partitions-refused-not-read = Nie wiadomo jeszcze, ile zajmuje { $partition }
partitions-refused-shrink-below = { $partition } nie może mieć mniej niż { $size }
partitions-refused-hibernated = { $partition }: bez zmniejszania, Windows hibernuje
partitions-refused-needs-check = { $partition }: zmniejszenie dopiero po sprawdzeniu
partitions-refused-encrypted = { $partition }: zaszyfrowana, bez zmniejszania
partitions-refused-in-use = { $partition }: w użyciu, bez zmniejszania
partitions-refused-unreadable = { $partition }: nieczytelna, bez zmniejszania
partitions-refused-no-hiberfile = { $partition }: zmniejsz ją w Windows, nie tutaj
partitions-refused-windows-unread = Bez zmniejszania: Windows na { $other } nieczytelny
partitions-refused-bitlocker = Bez zmniejszania: { $other } ma BitLockera
partitions-why-hibernated = Windows na { $other }: wyłącz bez szybkiego uruchamiania
partitions-why-windows-unread = Zmniejszysz, gdy Windows na { $other } będzie czytelny
partitions-why-bitlocker = Aby zmniejszyć, wyłącz BitLockera na { $other }
partitions-why-no-hiberfile = Aby ją zmniejszyć, użyj Zarządzania dyskami w Windows
partitions-refused-esp-full = { $partition } potrzebuje { $size } wolnego miejsca
partitions-refused-image-unknown = Nie wiadomo jeszcze, ile Linexin umieści w { $path }
partitions-refused-unread = Nie udało się odczytać, ile zajmuje { $partition }
partitions-refused-image-unread = Nie udało się odczytać, ile Linexin umieści w { $path }
partitions-new = Nowa…
partitions-resize = Zmień rozmiar…
partitions-relabel = Etykieta…
partitions-other = Inny…
partitions-new-name = Nowa partycja nr { $number }
partitions-tile-new = nowa
partitions-new-title = Nowa partycja
partitions-resize-title = Zmień rozmiar { $partition }
partitions-label-title = Etykieta { $partition }
partitions-other-title = Punkt montowania { $partition }
partitions-new-size = Nowy rozmiar
partitions-position = Położenie
partitions-position-start = początek obszaru
partitions-position-end = koniec obszaru
partitions-bios-grub = bios_grub (niesformatowana)
partitions-cancel = Anuluj
partitions-add = Dodaj
partitions-resize-button = Zmień rozmiar
partitions-set = Ustaw
partitions-leaves = Za nią zostaje { $size } nieprzydzielonego miejsca
partitions-leaves-none = Za nią nie zostaje nieprzydzielone miejsce
partitions-takes = Zajmuje { $size } nieprzydzielonego miejsca za nią
partitions-takes-all = Zajmuje całe nieprzydzielone miejsce za nią
partitions-at-least = Potrzeba co najmniej { $size }
partitions-path-placeholder = Na przykład /data
partitions-path-whole = Punkt montowania to pełna ścieżka od /, np. /data
partitions-path-twice = { $path } jest już punktem montowania
partitions-path-system = { $path } należy do systemu
partitions-path-long = Punkt montowania ma najwyżej 255 bajtów
partitions-path-spaces = Punkt montowania nie zawiera spacji ani ukośników wstecznych
partitions-path-under = Wewnątrz { $path } nie można niczego montować
partitions-label-bytes = Etykieta { $filesystem } mieści najwyżej { $count ->
    [one] jeden bajt
    [few] { $count } bajty
    [many] { $count } bajtów
   *[other] { $count } bajta
}
partitions-label-chars = Etykieta { $filesystem } mieści najwyżej { $count ->
    [one] jeden znak
    [few] { $count } znaki
    [many] { $count } znaków
   *[other] { $count } znaku
}
partitions-label-slash = Etykieta nie może zawierać „/”
partitions-why-table = Brak tablicy partycji: wybierz to miejsce, aby ją utworzyć
partitions-why-primaries = Dysk msdos mieści najwyżej cztery partycje podstawowe
partitions-why-logical = Tutaj nie można tworzyć partycji logicznych
partitions-why-beside-extended = Obok partycji rozszerzonej nic nie powstanie
partitions-why-whole = Coś zajmuje cały dysk: nie można utworzyć tablicy ani partycji
partitions-why-unmountable = { $filesystem } nie da się zamontować: sformatuj, aby użyć
partitions-why-unformatted = Brak systemu plików: sformatuj, aby użyć
partitions-why-logical-delete = Najpierw usuń partycje logiczne o wyższych numerach
partitions-primaries = { $disk } miałby więcej niż cztery partycje podstawowe
partitions-do-create = Utwórz
partitions-do-shrink = Zmniejsz do { $size }
partitions-do-grow = Powiększ do { $size }

## The desktop step

button-done = Gotowe
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = Oparty na GNOME
desktop-about-kinexin = Oparty na KDE Plasma
desktop-about-linexinbar = Stworzony do kontrolera
# Under a desktop: whether it has to be downloaded.
desktop-ready = Nie wymaga internetu
desktop-needs-internet = Wymaga internetu
desktop-refused = { $desktop } wymaga internetu. Połącz się albo wybierz Linexin.
desktop-checking = Sprawdzanie połączenia z internetem…
desktop-checking-short = Sprawdzanie internetu…
desktop-offline = Brak internetu, więc to, co go wymaga, zostanie pominięte.
desktop-updates = Zainstaluj aktualizacje
desktop-updates-on = Wszystko będzie aktualne
desktop-updates-off = Tak jak na pendrivie
updates-refused = Aktualizacje wymagają internetu. Można je zainstalować później.
desktop-options-title = Opcje
desktop-options-caption = Co jeszcze zostanie zainstalowane i jak uruchamia się komputer.
desktop-option-remove = Oprogramowanie do usunięcia
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = Oprogramowanie
desktop-flatpaks-title = Aplikacje
desktop-bootloader-title = Start
desktop-remove-caption = Wyłącz to, czego nie chcesz w zainstalowanym systemie.
desktop-remove-looking = Sprawdzanie oprogramowania…
desktop-remove-unlisted = Nie udało się wyświetlić listy oprogramowania
desktop-remove-none = Nie ma nic do usunięcia
desktop-remove-nothing = Nic nie zostanie usunięte
desktop-remove-count = { $count ->
    [one] Jeden pominięty
    [few] { $count } pominięte
    [many] { $count } pominiętych
   *[other] { $count } pominiętych
}
desktop-option-flatpaks = Aplikacje z Flathub
desktop-flatpaks-caption = Instalowane z internetu razem z Linexin.
# How many of the apps offered are switched on.
desktop-flatpaks-count = Wybrano { $count } z { $total }
desktop-flatpak-on = Instalowana razem z Linexin
desktop-flatpak-off = Pominięta
flatpaks-refused = Aplikacje z Flathub wymagają internetu. Można je dodać później.
desktop-option-bootloader = Menedżer startu
desktop-bootloader-caption = Co uruchamia Linexin po włączeniu komputera.
desktop-option-drivers = Usuń nieużywane sterowniki
desktop-drivers-on = Nieużywane zostaną usunięte
desktop-drivers-off = Wszystkie sterowniki zostaną
desktop-option-snapshots = Punkty przywracania
desktop-snapshots-on = Zapisywane przed każdą aktualizacją
desktop-snapshots-off = Nie są zapisywane
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = Ustaw w kroku „Gdzie zainstalować”
desktop-snapshots-var = Nie obok osobnego /var
bootloader-automatic = Automatycznie
bootloader-automatic-note = Dobrany do tego komputera
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = Uruchamia Linexin od razu, bez menu
bootloader-grub = GRUB
bootloader-grub-note = Menu przy starcie, na każdym komputerze
bootloader-refind = rEFInd
bootloader-refind-note = Menu ze wszystkimi systemami na komputerze
bootloader-unavailable = Niedostępny na tym komputerze
# The groups the software to remove is listed in.
package-group-applications = Aplikacje
package-group-multimedia = Dźwięk i wideo
package-group-network = Sieć
package-group-fonts = Czcionki
package-group-libraries = Biblioteki
