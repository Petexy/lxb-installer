# Linexin Installer — Russian.
#
# Every message of en-GB.ftl, in Russian. The comments are en-GB's: they say
# where a message appears and what it means. The words the family already
# uses — Назад, Продолжить, Параметры, Закрыть, Выбрать, Готово, Установить,
# Перезагрузить сейчас, Не сейчас, Подробности, Имя пользователя, Пароль,
# Учётная запись, Программы, компьютер, диск, "попробуйте снова" — are the
# shell's and the toolkit's, and so is the polite "вы", written in lower
# case as the shell writes it. A part of a disk is a "раздел", the everyday
# word, spare memory "дополнительная память" (as the shell's Storage page
# says it), a USB stick a "флешка" and start-up "загрузка". A count takes
# Russian's own forms: one, few, many, and other behind them; "one" is also
# 21 and 31, so it always carries the number. Where a value whose gender or
# case is not known (a part, what it holds, a disk's model) would have to
# agree or decline, it stands after "раздел", "диск" or a colon, or the
# sentence is turned ("всё будет стёрто", "без стирания") so that it never
# has to. A name a person or a disk gave stands in « », as the shell sets it.

## The application itself

app-name = Установщик Linexin

## The head of each screen: its title, and the one line under it

welcome-caption = Несколько вопросов — и Linexin будет на этом компьютере.
language-title = Язык
language-caption = Язык, на котором говорит этот компьютер.
timezone-title = Часовой пояс
timezone-caption = Где вы находитесь, чтобы часы шли верно.
keyboard-title = Клавиатура
keyboard-caption = Клавиши, на которых вы печатаете.
target-title = Куда установить
target-caption = Диск, на который будет установлен Linexin.
desktop-title = Рабочий стол
desktop-caption = Как выглядит Linexin. Любой из них можно настроить позже.
account-title = Вы
account-caption = Ваше имя и учётная запись для входа.
review-title = Всё готово к установке Linexin
review-caption = Проверьте всё. Пока ничего не изменено.
installing-title = Установка Linexin
installing-caption = Это займёт какое-то время. Ничего нажимать не нужно.
done-title = Linexin установлен
done-caption = Перезагрузите компьютер и извлеките флешку, чтобы начать.
stopped-title = Установка остановилась

## The verbs at the foot of a screen

button-continue = Продолжить
button-install = Установить
button-close = Закрыть
button-restart = Перезагрузить сейчас
button-not-now = Не сейчас
button-restart-short = Перезагрузить

## What the buttons do, in the legend at the foot of the panel

hint-choose = Выбрать
hint-back = Назад
hint-quit = Выйти
hint-type = Ввод
hint-shift = Shift
hint-close = Закрыть
hint-done = Готово
hint-options = Параметры
hint-details = Подробности
hint-hide-details = Скрыть подробности
hint-search = Поиск
hint-adjust = Изменить
hint-keep = Принять
hint-put-back = Вернуть
hint-cancel = Отмена

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = àé

## Questions, asked with the safe answer first

quit-title = Выйти из установки?
quit-body = На этом компьютере ничего не изменено.
quit-stay = Остаться
quit-leave = Выйти
erase-title = Стереть и установить?
erase-title-short = Стереть?
install-title = Установить Linexin?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = Всё на диске { $disk } будет стёрто.
erase-target-part = Всё в разделе { $part } · { $size } на диске { $disk } будет стёрто.
erase-target-free = Linexin будет установлен в свободное место на диске { $disk }.
erase-home = Всё в разделе { $part }, где будут храниться ваши файлы, будет стёрто.
erase-final = Это нельзя отменить.
erase-nothing = Ничего на нём не будет стёрто.
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = Эти операции изменят содержимое ваших дисков:
erase-plan-nothing = На ваших дисках ничего не будет стёрто.
erase-plan-erased = Стирает { $partition } ({ $filesystem }, { $size })
erase-plan-esp = { $what } · системный раздел EFI
erase-plan-windows = { $what } · Windows
erase-plan-page = Страница { $page } из { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = «{ $label }»
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $part } на диске { $disk }
erase-back = Назад
erase-confirm = Стереть и установить
erase-confirm-short = Стереть
button-next = Далее

