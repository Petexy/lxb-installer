# Linexin Installer — Spanish.
#
# Every message of en-GB.ftl, in the Spanish of Spain. The comments are
# en-GB's: they say where a message appears and what it means. The words the
# family already uses — Atrás, Continuar, Opciones, Cerrar, Elegir, Hecho,
# Instalar, Reiniciar ahora, Ahora no, ordenador, disco — are the shell's and
# the toolkit's, and so is the polite "usted". A part of a disk is a "parte"
# and spare memory "memoria adicional". Where a value whose gender or number
# is not known (a part, what it holds) would have to agree with a verb, the
# sentence is turned ("lo que hay en", "contenido borrado") so that it never
# has to. A name a person or a disk gave stands in Spanish quotes, « ».

## The application itself

app-name = Instalador de Linexin

## The head of each screen: its title, and the one line under it

welcome-caption = Unas pocas preguntas y Linexin estará en este ordenador.
language-title = Idioma
language-caption = El idioma que habla este ordenador.
timezone-title = Zona horaria
timezone-caption = Dónde está, para que el reloj dé bien la hora.
keyboard-title = Teclado
keyboard-caption = Las teclas con las que escribe.
target-title = Dónde instalar
target-caption = El disco en el que irá Linexin.
desktop-title = Escritorio
desktop-caption = El aspecto de Linexin. Cada uno se puede personalizar después.
account-title = Sobre usted
account-caption = Su nombre y la cuenta con la que inicia sesión.
review-title = Listo para instalar Linexin
review-caption = Revíselo. Todavía no se ha cambiado nada.
installing-title = Instalando Linexin
installing-caption = Esto tarda un rato. No hace falta pulsar nada.
done-title = Linexin está instalado
done-caption = Reinicie el ordenador y retire la memoria USB para empezar a usarlo.
stopped-title = La instalación se ha detenido

## The verbs at the foot of a screen

button-continue = Continuar
button-install = Instalar
button-close = Cerrar
button-restart = Reiniciar ahora
button-not-now = Ahora no
button-restart-short = Reiniciar

## What the buttons do, in the legend at the foot of the panel

hint-choose = Elegir
hint-back = Atrás
hint-quit = Salir
hint-type = Escribir
hint-shift = Mayús
hint-close = Cerrar
hint-done = Hecho
hint-options = Opciones
hint-details = Detalles
hint-hide-details = Ocultar detalles
hint-search = Buscar
hint-adjust = Ajustar
hint-keep = Aceptar
hint-put-back = Restaurar
hint-cancel = Cancelar

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = áñ

## Questions, asked with the safe answer first

quit-title = ¿Salir del instalador?
quit-body = No se ha cambiado nada en este ordenador.
quit-stay = Quedarse
quit-leave = Salir
erase-title = ¿Borrar e instalar?
erase-title-short = ¿Borrar?
install-title = ¿Instalar Linexin?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = Se borrará todo lo que hay en { $disk }.
erase-target-part = Se borrará lo que hay en { $part } · { $size } de { $disk }.
erase-target-free = Linexin irá en el espacio libre de { $disk }.
erase-home = Se borrará lo que hay en { $part }, donde se guardarán sus archivos.
erase-final = Esto no se puede deshacer.
erase-nothing = No se borra nada de lo que contiene.
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = Estas operaciones cambian lo que hay en sus discos:
erase-plan-nothing = No se borra nada de sus discos.
erase-plan-erased = Borra { $partition } ({ $filesystem }, { $size })
erase-plan-esp = { $what } · partición del sistema EFI
erase-plan-windows = { $what } · Windows
erase-plan-page = Página { $page } de { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = «{ $label }»
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $part } en { $disk }
erase-back = Volver
erase-confirm = Borrar e instalar
erase-confirm-short = Borrar
button-next = Siguiente

## A line beside the legend