## A line beside the legend

demo-note = Вымышленный компьютер: всё ненастоящее, ничего не меняется.
install-unavailable = Чтобы установить Linexin, запустите этот компьютер с флешки Linexin.
disks-unreadable = Не удалось прочитать диски этого компьютера.
install-reading = Диски этого компьютера ещё читаются. Повторите попытку чуть позже.
installing-note = Не выключайте компьютер, пока установка не закончится.
done-all-well = Всё прошло как задумано.
done-warnings = { $count ->
    [one] Linexin готов к работе, но не всё прошло как задумано:
    [few] Linexin готов к работе, но несколько вещей прошли не так, как задумано:
    [many] Linexin готов к работе, но несколько вещей прошли не так, как задумано:
   *[other] Linexin готов к работе, но несколько вещей прошли не так, как задумано:
}
done-restart-failed = Не удалось перезагрузить компьютер. Перезагрузите его через системное меню.

## The fields

field-full-name = Полное имя
field-full-name-placeholder = Ваше имя, как его увидят другие
field-username = Имя пользователя
field-username-placeholder = С ним вы входите в систему
field-password = Пароль
field-password-placeholder = Придумайте пароль
field-repeat = Подтвердите пароль
field-repeat-placeholder = Тот же пароль ещё раз
field-hostname = Имя компьютера
field-hostname-placeholder = Имя этого компьютера в сети
field-root-password = Пароль администратора
field-root-password-placeholder = Пароль для изменений в системе
field-root-repeat = Подтвердите пароль администратора
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = Слабый
strength-fair = Средний
strength-good = Хороший
strength-strong = Надёжный

## You

# A separate administrator (root) account, off unless asked for.
account-root = Учётная запись администратора
account-root-off = Выключена: хватит вашего пароля
account-root-on = Со своим паролем, ниже

## Sizes, as a box writes them

size-tb = { $amount } ТБ
size-gb = { $amount } ГБ
size-mb = { $amount } МБ
size-kb = { $amount } КБ
size-bytes = { $amount } байт

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = Быстрый SSD-диск
disk-kind-sata = Внутренний диск
disk-kind-usb = USB-накопитель
disk-kind-card = Карта памяти
disk-kind-virtual = Виртуальный диск
disk-kind-other = Диск
disk-empty = Пустой
disk-holds-more = { $first } и ещё { $count ->
    [one] { $count } раздел
    [few] { $count } раздела
    [many] { $count } разделов
   *[other] { $count } раздела
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what } на весь диск

## What a part of a disk appears to hold

content-efi = Файлы загрузки
content-bios-boot = Место для загрузки
content-reserved = Резерв Windows
content-recovery = Восстановление Windows
content-windows = Windows
content-linux = Linux
content-swap = Дополнительная память
content-data = Файлы
content-empty = Пусто
content-other = Что-то другое

## Why a disk cannot be chosen, said under it

blocked-installation-media = С него запущен установщик
blocked-in-use = Сейчас используется
blocked-swap = Используется как дополнительная память
blocked-holder = Занят чем-то другим на этом компьютере
blocked-read-only = Можно читать, но нельзя записывать
blocked-too-small = Слишком мал для Linexin
blocked-unknown = Нельзя использовать

## The review

# Each row names a step; under it, that step's answer.
review-where = Куда
review-desktop = Рабочий стол
review-region-name = Язык и регион
review-account-name = Учётная запись
review-extras = Дополнительно
review-where-disk = { $disk } · всё на нём будет стёрто
review-where-free = { $disk } · в свободное место
review-where-part = { $disk } · вместо раздела { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = ваши файлы — в разделе { $part }, без стирания
review-where-home-erased = ваши файлы — в разделе { $part }, со стиранием
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = Пока ничего не выбрано
review-extra-updates = Обновления
review-extra-apps = { $count ->
    [one] { $count } приложение
    [few] { $count } приложения
    [many] { $count } приложений
   *[other] { $count } приложения
}
review-extra-snapshots = Точки восстановления
review-extra-swap = { $size } дополнительной памяти
review-extra-none = Больше ничего
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size } на { $disk }
review-part-formatted = { $partition } · { $filesystem } · { $size } · с форматированием
review-part-kept = { $partition } · { $filesystem } · { $size } · без форматирования
review-operations = { $count ->
    [one] Будет применена { $count } операция
    [few] Будут применены { $count } операции
    [many] Будет применено { $count } операций
   *[other] Будет применено { $count } операции
}

## The installation, while it runs

installing-waiting = Подготовка…
installing-count = Шаг { $step } из { $steps } · { $percent } %
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = Остановилась на шаге: { $step }
step-check = Проверка компьютера
step-resize = Уменьшение разделов, которые остаются
step-partition = Освобождение места на диске
step-format = Подготовка диска
step-copy = Копирование Linexin
step-swap = Выделение дополнительной памяти
step-system = Настройка системы
step-region = Настройка языка, часового пояса и клавиатуры
step-accounts = Создание вашей учётной записи
step-desktop = Настройка рабочего стола
step-updates = Установка обновлений
step-packages = Добавление и удаление программ
step-flatpaks = Установка приложений
step-drivers = Удаление драйверов, которые этому компьютеру не нужны
step-snapshots = Настройка точек восстановления
step-initramfs = Подготовка системы к запуску
step-bootloader = Настройка запуска Linexin
step-finish = Завершение

## What did not work, said once Linexin is installed

warning-updates-offline = Обновления пропущены, потому что не было интернета. Их можно установить позже.
warning-updates-failed = Не удалось установить обновления. Их можно установить позже.
warning-flatpaks-failed = Некоторые приложения не удалось установить. Их можно добавить позже.
warning-packages-kept = Некоторые программы, которые должны были быть удалены, остались.
warning-boot-entry = Если Linexin не запускается, выберите его в меню загрузки компьютера.
warning-snapshots = Не удалось настроить точки восстановления.
warning-drivers-kept = Драйверы, которые этому компьютеру не нужны, оставлены.
warning-windows-check = Дайте Windows проверить свой диск при следующем запуске. До тех пор Linexin его не трогает.

## Why the installation stopped, and what can be done about it