demo-note = Un ordenador inventado: aquí nada es real y no se cambia nada.
install-unavailable = Inicie este ordenador desde una memoria USB de Linexin para instalar Linexin en él.
disks-unreadable = No se han podido leer los discos de este ordenador.
install-reading = Todavía se están leyendo los discos de este ordenador. Inténtelo de nuevo en un momento.
installing-note = Mantenga el ordenador encendido hasta que termine.
done-all-well = Todo ha salido según lo previsto.
done-warnings = { $count ->
    [one] Linexin está listo para usarse, pero una cosa no ha salido según lo previsto:
   *[other] Linexin está listo para usarse, pero algunas cosas no han salido según lo previsto:
}
done-restart-failed = No se ha podido reiniciar el ordenador. Reinícielo desde el menú del sistema.

## The fields

field-full-name = Nombre completo
field-full-name-placeholder = Su nombre, como lo verán los demás
field-username = Nombre de usuario
field-username-placeholder = Con lo que inicia sesión
field-password = Contraseña
field-password-placeholder = Elija una contraseña
field-repeat = Confirmar la contraseña
field-repeat-placeholder = La misma contraseña otra vez
field-hostname = Nombre del ordenador
field-hostname-placeholder = El nombre del ordenador en una red
field-root-password = Contraseña de administrador
field-root-password-placeholder = Contraseña para cambiar el sistema
field-root-repeat = Confirmar la contraseña de administrador
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = Débil
strength-fair = Aceptable
strength-good = Buena
strength-strong = Fuerte

## You

# A separate administrator (root) account, off unless asked for.
account-root = Cuenta de administrador
account-root-off = Desactivada: basta con su contraseña
account-root-on = Con su propia contraseña, aquí debajo

## Sizes, as a box writes them

size-tb = { $amount } TB
size-gb = { $amount } GB
size-mb = { $amount } MB
size-kb = { $amount } kB
size-bytes = { $amount } bytes

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = Disco SSD rápido
disk-kind-sata = Disco interno
disk-kind-usb = Unidad USB
disk-kind-card = Tarjeta de memoria
disk-kind-virtual = Disco virtual
disk-kind-other = Disco
disk-empty = Vacío
disk-holds-more = { $first } y { $count ->
    [one] otra parte
   *[other] { $count } partes más
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what }, en todo el disco

## What a part of a disk appears to hold

content-efi = Archivos de arranque
content-bios-boot = Espacio de arranque
content-reserved = Espacio reservado de Windows
content-recovery = Recuperación de Windows
content-windows = Windows
content-linux = Linux
content-swap = Memoria adicional
content-data = Archivos
content-empty = Vacío
content-other = Otra cosa

## Why a disk cannot be chosen, said under it

blocked-installation-media = Lo usa el instalador
blocked-in-use = En uso ahora mismo
blocked-swap = En uso como memoria adicional
blocked-holder = Lo usa otra cosa de este ordenador
blocked-read-only = Se puede leer, pero no escribir
blocked-too-small = Demasiado pequeño para Linexin
blocked-unknown = No se puede usar

## The review

# Each row names a step; under it, that step's answer.
review-where = Dónde
review-desktop = Escritorio
review-region-name = Idioma y región
review-account-name = Cuenta
review-extras = Extras
review-where-disk = { $disk } · se borra todo lo que contiene
review-where-free = { $disk } · en su espacio libre
review-where-part = { $disk } · en lugar de { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = sus archivos en { $part }, contenido conservado
review-where-home-erased = sus archivos en { $part }, contenido borrado
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = Aún sin elegir
review-extra-updates = Actualizaciones
review-extra-apps = { $count ->
    [one] Una aplicación
   *[other] { $count } aplicaciones
}
review-extra-snapshots = Puntos de restauración
review-extra-swap = { $size } de memoria adicional
review-extra-none = Nada más
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size } en { $disk }
review-part-formatted = { $partition } · { $filesystem } · { $size } · formateada
review-part-kept = { $partition } · { $filesystem } · { $size } · conservada
review-operations = { $count ->
    [one] Se aplicará una operación
   *[other] Se aplicarán { $count } operaciones
}

## The installation, while it runs