failure-invalid-request = Установщик запросил то, что не может сделать
failure-invalid-request-do = Закройте установщик, откройте его снова и пройдите все шаги ещё раз.
failure-consent-mismatch = Изменилось то, что должно было быть стёрто
failure-consent-mismatch-do = Ничего не стёрто. Пройдите шаги заново и проверьте диск.
failure-not-root = Установщик не получил разрешения
failure-not-root-do = Откройте установщик снова и разрешите, когда вас спросят.
failure-not-live = Этот компьютер запущен не с установщика
failure-not-live-do = Запустите компьютер с флешки Linexin и попробуйте снова.
failure-secure-boot = Включён Secure Boot
failure-secure-boot-do = Отключите Secure Boot в настройках загрузки компьютера и попробуйте снова.
failure-already-running = Уже идёт другая установка
failure-already-running-do = Дождитесь её окончания или перезагрузите компьютер и попробуйте снова.
failure-missing-tool = Не хватает того, что нужно установщику
failure-missing-tool-do = Возьмите флешку с более новым Linexin и попробуйте снова.
failure-image-invalid = Копия Linexin на флешке повреждена
failure-image-invalid-do = Запишите флешку заново и попробуйте ещё раз.
failure-disk-changed = Диск изменился после того, как его выбрали
failure-disk-changed-do = Часть диска, возможно, уже стёрта. Пройдите шаги заново и снова выберите диск.
failure-disk-changed-untouched-do = Ничего не стёрто. Пройдите шаги заново и снова выберите диск.
failure-disk-busy = Диск занят чем-то другим
failure-disk-busy-do = Закройте то, что его использует, или перезагрузите компьютер и попробуйте снова.
failure-disk-busy-untouched-do = Ничего не стёрто. Закройте то, что использует диск, или перезагрузите компьютер и попробуйте снова.
failure-no-space = Недостаточно места
failure-no-space-do = Выберите диск побольше или больше свободного места и попробуйте снова.
failure-esp-full = Место для загрузки на диске заполнено
failure-esp-full-do = Сотрите весь диск или сначала освободите на нём место для загрузки.
failure-shrink-refused = Не удалось уменьшить раздел диска
failure-shrink-refused-do = Все разделы диска на месте, но тот, что уменьшался, мог измениться. Дайте системе, которой он принадлежит, проверить его, и попробуйте снова.
failure-shrink-refused-untouched-do = Ничего не изменено. Откройте установщик снова и оставьте этому разделу больше места или уменьшите его в той системе, которой он принадлежит.
failure-windows-hibernated = Windows выключен не полностью
failure-windows-hibernated-do = Ничего не изменено. Запустите Windows, отключите быстрый запуск, выключите его и попробуйте снова.
failure-needs-check = Раздел диска нужно сначала проверить
failure-needs-check-do = Все разделы диска на месте, но тот, что уменьшался, мог измениться. Запустите систему, которой он принадлежит, дайте ей проверить диск и попробуйте снова.
failure-needs-check-untouched-do = Ничего не изменено. Запустите систему, которой принадлежит этот раздел, дайте ей проверить диск и попробуйте снова.
failure-windows-in-use = Раздел диска с Windows используется
failure-windows-in-use-do = Ничего не изменено. Закройте то, что открыло файлы Windows, или перезагрузите компьютер и попробуйте снова.
failure-windows-needs-check = Windows нужно сначала проверить свой диск
failure-windows-needs-check-do = Ничего не изменено. Запустите Windows, дайте ему проверить диск, выключите его и попробуйте снова.
failure-windows-unread = Не удалось прочитать раздел диска с Windows
failure-windows-unread-do = Ничего не изменено. Отключите диски, на которые вы не устанавливаете систему, и попробуйте снова или уменьшите Windows в самом Windows.
failure-bitlocker = Раздел диска заблокирован BitLocker
failure-bitlocker-do = Ничего не изменено. Отключите BitLocker в Windows и попробуйте снова или уменьшите Windows в самом Windows.
failure-needs-internet = Для этого выбора нужен интернет
failure-needs-internet-do = Подключитесь к интернету или выберите рабочий стол Linexin, которому он не нужен.
failure-needs-internet-untouched-do = Ничего не стёрто. Подключитесь к интернету или выберите рабочий стол Linexin, которому он не нужен.
failure-command = Во время установки что-то пошло не так
failure-command-do = Посмотрите подробности и попробуйте снова. Возможно, диск придётся стереть.
failure-internal = В установщике возникла проблема
failure-internal-do = Закройте его и попробуйте снова. Если это повторится, сохраните подробности для отчёта об ошибке.

## Why a choice cannot be installed, said beside it

invalid-username-empty = Придумайте имя пользователя
invalid-username-shape = Строчные латинские буквы, цифры, - и _, начиная с буквы
invalid-username-reserved = Это имя занято системой
invalid-full-name = В имени не может быть : или ,
invalid-hostname = Латинские буквы, цифры и -, не больше 63
invalid-password-empty = Придумайте пароль
invalid-password-mismatch = Пароли не совпадают
invalid-password-shape = Не больше 128 символов, без переносов строк
invalid-root-password-empty = Придумайте пароль для учётной записи администратора
invalid-locale = Выберите язык
invalid-timezone = Выберите часовой пояс
invalid-keyboard = Выберите клавиатуру
invalid-disk-blocked = Этот диск нельзя использовать
invalid-target-missing = Выберите, куда установить
invalid-target-small = Там недостаточно места для Linexin
table-full = На этом диске нет места для ещё одного раздела. Замените один из них.
invalid-target-esp = С этого раздела запускается компьютер
target-protected = Этот раздел Windows держит для себя
invalid-home = В этом разделе нельзя хранить ваши файлы
invalid-swap = Дополнительная память больше, чем там есть места
invalid-bootloader = Этот менеджер загрузки нельзя использовать на этом компьютере
invalid-source = Не удаётся найти копию Linexin для установки
invalid-layout = Ваши разделы нельзя использовать так, как задумано

## The time zone step

timezone-heading-region = Часть света
timezone-heading-city = Ближайший город
timezone-heading-found = Найденные места
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] { $count } место
    [few] { $count } места
    [many] { $count } мест
   *[other] { $count } места
}
zone-region-africa = Африка
zone-region-america = Америка
zone-region-antarctica = Антарктида
zone-region-arctic = Арктика
zone-region-asia = Азия
zone-region-atlantic = Атлантический океан
zone-region-australia = Австралия
zone-region-europe = Европа
zone-region-indian = Индийский океан
zone-region-pacific = Тихий океан
zone-region-utc = Всемирное время
# The one place under Universal time: the clock no country keeps.
zone-utc = Всемирное координированное время

## The keyboard step

keyboard-heading-layout = Раскладка
keyboard-heading-variant = Вариант
keyboard-heading-found = Найденные варианты
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] Только стандартный
    [one] Стандартный и ещё { $count }
    [few] Стандартный и ещё { $count }
    [many] Стандартный и ещё { $count }
   *[other] Стандартный и ещё { $count }
}
# Under the plain layout, first among its kinds.
keyboard-plain = Стандартный
keyboard-test = Проверьте клавиатуру
keyboard-test-placeholder = Напечатайте что-нибудь для проверки

## The search over a long list (Language, Time zone, Keyboard)

search-field = Поиск
search-language-placeholder = Язык, также по-английски
search-timezone-placeholder = Город, страна или часть света
search-keyboard-placeholder = Раскладка или её вариант
search-nothing = Ничего не найдено по запросу «{ $query }»

## The step of where to install

button-options = Параметры
target-heading-disks = Диски
target-erase = Стереть этот диск
target-erase-note = Всё на нём будет стёрто
target-free = Свободное место · { $size }
target-free-note = Ничего на диске не стирается
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = Заменить: { $what } · { $size }
target-replace-note = Всё в нём будет стёрто
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = Заменить «{ $label }» · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · всё будет стёрто
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = «{ $label }» · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = Свободное место
target-no-disk = Нет диска, на который можно установить Linexin.
target-options = Параметры
# Each option with the state it is in.
target-option-storage = Хранилище · { $value }
target-option-swap = Дополнительная память · { $value }
target-option-home = Ваши файлы · { $value }
target-storage = Хранилище
target-storage-standard = Стандартное
target-storage-snapshots = С точками восстановления
target-storage-snapshots-short = Точки восстановления
target-swap = Дополнительная память
target-swap-off = Выключена
target-home = Ваши файлы
target-home-system = Вместе с системой
target-home-kept = { $part }, без стирания
target-home-erased = { $part }, со стиранием
target-home-keep = Сохранить содержимое
target-home-erase = Стереть содержимое
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = Используются ваши разделы · { $count ->
    [one] { $count } операция
    [few] { $count } операции
    [many] { $count } операций
   *[other] { $count } операции
}
target-partitions-let-go = Диск изменился, поэтому ваши разделы сброшены. Выберите снова.

## Разделы