installing-waiting = Preparando…
installing-count = Paso { $step } de { $steps } · { $percent } %
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = Se ha detenido en: { $step }
step-check = Comprobando el ordenador
step-resize = Reduciendo lo que se queda en el disco
step-partition = Haciendo sitio en el disco
step-format = Preparando el disco
step-copy = Copiando Linexin
step-swap = Reservando memoria adicional
step-system = Configurando el sistema
step-region = Configurando el idioma, la zona horaria y el teclado
step-accounts = Creando su cuenta
step-desktop = Configurando el escritorio
step-updates = Instalando actualizaciones
step-packages = Añadiendo y quitando software
step-flatpaks = Instalando aplicaciones
step-drivers = Quitando los controladores que este ordenador no necesita
step-snapshots = Configurando los puntos de restauración
step-initramfs = Preparando el arranque del sistema
step-bootloader = Haciendo que el ordenador arranque Linexin
step-finish = Terminando

## What did not work, said once Linexin is installed

warning-updates-offline = Las actualizaciones se han omitido porque no había internet. Se pueden instalar más tarde.
warning-updates-failed = No se han podido instalar las actualizaciones. Se pueden instalar más tarde.
warning-flatpaks-failed = No se han podido instalar algunas aplicaciones. Se pueden añadir más tarde.
warning-packages-kept = Sigue ahí parte del software que se iba a quitar.
warning-boot-entry = Si Linexin no arranca, elíjalo en el menú de arranque del ordenador.
warning-snapshots = No se han podido configurar los puntos de restauración.
warning-drivers-kept = Se han dejado los controladores que este ordenador no necesita.
warning-windows-check = Deje que Windows revise su disco al arrancar. Hasta entonces, Linexin no toca sus archivos.

## Why the installation stopped, and what can be done about it