button-advanced = Дополнительно
partitions-title = Разделы
partitions-not-yet = В этой версии разделы пока нельзя применить.
partitions-not-yet-fresh-table = В этой версии новую таблицу разделов пока нельзя применить.
partitions-not-yet-keep-table = В этой версии создание и удаление разделов пока нельзя применить.
partitions-not-yet-change-entries = В этой версии форматирование раздела пока нельзя применить.
partitions-not-yet-kept-home = В этой версии сохранение /home пока нельзя применить.
partitions-not-yet-several-disks = В этой версии план для нескольких дисков пока нельзя применить.
partitions-not-yet-bios = В этой версии разделы на компьютере с BIOS пока нельзя применить.
partitions-not-yet-shrink = В этой версии уменьшение раздела пока нельзя применить.
partitions-unallocated = не размечено
partitions-unknown = неизвестно
partitions-no-table = нет таблицы разделов
partitions-partition = Раздел
partitions-filesystem = Файловая система
partitions-label = Метка
partitions-size = Размер
partitions-used = Использовано
partitions-mount-point = Точка монтирования
partitions-flags = Флаги
partitions-pending = Ожидает
partitions-start = Начало
partitions-end = Конец
partitions-reading = Чтение…
partitions-menu-mount = Точка монтирования · { $value }
partitions-menu-format = Форматировать в · { $value }
partitions-format-as = Форматировать в
partitions-none = нет
partitions-keep = оставить
partitions-delete = Удалить
partitions-undo-part = Отменить изменения { $partition }
partitions-undo = Отменить изменения
partitions-create-table = Создать таблицу разделов
partitions-create-table-kind = Создать таблицу разделов { $table }
partitions-table = Таблица разделов
partitions-undo-last = Отменить последнюю операцию
partitions-clear = Очистить все операции
partitions-pending-list = Ожидающие операции ({ $count })…
partitions-swap-file = Файл подкачки · { $value }
partitions-swap-file-title = Файл подкачки
partitions-pending-title = Ожидающие операции
partitions-pending-short = В очереди
partitions-pending-none = Нет ожидающих операций
partitions-pending-earlier = { $count ->
    [one] { $count } более ранняя операция
    [few] { $count } более ранние операции
    [many] { $count } более ранних операций
   *[other] { $count } более ранней операции
}
partitions-tile-format = формат
partitions-tile-deleted = { $count ->
   *[other] удалено: { $partitions }
}
partitions-do-format = Форматировать в { $filesystem }
partitions-do-mount = Монтировать в { $path }
partitions-do-delete = Удалить { $partition }
partitions-op-delete = Удалить { $partition } ({ $filesystem }, { $size })
partitions-op-format = Форматировать { $partition } в { $filesystem }
partitions-op-mount = Задать точку монтирования { $partition }: { $path }
partitions-op-table = Создать таблицу разделов { $table } на { $disk }
partitions-op-create = Создать основной раздел №{ $number } ({ $filesystem }, { $size }) на { $disk }
partitions-op-create-at = Создать основной раздел №{ $number } ({ $filesystem }, { $size }, { $path }) на { $disk }
partitions-op-shrink = Уменьшить { $partition } с { $from } до { $to }
partitions-op-grow = Увеличить { $partition } с { $from } до { $to }
partitions-op-label = Задать метку файловой системы «{ $label }» для { $partition }
partitions-op-label-clear = Удалить метку файловой системы { $partition }
partitions-missing-root = Нет раздела, смонтированного в /
partitions-missing-efi = Нет раздела EFI в /boot или /efi
partitions-missing-boot = Добавьте /boot или смонтируйте раздел EFI в /boot
partitions-missing-bios-grub = Нет раздела bios_grub на { $disk }
partitions-too-small = Для { $path } нужно не меньше { $size }
partitions-must-format = Отформатируйте { $partition } для { $path }
partitions-needs = Для { $path } нужна { $filesystem }
partitions-needs-linux = Для { $path } нужна ext4, btrfs или xfs
partitions-needs-root = Для { $path } нужна ext4 или btrfs
partitions-twice = { $path } назначена больше чем одному разделу
partitions-blocked = На { $disk } ничего нельзя изменить
partitions-refused-mismatch = План для { $disk } не совпадает с диском
partitions-refused-unaligned = { $partition }: начало и конец — на целых MiB
partitions-refused-outside = { $partition } выходит за пределы места на диске
partitions-refused-overlap = { $partition } перекрывает другой раздел
partitions-refused-beside-extended = { $disk }: рядом с расширенным разделом — ничего
partitions-refused-logical = { $partition }: сначала удалите разделы после него
partitions-refused-entries = { $disk }: больше 128 разделов
partitions-refused-shrink = { $partition } нельзя уменьшить
partitions-refused-grow = { $partition } сохраняется, поэтому не может расти
partitions-refused-bios-grub = { $partition } не может быть здесь bios_grub
partitions-refused-label = { $partition }: метка не подходит файловой системе
partitions-refused-unmountable = { $partition }: { $filesystem } не монтируется
partitions-refused-inside-kept = { $path }: сохраняется, внутри монтировать нельзя
partitions-refused-fs-small = { $partition } слишком мал для { $filesystem }
partitions-refused-restore-points = Точки восстановления несовместимы с отдельным /var
partitions-refused-boot-disk = /boot должен быть на том же диске, что и { $path }
partitions-refused-efi-bios = /efi нужен, только если компьютер запускает UEFI
partitions-refused-grub-gap = GRUB нужен свободный первый MiB на { $disk }
partitions-refused-bootloader = { $loader } не запустит Linexin с этих разделов
partitions-refused-not-read = Пока неизвестно, сколько занято на { $partition }
partitions-refused-shrink-below = { $partition } не может быть меньше { $size }
partitions-refused-hibernated = { $partition } не уменьшить: гибернация Windows
partitions-refused-needs-check = { $partition }: сначала нужна проверка
partitions-refused-encrypted = { $partition } зашифрован, его нельзя уменьшить
partitions-refused-in-use = { $partition } используется, его нельзя уменьшить
partitions-refused-unreadable = { $partition } не читается, его нельзя уменьшить
partitions-refused-no-hiberfile = { $partition }: уменьшите его в Windows, не здесь
partitions-refused-windows-unread = Не уменьшить: Windows на { $other } не прочитан
partitions-refused-bitlocker = Не уменьшить: { $other } заблокирован BitLocker
partitions-why-hibernated = Windows на { $other }: выключите без быстрого запуска
partitions-why-windows-unread = Уменьшить можно, когда Windows на { $other } прочитается
partitions-why-bitlocker = Чтобы уменьшить, отключите BitLocker на { $other }
partitions-why-no-hiberfile = Чтобы уменьшить, откройте «Управление дисками» в Windows
partitions-refused-esp-full = На { $partition } нужно { $size } свободного места
partitions-refused-image-unknown = Пока неизвестно, сколько Linexin кладёт в { $path }
partitions-refused-unread = Не удалось узнать, сколько занято на { $partition }
partitions-refused-image-unread = Не удалось узнать, сколько Linexin кладёт в { $path }
partitions-new = Создать…
partitions-resize = Изменить размер…
partitions-relabel = Метка…
partitions-other = Другая…
partitions-new-name = Новый раздел №{ $number }
partitions-tile-new = новый
partitions-new-title = Новый раздел
partitions-resize-title = Изменить размер { $partition }
partitions-label-title = Метка { $partition }
partitions-other-title = Точка монтирования { $partition }
partitions-new-size = Новый размер
partitions-position = Расположение
partitions-position-start = начало области
partitions-position-end = конец области
partitions-bios-grub = bios_grub (без ФС)
partitions-cancel = Отмена
partitions-add = Добавить
partitions-resize-button = Изменить
partitions-set = Задать
partitions-leaves = После него остаётся { $size } неразмеченного места
partitions-leaves-none = После него не остаётся неразмеченного места
partitions-takes = Занимает { $size } неразмеченного места после него
partitions-takes-all = Занимает всё неразмеченное место после него
partitions-at-least = Нужно не меньше { $size }
partitions-path-placeholder = Например, /data
partitions-path-whole = Точка монтирования — полный путь от /, например /data
partitions-path-twice = { $path } уже используется как точка монтирования
partitions-path-system = { $path } принадлежит системе
partitions-path-long = Точка монтирования — не длиннее 255 байт
partitions-path-spaces = В точке монтирования нет пробелов и обратных косых черт
partitions-path-under = Внутри { $path } ничего нельзя монтировать
partitions-label-bytes = Метка { $filesystem }: не больше { $count ->
    [one] { $count } байта
   *[other] { $count } байт
}
partitions-label-chars = Метка { $filesystem }: не больше { $count ->
    [one] { $count } символа
   *[other] { $count } символов
}
partitions-label-slash = Метка не может содержать «/»
partitions-why-table = Нет таблицы разделов: выберите место, чтобы её создать
partitions-why-primaries = На диске msdos не больше четырёх основных разделов
partitions-why-logical = Логические разделы здесь создать нельзя
partitions-why-beside-extended = Рядом с расширенным разделом ничего не создать
partitions-why-whole = Что-то занимает весь диск: таблицу или раздел не создать
partitions-why-unmountable = { $filesystem } не монтируется: сначала форматируйте
partitions-why-unformatted = Нет файловой системы: отформатируйте раздел
partitions-why-logical-delete = Сначала удалите логические разделы с номерами после него
partitions-primaries = { $disk }: больше четырёх основных разделов
partitions-do-create = Создать
partitions-do-shrink = Уменьшить до { $size }
partitions-do-grow = Увеличить до { $size }

## The desktop step

button-done = Готово
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = На основе GNOME
desktop-about-kinexin = На основе KDE Plasma
desktop-about-linexinbar = Создан для контроллера
# Under a desktop: whether it has to be downloaded.
desktop-ready = Интернет не нужен
desktop-needs-internet = Нужен интернет
desktop-refused = Для { $desktop } нужен интернет. Подключитесь или выберите Linexin.
desktop-checking = Проверка подключения к интернету…
desktop-checking-short = Проверка интернета…
desktop-offline = Интернета нет, поэтому то, что требует его, пропускается.
desktop-updates = Установить обновления
desktop-updates-on = Всё будет обновлено
desktop-updates-off = Как на флешке
updates-refused = Для обновлений нужен интернет. Их можно установить позже.
desktop-options-title = Параметры
desktop-options-caption = Что ещё установить и как будет запускаться компьютер.
desktop-option-remove = Программы для удаления
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = Программы
desktop-flatpaks-title = Приложения
desktop-bootloader-title = Загрузка
desktop-remove-caption = Выключите то, что вам не нужно в установленной системе.
desktop-remove-looking = Получение списка программ…
desktop-remove-unlisted = Не удалось получить список программ
desktop-remove-none = Удалять нечего
desktop-remove-nothing = Ничего не удаляется
desktop-remove-count = { $count ->
    [one] { $count } программа убрана
    [few] { $count } программы убраны
    [many] { $count } программ убрано
   *[other] { $count } программы убраны
}
desktop-option-flatpaks = Приложения из Flathub
desktop-flatpaks-caption = Устанавливаются из интернета вместе с Linexin.
# How many of the apps offered are switched on.
desktop-flatpaks-count = Выбрано { $count } из { $total }
desktop-flatpak-on = Устанавливается с Linexin
desktop-flatpak-off = Пропускается
flatpaks-refused = Для приложений из Flathub нужен интернет. Их можно добавить позже.
desktop-option-bootloader = Менеджер загрузки
desktop-bootloader-caption = Что запускает Linexin при включении компьютера.
desktop-option-drivers = Удалить ненужные драйверы
desktop-drivers-on = Неиспользуемые удаляются
desktop-drivers-off = Все драйверы остаются
desktop-option-snapshots = Точки восстановления
desktop-snapshots-on = Сохраняются перед каждым обновлением
desktop-snapshots-off = Не сохраняются
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = Включаются на шаге «Куда установить»
desktop-snapshots-var = Не с отдельным /var
bootloader-automatic = Автоматически
bootloader-automatic-note = Подбирается под этот компьютер
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = Сразу запускает Linexin, без меню
bootloader-grub = GRUB
bootloader-grub-note = Меню при запуске, на любом компьютере
bootloader-refind = rEFInd
bootloader-refind-note = Меню со всеми системами компьютера
bootloader-unavailable = Нельзя использовать на этом компьютере
# The groups the software to remove is listed in.
package-group-applications = Приложения
package-group-multimedia = Звук и видео
package-group-network = Сеть
package-group-fonts = Шрифты
package-group-libraries = Библиотеки