failure-invalid-request = El instalador ha pedido algo que no puede hacer
failure-invalid-request-do = Cierre el instalador, vuelva a abrirlo y repita los pasos.
failure-consent-mismatch = Ha cambiado lo que se iba a borrar
failure-consent-mismatch-do = No se ha borrado nada. Repita los pasos y revise el disco.
failure-not-root = El instalador no ha recibido permiso
failure-not-root-do = Vuelva a abrir el instalador y permítalo cuando se le pida.
failure-not-live = Este ordenador no se ha iniciado desde el instalador
failure-not-live-do = Inicie el ordenador desde la memoria USB de Linexin y vuelva a intentarlo.
failure-secure-boot = Secure Boot está activado
failure-secure-boot-do = Desactive Secure Boot en los ajustes de arranque del ordenador y vuelva a intentarlo.
failure-already-running = Ya hay otra instalación en marcha
failure-already-running-do = Espere a que termine, o reinicie el ordenador y vuelva a intentarlo.
failure-missing-tool = Falta algo que el instalador necesita
failure-missing-tool-do = Use una memoria USB de Linexin más reciente y vuelva a intentarlo.
failure-image-invalid = La copia de Linexin de la memoria USB está dañada
failure-image-invalid-do = Vuelva a grabar la memoria USB e inténtelo de nuevo.
failure-disk-changed = El disco ha cambiado después de elegirlo
failure-disk-changed-do = Puede que parte del disco ya se haya borrado. Repita los pasos y vuelva a elegir el disco.
failure-disk-changed-untouched-do = No se ha borrado nada. Repita los pasos y vuelva a elegir el disco.
failure-disk-busy = Otra cosa está usando el disco
failure-disk-busy-do = Cierre lo que lo esté usando, o reinicie el ordenador, y vuelva a intentarlo.
failure-disk-busy-untouched-do = No se ha borrado nada. Cierre lo que esté usando el disco, o reinicie el ordenador, y vuelva a intentarlo.
failure-no-space = No hay espacio suficiente
failure-no-space-do = Elija un disco más grande o más espacio libre y vuelva a intentarlo.
failure-esp-full = El espacio de arranque del disco está lleno
failure-esp-full-do = Borre todo el disco, o antes haga sitio en su espacio de arranque.
failure-shrink-refused = No se ha podido reducir una parte del disco
failure-shrink-refused-do = Todas las partes del disco siguen ahí, pero la que se estaba reduciendo puede haber cambiado. Deje que el sistema al que pertenece la revise y vuelva a intentarlo.
failure-shrink-refused-untouched-do = No se ha cambiado nada. Vuelva a abrir el instalador y deje más espacio a esa parte, o redúzcala desde el sistema al que pertenece.
failure-windows-hibernated = Windows no está apagado del todo
failure-windows-hibernated-do = No se ha cambiado nada. Inicie Windows, desactive el inicio rápido, apáguelo y vuelva a intentarlo.
failure-needs-check = Hay que revisar antes una parte del disco
failure-needs-check-do = Todas las partes del disco siguen ahí, pero la que se estaba reduciendo puede haber cambiado. Inicie el sistema al que pertenece, deje que revise su disco y vuelva a intentarlo.
failure-needs-check-untouched-do = No se ha cambiado nada. Inicie el sistema al que pertenece esa parte, deje que revise su disco y vuelva a intentarlo.
failure-windows-in-use = Una parte de un disco con Windows está en uso
failure-windows-in-use-do = No se ha cambiado nada. Cierre lo que tenga abiertos archivos de Windows, o reinicie el ordenador, y vuelva a intentarlo.
failure-windows-needs-check = Windows tiene que revisar antes su disco
failure-windows-needs-check-do = No se ha cambiado nada. Inicie Windows, deje que revise su disco, apáguelo y vuelva a intentarlo.
failure-windows-unread = No se ha podido leer una parte de un disco con Windows
failure-windows-unread-do = No se ha cambiado nada. Desconecte los discos en los que no va a instalar y vuelva a intentarlo, o reduzca Windows desde el propio Windows.
failure-bitlocker = Una parte de un disco está bloqueada con BitLocker
failure-bitlocker-do = No se ha cambiado nada. Desactive BitLocker en Windows y vuelva a intentarlo, o reduzca Windows desde el propio Windows.
failure-needs-internet = Esta opción necesita internet
failure-needs-internet-do = Conéctese a internet, o elija el escritorio Linexin, que no lo necesita.
failure-needs-internet-untouched-do = No se ha borrado nada. Conéctese a internet, o elija el escritorio Linexin, que no lo necesita.
failure-command = Algo ha fallado durante la instalación
failure-command-do = Mire los detalles y vuelva a intentarlo. Puede que haya que borrar el disco.
failure-internal = El instalador ha tenido un problema
failure-internal-do = Ciérrelo y vuelva a intentarlo. Si vuelve a pasar, guarde los detalles para informar del fallo.

## Why a choice cannot be installed, said beside it

invalid-username-empty = Elija un nombre de usuario
invalid-username-shape = Minúsculas, dígitos, - y _, empezando por una letra
invalid-username-reserved = Ese nombre pertenece al sistema
invalid-full-name = Un nombre no puede llevar : ni ,
invalid-hostname = Letras, dígitos y -, 63 como máximo
invalid-password-empty = Elija una contraseña
invalid-password-mismatch = Las dos contraseñas no coinciden
invalid-password-shape = 128 caracteres como máximo, sin saltos de línea
invalid-root-password-empty = Elija una contraseña para la cuenta de administrador
invalid-locale = Elija un idioma
invalid-timezone = Elija una zona horaria
invalid-keyboard = Elija un teclado
invalid-disk-blocked = Este disco no se puede usar
invalid-target-missing = Elija dónde instalar
invalid-target-small = Ahí no hay espacio suficiente para Linexin
table-full = Este disco no tiene sitio para otra parte. Reemplace una.
invalid-target-esp = Esa parte es la que arranca el ordenador
target-protected = Windows se reserva esa parte
invalid-home = Esa parte no puede guardar sus archivos
invalid-swap = La memoria adicional es mayor que el espacio disponible
invalid-bootloader = Ese gestor de arranque no se puede usar en este ordenador
invalid-source = No se encuentra la copia de Linexin que se va a instalar
invalid-layout = Sus particiones no se pueden usar tal como se planearon

## The time zone step

timezone-heading-region = Región del mundo
timezone-heading-city = Ciudad más cercana
timezone-heading-found = Lugares encontrados
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] Un lugar
   *[other] { $count } lugares
}
zone-region-africa = África
zone-region-america = América
zone-region-antarctica = Antártida
zone-region-arctic = Ártico
zone-region-asia = Asia
zone-region-atlantic = Océano Atlántico
zone-region-australia = Australia
zone-region-europe = Europa
zone-region-indian = Océano Índico
zone-region-pacific = Océano Pacífico
zone-region-utc = Hora universal
# The one place under Universal time: the clock no country keeps.
zone-utc = Tiempo universal coordinado

## The keyboard step

keyboard-heading-layout = Distribución
keyboard-heading-variant = Variante
keyboard-heading-found = Variantes encontradas
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] Solo la estándar
    [one] La estándar y otra
   *[other] La estándar y { $count } más
}
# Under the plain layout, first among its kinds.
keyboard-plain = Estándar
keyboard-test = Pruebe su teclado
keyboard-test-placeholder = Escriba algo para probar las teclas

## The search over a long list (Language, Time zone, Keyboard)

search-field = Buscar
search-language-placeholder = Un idioma, también en inglés
search-timezone-placeholder = Ciudad, país o parte del mundo
search-keyboard-placeholder = Una distribución o una variante
search-nothing = No se encontró nada para «{ $query }»

## The step of where to install

button-options = Opciones
target-heading-disks = Discos
target-erase = Borrar este disco
target-erase-note = Se borra todo lo que contiene
target-free = Espacio libre · { $size }
target-free-note = No se borra nada del disco
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = Reemplazar { $what } · { $size }
target-replace-note = Se borra lo que contiene
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = Reemplazar «{ $label }» · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · contenido borrado
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = «{ $label }» · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = Espacio libre
target-no-disk = No hay ningún disco en el que se pueda instalar Linexin.
target-options = Opciones
# Each option with the state it is in.
target-option-storage = Almacenamiento · { $value }
target-option-swap = Memoria adicional · { $value }
target-option-home = Sus archivos · { $value }
target-storage = Almacenamiento
target-storage-standard = Estándar
target-storage-snapshots = Con puntos de restauración
target-storage-snapshots-short = Con restauración
target-swap = Memoria adicional
target-swap-off = Desactivada
target-home = Sus archivos
target-home-system = Con el sistema
target-home-kept = { $part }, contenido conservado
target-home-erased = { $part }, contenido borrado
target-home-keep = Conservar lo que contiene
target-home-erase = Borrar lo que contiene
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = Se usan sus particiones · { $count ->
    [one] una operación
   *[other] { $count } operaciones
}
target-partitions-let-go = Un disco ha cambiado, así que se han descartado sus particiones. Elija de nuevo.

## Particiones

button-advanced = Avanzado
partitions-title = Particiones
partitions-not-yet = Las particiones aún no se pueden aplicar en esta versión.
partitions-not-yet-fresh-table = Una tabla de particiones nueva aún no se puede aplicar en esta versión.
partitions-not-yet-keep-table = Crear o eliminar particiones aún no se puede aplicar en esta versión.
partitions-not-yet-change-entries = Formatear una partición aún no se puede aplicar en esta versión.
partitions-not-yet-kept-home = Conservar un /home aún no se puede aplicar en esta versión.
partitions-not-yet-several-disks = Un plan en varios discos aún no se puede aplicar en esta versión.
partitions-not-yet-bios = Las particiones en un equipo con BIOS aún no se pueden aplicar en esta versión.
partitions-not-yet-shrink = Reducir una partición aún no se puede aplicar en esta versión.
partitions-unallocated = sin asignar
partitions-unknown = desconocido
partitions-no-table = sin tabla de particiones
partitions-partition = Partición
partitions-filesystem = Sistema de archivos
partitions-label = Etiqueta
partitions-size = Tamaño
partitions-used = Usado
partitions-mount-point = Punto de montaje
partitions-flags = Indicadores
partitions-pending = Pendiente
partitions-start = Inicio
partitions-end = Fin
partitions-reading = Leyendo…
partitions-menu-mount = Punto de montaje · { $value }
partitions-menu-format = Formatear como · { $value }
partitions-format-as = Formatear como
partitions-none = ninguno
partitions-keep = conservar
partitions-delete = Eliminar
partitions-undo-part = Deshacer los cambios en { $partition }
partitions-undo = Deshacer los cambios
partitions-create-table = Crear tabla de particiones
partitions-create-table-kind = Crear tabla de particiones { $table }
partitions-table = Tabla de particiones
partitions-undo-last = Deshacer la última operación
partitions-clear = Borrar todas las operaciones
partitions-pending-list = Operaciones pendientes ({ $count })…
partitions-swap-file = Archivo de intercambio · { $value }
partitions-swap-file-title = Archivo de intercambio
partitions-pending-title = Operaciones pendientes
partitions-pending-short = Pendientes
partitions-pending-none = No hay operaciones pendientes
partitions-pending-earlier = { $count ->
    [one] Una operación anterior
   *[other] { $count } operaciones anteriores
}
partitions-tile-format = formatear
partitions-tile-deleted = { $count ->
    [one] { $partitions } eliminada
   *[other] { $partitions } eliminadas
}
partitions-do-format = Formatear como { $filesystem }
partitions-do-mount = Montar en { $path }
partitions-do-delete = Eliminar { $partition }
partitions-op-delete = Eliminar { $partition } ({ $filesystem }, { $size })
partitions-op-format = Formatear { $partition } como { $filesystem }
partitions-op-mount = Establecer el punto de montaje de { $partition } en { $path }
partitions-op-table = Crear tabla de particiones { $table } en { $disk }
partitions-op-create = Crear la partición primaria n.º { $number } ({ $filesystem }, { $size }) en { $disk }
partitions-op-create-at = Crear la partición primaria n.º { $number } ({ $filesystem }, { $size }, { $path }) en { $disk }
partitions-op-shrink = Reducir { $partition } de { $from } a { $to }
partitions-op-grow = Ampliar { $partition } de { $from } a { $to }
partitions-op-label = Poner la etiqueta «{ $label }» al sistema de archivos de { $partition }
partitions-op-label-clear = Quitar la etiqueta del sistema de archivos de { $partition }
partitions-missing-root = Falta una partición montada en /
partitions-missing-efi = Falta una partición EFI en /boot o /efi
partitions-missing-boot = Añada /boot o monte la partición EFI en /boot
partitions-missing-bios-grub = Falta una partición bios_grub en { $disk }
partitions-too-small = { $path } necesita al menos { $size }
partitions-must-format = Formatee { $partition } para usarla en { $path }
partitions-needs = { $path } necesita { $filesystem }
partitions-needs-linux = { $path } necesita ext4, btrfs o xfs
partitions-needs-root = { $path } necesita ext4 o btrfs
partitions-twice = { $path } está asignado a más de una partición
partitions-blocked = No se puede cambiar nada en { $disk }
partitions-refused-mismatch = El plan de { $disk } no coincide con el disco
partitions-refused-unaligned = { $partition } debe ir alineada a MiB
partitions-refused-outside = { $partition } se sale del espacio de su disco
partitions-refused-overlap = { $partition } se solapa con otra partición
partitions-refused-beside-extended = { $disk }: nada junto a la partición extendida
partitions-refused-logical = Elimine antes las lógicas tras { $partition }
partitions-refused-entries = { $disk } tendría más de 128 particiones
partitions-refused-shrink = { $partition } no se puede reducir
partitions-refused-grow = { $partition } se conserva, así que no puede crecer
partitions-refused-bios-grub = { $partition } no puede ser bios_grub aquí
partitions-refused-label = { $partition }: etiqueta no válida
partitions-refused-unmountable = { $partition }: { $filesystem } no es montable
partitions-refused-inside-kept = { $path } se conserva: no se monta nada dentro
partitions-refused-fs-small = { $partition } es demasiado pequeña para { $filesystem }
partitions-refused-restore-points = No hay puntos de restauración junto a un /var aparte
partitions-refused-boot-disk = /boot debe estar en el mismo disco que { $path }
partitions-refused-efi-bios = /efi solo se usa si UEFI arranca el ordenador
partitions-refused-grub-gap = GRUB necesita libre el primer MiB de { $disk }
partitions-refused-bootloader = { $loader } no puede arrancar Linexin desde aquí
partitions-refused-not-read = Aún no se sabe cuánto ocupa { $partition }
partitions-refused-shrink-below = { $partition } no puede ser menor que { $size }
partitions-refused-hibernated = { $partition } no se puede reducir: Windows hiberna
partitions-refused-needs-check = { $partition } no se puede reducir hasta revisarla
partitions-refused-encrypted = { $partition } está cifrada y no se puede reducir
partitions-refused-in-use = { $partition } está en uso y no se puede reducir
partitions-refused-unreadable = { $partition } no se puede leer ni reducir
partitions-refused-no-hiberfile = { $partition }: redúzcala en Windows, no aquí
partitions-refused-windows-unread = No se reduce: Windows en { $other } no se pudo leer
partitions-refused-bitlocker = No se reduce: { $other } tiene BitLocker
partitions-why-hibernated = Windows en { $other }: apáguelo sin inicio rápido
partitions-why-windows-unread = No se reduce hasta poder leer Windows en { $other }
partitions-why-bitlocker = Para reducir, desactive BitLocker en { $other }
partitions-why-no-hiberfile = Para reducirla, use Administración de discos de Windows
partitions-refused-esp-full = { $partition } necesita { $size } libres
partitions-refused-image-unknown = Aún no se sabe cuánto pone Linexin en { $path }
partitions-refused-unread = No se ha podido leer cuánto ocupa { $partition }
partitions-refused-image-unread = No se ha podido leer cuánto pone Linexin en { $path }
partitions-new = Nueva…
partitions-resize = Redimensionar…
partitions-relabel = Etiqueta…
partitions-other = Otro…
partitions-new-name = Partición nueva n.º { $number }
partitions-tile-new = nueva
partitions-new-title = Partición nueva
partitions-resize-title = Redimensionar { $partition }
partitions-label-title = Etiqueta de { $partition }
partitions-other-title = Punto de montaje de { $partition }
partitions-new-size = Tamaño nuevo
partitions-position = Posición
partitions-position-start = inicio del espacio
partitions-position-end = final del espacio
partitions-bios-grub = bios_grub (sin formato)
partitions-cancel = Cancelar
partitions-add = Añadir
partitions-resize-button = Redimensionar
partitions-set = Establecer
partitions-leaves = Deja { $size } sin asignar detrás
partitions-leaves-none = No deja espacio sin asignar detrás
partitions-takes = Toma { $size } del espacio sin asignar que la sigue
partitions-takes-all = Toma todo el espacio sin asignar que la sigue
partitions-at-least = Necesita al menos { $size }
partitions-path-placeholder = Por ejemplo, /data
partitions-path-whole = Un punto de montaje es una ruta completa desde /, como /data
partitions-path-twice = { $path } ya es un punto de montaje
partitions-path-system = { $path } pertenece al sistema
partitions-path-long = Un punto de montaje tiene 255 bytes como máximo
partitions-path-spaces = Un punto de montaje no lleva espacios ni barras invertidas
partitions-path-under = No se puede montar nada dentro de { $path }
partitions-label-bytes = Las etiquetas { $filesystem } admiten { $count ->
    [one] un byte
   *[other] { $count } bytes
} como máximo
partitions-label-chars = Las etiquetas { $filesystem } admiten { $count ->
    [one] un carácter
   *[other] { $count } caracteres
} como máximo
partitions-label-slash = Una etiqueta no puede contener «/»
partitions-why-table = Aún no hay tabla de particiones: elija este espacio para crear una
partitions-why-primaries = Un disco msdos admite cuatro particiones primarias como máximo
partitions-why-logical = Aquí no se pueden crear particiones lógicas
partitions-why-beside-extended = No se puede crear nada junto a una partición extendida
partitions-why-whole = Algo ocupa todo el disco: no se puede crear una tabla ni una partición
partitions-why-unmountable = { $filesystem } no se puede montar: formatéela para usarla
partitions-why-unformatted = Sin sistema de archivos: formatéela para usarla
partitions-why-logical-delete = Elimine antes las particiones lógicas de número mayor
partitions-primaries = { $disk } tendría más de cuatro particiones primarias
partitions-do-create = Crear
partitions-do-shrink = Reducir a { $size }
partitions-do-grow = Ampliar a { $size }

## The desktop step

button-done = Hecho
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = Basado en GNOME
desktop-about-kinexin = Basado en KDE Plasma
desktop-about-linexinbar = Pensado para un mando
# Under a desktop: whether it has to be downloaded.
desktop-ready = No necesita internet
desktop-needs-internet = Necesita internet
desktop-refused = { $desktop } necesita internet. Conéctese, o elija Linexin.
desktop-checking = Comprobando la conexión a internet…
desktop-checking-short = Comprobando internet…
desktop-offline = No hay internet, así que se omite lo que lo necesita.
desktop-updates = Instalar las actualizaciones
desktop-updates-on = Todo al día
desktop-updates-off = Tal como está en la memoria USB
updates-refused = Las actualizaciones necesitan internet. Se pueden instalar más tarde.
desktop-options-title = Opciones
desktop-options-caption = Qué más se instala y cómo arranca el ordenador.
desktop-option-remove = Software que quitar
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = Software
desktop-flatpaks-title = Aplicaciones
desktop-bootloader-title = Arranque
desktop-remove-caption = Desactive lo que no quiera en el sistema instalado.
desktop-remove-looking = Mirando el software…
desktop-remove-unlisted = No se ha podido listar el software
desktop-remove-none = No hay nada que quitar
desktop-remove-nothing = No se quita nada
desktop-remove-count = { $count ->
    [one] Se quita uno
   *[other] Se quitan { $count }
}
desktop-option-flatpaks = Aplicaciones de Flathub
desktop-flatpaks-caption = Se instalan desde internet junto con Linexin.
# How many of the apps offered are switched on.
desktop-flatpaks-count = { $count } de { $total } elegidas
desktop-flatpak-on = Se instala con Linexin
desktop-flatpak-off = Se omite
flatpaks-refused = Las aplicaciones de Flathub necesitan internet. Se pueden añadir más tarde.
desktop-option-bootloader = Gestor de arranque
desktop-bootloader-caption = Lo que inicia Linexin al encender el ordenador.
desktop-option-drivers = Quitar controladores sin usar
desktop-drivers-on = Se quitan los que no se usan
desktop-drivers-off = Se conservan todos los controladores
desktop-option-snapshots = Puntos de restauración
desktop-snapshots-on = Se guardan antes de cada actualización
desktop-snapshots-off = No se guarda ninguno
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = Se activa en «Dónde instalar»
desktop-snapshots-var = No junto a un /var aparte
bootloader-automatic = Automático
bootloader-automatic-note = Elegido para este ordenador
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = Inicia Linexin directamente, sin menú
bootloader-grub = GRUB
bootloader-grub-note = Un menú al arrancar, en cualquier ordenador
bootloader-refind = rEFInd
bootloader-refind-note = Un menú con todos los sistemas del ordenador
bootloader-unavailable = No se puede usar en este ordenador
# The groups the software to remove is listed in.
package-group-applications = Aplicaciones
package-group-multimedia = Sonido y vídeo
package-group-network = Red
package-group-fonts = Fuentes
package-group-libraries = Bibliotecas
