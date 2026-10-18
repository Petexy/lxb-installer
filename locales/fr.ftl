# Linexin Installer — French.
#
# Every message of en-GB.ftl, in French. The comments are en-GB's: they say
# where a message appears and what it means. The words the family already
# uses — Retour, Continuer, Options, Fermer, Choisir, Terminé, Installer,
# Redémarrer maintenant, Pas maintenant, ordinateur, disque — are the shell's
# and the toolkit's, and so is the polite "vous". A part of a disk is a
# "partie" and spare memory "mémoire d'appoint". Where a value whose gender
# is not known (a part, what it holds) would have to agree with a verb, the
# sentence is turned ("Ceci sera effacé :", "contenu effacé") so that it
# never has to. A name a person or a disk gave stands in French quotes,
# « », and a colon, a question mark, a percent sign and a unit stand behind
# a no-break space, as French sets them.

## The application itself

app-name = Installateur Linexin

## The head of each screen: its title, and the one line under it

welcome-caption = Quelques questions, et Linexin sera sur cet ordinateur.
language-title = Langue
language-caption = La langue que parle cet ordinateur.
timezone-title = Fuseau horaire
timezone-caption = Où vous êtes, pour que l'heure soit juste.
keyboard-title = Clavier
keyboard-caption = Les touches sur lesquelles vous tapez.
target-title = Où installer
target-caption = Le disque qui accueille Linexin.
desktop-title = Bureau
desktop-caption = L'apparence de Linexin. Chacun se personnalise ensuite.
account-title = Vous
account-caption = Votre nom, et le compte avec lequel vous vous connectez.
review-title = Prêt à installer Linexin
review-caption = Vérifiez une dernière fois. Rien n'a encore été modifié.
installing-title = Installation de Linexin
installing-caption = Cela prend un moment. Vous n'avez rien à faire.
done-title = Linexin est installé
done-caption = Redémarrez l'ordinateur et retirez la clé USB pour commencer à l'utiliser.
stopped-title = L'installation s'est arrêtée

## The verbs at the foot of a screen

button-continue = Continuer
button-install = Installer
button-close = Fermer
button-restart = Redémarrer maintenant
button-not-now = Pas maintenant
button-restart-short = Redémarrer

## What the buttons do, in the legend at the foot of the panel

hint-choose = Choisir
hint-back = Retour
hint-quit = Quitter
hint-type = Saisir
hint-shift = Maj
hint-close = Fermer
hint-done = Terminé
hint-options = Options
hint-details = Détails
hint-hide-details = Masquer les détails
hint-search = Rechercher
hint-adjust = Ajuster
hint-keep = Valider
hint-put-back = Rétablir
hint-cancel = Annuler

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = àé

## Questions, asked with the safe answer first

quit-title = Quitter l'installateur ?
quit-body = Rien n'a été modifié sur cet ordinateur.
quit-stay = Rester
quit-leave = Quitter
erase-title = Effacer et installer ?
erase-title-short = Effacer ?
install-title = Installer Linexin ?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = Tout le contenu de { $disk } sera effacé.
erase-target-part = Ceci sera effacé : { $part } · { $size } sur { $disk }.
erase-target-free = Linexin s'installe dans l'espace libre de { $disk }.
erase-home = L'emplacement de vos fichiers, { $part }, sera effacé.
erase-final = Cela ne peut pas être annulé.
erase-nothing = Rien de ce qu'il contient n'est effacé.
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = Ces opérations modifient le contenu de vos disques :
erase-plan-nothing = Rien n'est effacé sur vos disques.
erase-plan-erased = Efface { $partition } ({ $filesystem }, { $size })
erase-plan-esp = { $what } · partition système EFI
erase-plan-windows = { $what } · Windows
erase-plan-page = Page { $page } sur { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = « { $label } »
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $part } sur { $disk }
erase-back = Retour
erase-confirm = Effacer et installer
erase-confirm-short = Effacer
button-next = Suivant

## A line beside the legend

demo-note = Un ordinateur imaginaire : rien n'est réel ici, et rien n'est modifié.
install-unavailable = Démarrez cet ordinateur depuis une clé USB Linexin pour y installer Linexin.
disks-unreadable = Les disques de cet ordinateur n'ont pas pu être lus.
install-reading = Les disques de cet ordinateur sont encore en cours de lecture. Réessayez dans un instant.
installing-note = Laissez l'ordinateur allumé jusqu'à la fin.
done-all-well = Tout s'est passé comme prévu.
done-warnings = { $count ->
    [one] Linexin est prêt, mais une chose ne s'est pas passée comme prévu :
   *[other] Linexin est prêt, mais quelques points ne se sont pas passés comme prévu :
}
done-restart-failed = L'ordinateur n'a pas pu redémarrer. Redémarrez-le depuis le menu du système.

## The fields

field-full-name = Nom complet
field-full-name-placeholder = Le nom que les autres verront
field-username = Nom d'utilisateur
field-username-placeholder = Ce avec quoi vous vous connectez
field-password = Mot de passe
field-password-placeholder = Choisissez un mot de passe
field-repeat = Confirmer le mot de passe
field-repeat-placeholder = Le même mot de passe, à nouveau
field-hostname = Nom de l'ordinateur
field-hostname-placeholder = Le nom de l'ordinateur sur un réseau
field-root-password = Mot de passe administrateur
field-root-password-placeholder = Pour modifier le système
field-root-repeat = Confirmer le mot de passe administrateur
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = Faible
strength-fair = Moyen
strength-good = Bon
strength-strong = Fort

## You

# A separate administrator (root) account, off unless asked for.
account-root = Compte administrateur
account-root-off = Désactivé : votre mot de passe suffit
account-root-on = Avec son propre mot de passe, ci-dessous

## Sizes, as a box writes them

size-tb = { $amount } To
size-gb = { $amount } Go
size-mb = { $amount } Mo
size-kb = { $amount } ko
size-bytes = { $amount } octets

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = Disque SSD rapide
disk-kind-sata = Disque interne
disk-kind-usb = Disque USB
disk-kind-card = Carte mémoire
disk-kind-virtual = Disque virtuel
disk-kind-other = Disque
disk-empty = Vide
disk-holds-more = { $first } et { $count ->
    [one] une autre partie
   *[other] { $count } autres parties
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what }, sur tout le disque

## What a part of a disk appears to hold

content-efi = Fichiers de démarrage
content-bios-boot = Espace de démarrage
content-reserved = Espace réservé à Windows
content-recovery = Récupération de Windows
content-windows = Windows
content-linux = Linux
content-swap = Mémoire d'appoint
content-data = Fichiers
content-empty = Vide
content-other = Autre chose

## Why a disk cannot be chosen, said under it

blocked-installation-media = Utilisé par l'installateur
blocked-in-use = En cours d'utilisation
blocked-swap = Utilisé comme mémoire d'appoint
blocked-holder = Utilisé par autre chose sur cet ordinateur
blocked-read-only = Peut être lu, mais pas modifié
blocked-too-small = Trop petit pour Linexin
blocked-unknown = Inutilisable

## The review

# Each row names a step; under it, that step's answer.
review-where = Emplacement
review-desktop = Bureau
review-region-name = Langue et région
review-account-name = Compte
review-extras = En plus
review-where-disk = { $disk } · tout son contenu est effacé
review-where-free = { $disk } · dans son espace libre
review-where-part = { $disk } · à la place de { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = vos fichiers sur { $part }, contenu conservé
review-where-home-erased = vos fichiers sur { $part }, contenu effacé
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = Pas encore choisi
review-extra-updates = Mises à jour
review-extra-apps = { $count ->
    [one] Une application
   *[other] { $count } applications
}
review-extra-snapshots = Points de restauration
review-extra-swap = { $size } de mémoire d'appoint
review-extra-none = Rien de plus
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size } sur { $disk }
review-part-formatted = { $partition } · { $filesystem } · { $size } · formatée
review-part-kept = { $partition } · { $filesystem } · { $size } · conservée
review-operations = { $count ->
    [one] Une opération sera appliquée
   *[other] { $count } opérations seront appliquées
}

## The installation, while it runs

installing-waiting = Préparation…
installing-count = Étape { $step } sur { $steps } · { $percent } %
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = Arrêtée à l'étape : { $step }
step-check = Vérification de l'ordinateur
step-resize = Réduction de ce qui reste sur le disque
step-partition = Création d'espace sur le disque
step-format = Préparation du disque
step-copy = Copie de Linexin
step-swap = Mise en place de la mémoire d'appoint
step-system = Configuration du système
step-region = Réglage de la langue, du fuseau horaire et du clavier
step-accounts = Création de votre compte
step-desktop = Configuration du bureau
step-updates = Installation des mises à jour
step-packages = Ajout et retrait de logiciels
step-flatpaks = Installation des applications
step-drivers = Retrait des pilotes dont cet ordinateur n'a pas besoin
step-snapshots = Mise en place des points de restauration
step-initramfs = Préparation du démarrage du système
step-bootloader = Réglage de l'ordinateur pour qu'il démarre Linexin
step-finish = Finalisation

## What did not work, said once Linexin is installed

warning-updates-offline = Les mises à jour ont été laissées de côté, faute d'Internet. Elles pourront être installées plus tard.
warning-updates-failed = Les mises à jour n'ont pas pu être installées. Elles pourront l'être plus tard.
warning-flatpaks-failed = Certaines applications n'ont pas pu être installées. Elles pourront être ajoutées plus tard.
warning-packages-kept = Certains logiciels qui devaient être retirés sont toujours là.
warning-boot-entry = Si Linexin ne démarre pas, choisissez-le dans le menu de démarrage de l'ordinateur.
warning-snapshots = Les points de restauration n'ont pas pu être mis en place.
warning-drivers-kept = Les pilotes dont cet ordinateur n'a pas besoin ont été laissés en place.
warning-windows-check = Laissez Windows vérifier son disque au prochain démarrage. D'ici là, Linexin n'y touche pas.

## Why the installation stopped, and what can be done about it

failure-invalid-request = L'installateur a demandé quelque chose d'impossible
failure-invalid-request-do = Fermez l'installateur, rouvrez-le et refaites les étapes.
failure-consent-mismatch = Ce qui devait être effacé a changé
failure-consent-mismatch-do = Rien n'a été effacé. Refaites les étapes et vérifiez le disque.
failure-not-root = L'installateur n'a pas reçu l'autorisation
failure-not-root-do = Rouvrez l'installateur et autorisez-le quand on vous le demande.
failure-not-live = Cet ordinateur n'a pas démarré depuis l'installateur
failure-not-live-do = Démarrez l'ordinateur depuis la clé USB Linexin et réessayez.
failure-secure-boot = Secure Boot est activé
failure-secure-boot-do = Désactivez Secure Boot dans les réglages de démarrage de l'ordinateur, puis réessayez.
failure-already-running = Une autre installation est déjà en cours
failure-already-running-do = Attendez qu'elle se termine, ou redémarrez l'ordinateur et réessayez.
failure-missing-tool = Il manque quelque chose dont l'installateur a besoin
failure-missing-tool-do = Utilisez une clé USB Linexin plus récente et réessayez.
failure-image-invalid = La copie de Linexin sur la clé USB est abîmée
failure-image-invalid-do = Réécrivez la clé USB et réessayez.
failure-disk-changed = Le disque a changé après avoir été choisi
failure-disk-changed-do = Une partie du disque est peut-être déjà effacée. Refaites les étapes et choisissez à nouveau le disque.
failure-disk-changed-untouched-do = Rien n'a été effacé. Refaites les étapes et choisissez à nouveau le disque.
failure-disk-busy = Le disque est utilisé par autre chose
failure-disk-busy-do = Fermez ce qui l'utilise, ou redémarrez l'ordinateur, et réessayez.
failure-disk-busy-untouched-do = Rien n'a été effacé. Fermez ce qui utilise le disque, ou redémarrez l'ordinateur, et réessayez.
failure-no-space = Il n'y a pas assez de place
failure-no-space-do = Choisissez un disque plus grand ou plus d'espace libre, et réessayez.
failure-esp-full = L'espace de démarrage du disque est plein
failure-esp-full-do = Effacez plutôt tout le disque, ou libérez d'abord de la place dans son espace de démarrage.
failure-shrink-refused = Une partie du disque n'a pas pu être réduite
failure-shrink-refused-do = Toutes les parties du disque sont toujours là, mais celle qui était réduite a pu changer. Laissez le système auquel elle appartient la vérifier, puis réessayez.
failure-shrink-refused-untouched-do = Rien n'a été modifié. Rouvrez l'installateur et laissez plus de place à cette partie, ou réduisez-la depuis le système auquel elle appartient.
failure-windows-hibernated = Windows n'est pas complètement éteint
failure-windows-hibernated-do = Rien n'a été modifié. Démarrez Windows, désactivez le démarrage rapide, éteignez-le, puis réessayez.
failure-needs-check = Une partie du disque doit d'abord être vérifiée
failure-needs-check-do = Toutes les parties du disque sont toujours là, mais celle qui était réduite a pu changer. Démarrez le système auquel elle appartient, laissez-le vérifier son disque, puis réessayez.
failure-needs-check-untouched-do = Rien n'a été modifié. Démarrez le système auquel cette partie appartient, laissez-le vérifier son disque, puis réessayez.
failure-windows-in-use = Une partie d'un disque avec Windows est utilisée
failure-windows-in-use-do = Rien n'a été modifié. Fermez ce qui a ouvert des fichiers de Windows, ou redémarrez l'ordinateur, puis réessayez.
failure-windows-needs-check = Windows doit d'abord vérifier son disque
failure-windows-needs-check-do = Rien n'a été modifié. Démarrez Windows, laissez-le vérifier son disque, éteignez-le, puis réessayez.
failure-windows-unread = Une partie d'un disque avec Windows n'a pas pu être lue
failure-windows-unread-do = Rien n'a été modifié. Débranchez les disques sur lesquels vous n'installez pas et réessayez, ou réduisez Windows depuis Windows lui-même.
failure-bitlocker = Une partie d'un disque est verrouillée par BitLocker
failure-bitlocker-do = Rien n'a été modifié. Désactivez BitLocker dans Windows et réessayez, ou réduisez Windows depuis Windows lui-même.
failure-needs-internet = Ce choix nécessite Internet
failure-needs-internet-do = Connectez-vous à Internet, ou choisissez le bureau Linexin, qui n'en a pas besoin.
failure-needs-internet-untouched-do = Rien n'a été effacé. Connectez-vous à Internet, ou choisissez le bureau Linexin, qui n'en a pas besoin.
failure-command = Un problème est survenu pendant l'installation
failure-command-do = Consultez les détails, puis réessayez. Le disque devra peut-être être effacé.
failure-internal = L'installateur a rencontré un problème
failure-internal-do = Fermez-le et réessayez. Si cela se reproduit, gardez les détails pour signaler le problème.

## Why a choice cannot be installed, said beside it

invalid-username-empty = Choisissez un nom d'utilisateur
invalid-username-shape = Minuscules, chiffres, - et _, en commençant par une lettre
invalid-username-reserved = Ce nom est réservé au système
invalid-full-name = Un nom ne peut pas contenir : ou ,
invalid-hostname = Lettres, chiffres et -, 63 au maximum
invalid-password-empty = Choisissez un mot de passe
invalid-password-mismatch = Les deux mots de passe ne sont pas identiques
invalid-password-shape = 128 caractères au maximum, sans retour à la ligne
invalid-root-password-empty = Choisissez un mot de passe pour le compte administrateur
invalid-locale = Choisissez une langue
invalid-timezone = Choisissez un fuseau horaire
invalid-keyboard = Choisissez un clavier
invalid-disk-blocked = Ce disque ne peut pas être utilisé
invalid-target-missing = Choisissez où installer
invalid-target-small = Il n'y a pas assez de place pour Linexin à cet endroit
table-full = Ce disque n'a plus de place pour une autre partie. Remplacez-en une.
invalid-target-esp = Cette partie sert à démarrer l'ordinateur
target-protected = Windows garde cette partie pour lui
invalid-home = Cette partie ne peut pas accueillir vos fichiers
invalid-swap = La mémoire d'appoint est plus grande que la place disponible
invalid-bootloader = Ce gestionnaire de démarrage ne peut pas servir sur cet ordinateur
invalid-source = La copie de Linexin à installer est introuvable
invalid-layout = Vos partitions ne peuvent pas être utilisées ainsi

## The time zone step

timezone-heading-region = Région du monde
timezone-heading-city = Ville la plus proche
timezone-heading-found = Lieux trouvés
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] Un lieu
   *[other] { $count } lieux
}
zone-region-africa = Afrique
zone-region-america = Amériques
zone-region-antarctica = Antarctique
zone-region-arctic = Arctique
zone-region-asia = Asie
zone-region-atlantic = Océan Atlantique
zone-region-australia = Australie
zone-region-europe = Europe
zone-region-indian = Océan Indien
zone-region-pacific = Océan Pacifique
zone-region-utc = Temps universel
# The one place under Universal time: the clock no country keeps.
zone-utc = Temps universel coordonné

## The keyboard step

keyboard-heading-layout = Disposition
keyboard-heading-variant = Variante
keyboard-heading-found = Variantes trouvées
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] Standard uniquement
    [one] Standard et une autre
   *[other] Standard et { $count } autres
}
# Under the plain layout, first among its kinds.
keyboard-plain = Standard
keyboard-test = Essayez votre clavier
keyboard-test-placeholder = Tapez ici pour essayer les touches

## The search over a long list (Language, Time zone, Keyboard)

search-field = Recherche
search-language-placeholder = Une langue, aussi en anglais
search-timezone-placeholder = Ville, pays ou partie du monde
search-keyboard-placeholder = Une disposition ou une variante
search-nothing = Rien trouvé pour « { $query } »

## The step of where to install

button-options = Options
target-heading-disks = Disques
target-erase = Effacer ce disque
target-erase-note = Tout son contenu est effacé
target-free = Espace libre · { $size }
target-free-note = Rien n'est effacé sur le disque
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = Remplacer { $what } · { $size }
target-replace-note = Son contenu est effacé
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = Remplacer « { $label } » · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · contenu effacé
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = « { $label } » · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = Espace libre
target-no-disk = Il n'y a aucun disque sur lequel installer Linexin.
target-options = Options
# Each option with the state it is in.
target-option-storage = Stockage · { $value }
target-option-swap = Mémoire d'appoint · { $value }
target-option-home = Vos fichiers · { $value }
target-storage = Stockage
target-storage-standard = Standard
target-storage-snapshots = Avec points de restauration
target-storage-snapshots-short = Points de restauration
target-swap = Mémoire d'appoint
target-swap-off = Désactivée
target-home = Vos fichiers
target-home-system = Avec le système
target-home-kept = { $part }, contenu conservé
target-home-erased = { $part }, contenu effacé
target-home-keep = Garder son contenu
target-home-erase = Effacer son contenu
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = Vos partitions sont utilisées · { $count ->
    [one] une opération
   *[other] { $count } opérations
}
target-partitions-let-go = Un disque a changé, vos partitions ont donc été abandonnées. Choisissez à nouveau.

## Partitions

button-advanced = Avancé
partitions-title = Partitions
partitions-not-yet = Les partitions ne peuvent pas encore être appliquées dans cette version.
partitions-not-yet-fresh-table = Une nouvelle table de partitions ne peut pas encore être appliquée dans cette version.
partitions-not-yet-keep-table = Créer ou supprimer des partitions ne peut pas encore être appliqué dans cette version.
partitions-not-yet-change-entries = Formater une partition ne peut pas encore être appliqué dans cette version.
partitions-not-yet-kept-home = Conserver un /home ne peut pas encore être appliqué dans cette version.
partitions-not-yet-several-disks = Un plan sur plusieurs disques ne peut pas encore être appliqué dans cette version.
partitions-not-yet-bios = Partitionner un ordinateur à BIOS n'est pas encore possible dans cette version.
partitions-not-yet-shrink = Réduire une partition ne peut pas encore être appliqué dans cette version.
partitions-unallocated = non alloué
partitions-unknown = inconnu
partitions-no-table = aucune table de partitions
partitions-partition = Partition
partitions-filesystem = Système de fichiers
partitions-label = Étiquette
partitions-size = Taille
partitions-used = Utilisé
partitions-mount-point = Point de montage
partitions-flags = Drapeaux
partitions-pending = En attente
partitions-start = Début
partitions-end = Fin
partitions-reading = Lecture…
partitions-menu-mount = Point de montage · { $value }
partitions-menu-format = Formater en · { $value }
partitions-format-as = Formater en
partitions-none = aucun
partitions-keep = conserver
partitions-delete = Supprimer
partitions-undo-part = Annuler les modifications de { $partition }
partitions-undo = Annuler les modifications
partitions-create-table = Créer une table de partitions
partitions-create-table-kind = Créer une table de partitions { $table }
partitions-table = Table de partitions
partitions-undo-last = Annuler la dernière opération
partitions-clear = Effacer toutes les opérations
partitions-pending-list = Opérations en attente ({ $count })…
partitions-swap-file = Fichier d'échange · { $value }
partitions-swap-file-title = Fichier d'échange
partitions-pending-title = Opérations en attente
partitions-pending-short = En attente
partitions-pending-none = Aucune opération en attente
partitions-pending-earlier = { $count ->
    [one] Une opération antérieure
   *[other] { $count } opérations antérieures
}
partitions-tile-format = formater
partitions-tile-deleted = { $count ->
    [one] { $partitions } supprimée
   *[other] { $partitions } supprimées
}
partitions-do-format = Formater en { $filesystem }
partitions-do-mount = Monter sur { $path }
partitions-do-delete = Supprimer { $partition }
partitions-op-delete = Supprimer { $partition } ({ $filesystem }, { $size })
partitions-op-format = Formater { $partition } en { $filesystem }
partitions-op-mount = Définir le point de montage de { $partition } sur { $path }
partitions-op-table = Créer une table de partitions { $table } sur { $disk }
partitions-op-create = Créer la partition primaire nº { $number } ({ $filesystem }, { $size }) sur { $disk }
partitions-op-create-at = Créer la partition primaire nº { $number } ({ $filesystem }, { $size }, { $path }) sur { $disk }
partitions-op-shrink = Réduire { $partition } de { $from } à { $to }
partitions-op-grow = Agrandir { $partition } de { $from } à { $to }
partitions-op-label = Définir l'étiquette « { $label } » du système de fichiers de { $partition }
partitions-op-label-clear = Effacer l'étiquette du système de fichiers de { $partition }
partitions-missing-root = Il manque une partition montée sur /
partitions-missing-efi = Il manque une partition EFI sur /boot ou /efi
partitions-missing-boot = Ajoutez /boot, ou montez la partition EFI sur /boot
partitions-missing-bios-grub = Il manque une partition bios_grub sur { $disk }
partitions-too-small = { $path } doit faire au moins { $size }
partitions-must-format = Formatez { $partition } pour l'utiliser comme { $path }
partitions-needs = { $path } nécessite { $filesystem }
partitions-needs-linux = { $path } nécessite ext4, btrfs ou xfs
partitions-needs-root = { $path } nécessite ext4 ou btrfs
partitions-twice = { $path } est attribué à plus d'une partition
partitions-blocked = Rien ne peut être modifié sur { $disk }
partitions-refused-mismatch = Le plan pour { $disk } ne correspond pas au disque
partitions-refused-unaligned = { $partition } doit être alignée sur le MiB
partitions-refused-outside = { $partition } dépasse l'espace de son disque
partitions-refused-overlap = { $partition } chevauche une autre partition
partitions-refused-beside-extended = { $disk } : rien à côté de la partition étendue
partitions-refused-logical = Supprimez d'abord les logiques après { $partition }
partitions-refused-entries = { $disk } aurait plus de 128 partitions
partitions-refused-shrink = { $partition } ne peut pas être réduite
partitions-refused-grow = { $partition } est conservée et ne peut pas grandir
partitions-refused-bios-grub = { $partition } ne peut pas être bios_grub ici
partitions-refused-label = { $partition } : étiquette non valide
partitions-refused-unmountable = { $partition } : { $filesystem } non montable
partitions-refused-inside-kept = { $path } est conservé : rien ne s'y monte
partitions-refused-fs-small = { $partition } est trop petite pour { $filesystem }
partitions-refused-restore-points = Pas de points de restauration avec un /var séparé
partitions-refused-boot-disk = /boot doit être sur le même disque que { $path }
partitions-refused-efi-bios = /efi ne sert que si l'UEFI démarre l'ordinateur
partitions-refused-grub-gap = GRUB a besoin du premier MiB de { $disk } libre
partitions-refused-bootloader = { $loader } ne peut pas démarrer Linexin ici
partitions-refused-not-read = Espace utilisé sur { $partition } encore inconnu
partitions-refused-shrink-below = { $partition } ne peut pas faire moins de { $size }
partitions-refused-hibernated = { $partition } non réductible : Windows en veille
partitions-refused-needs-check = { $partition } : à vérifier avant réduction
partitions-refused-encrypted = { $partition } est chiffrée et ne peut être réduite
partitions-refused-in-use = { $partition } est utilisée et ne peut être réduite
partitions-refused-unreadable = { $partition } est illisible et ne peut être réduite
partitions-refused-no-hiberfile = { $partition } : à réduire dans Windows, pas ici
partitions-refused-windows-unread = Pas de réduction : Windows sur { $other } illisible
partitions-refused-bitlocker = Pas de réduction : { $other } verrouillée par BitLocker
partitions-why-hibernated = Windows sur { $other } : éteignez-le sans démarrage rapide
partitions-why-windows-unread = Réductible quand Windows sur { $other } sera lisible
partitions-why-bitlocker = Pour réduire, désactivez BitLocker sur { $other }
partitions-why-no-hiberfile = Pour la réduire, utilisez la Gestion des disques de Windows
partitions-refused-esp-full = { $partition } doit avoir { $size } libres
partitions-refused-image-unknown = Taille de Linexin dans { $path } encore inconnue
partitions-refused-unread = Espace utilisé sur { $partition } illisible
partitions-refused-image-unread = Taille de Linexin dans { $path } illisible
partitions-new = Nouvelle…
partitions-resize = Redimensionner…
partitions-relabel = Étiquette…
partitions-other = Autre…
partitions-new-name = Nouvelle partition #{ $number }
partitions-tile-new = nouvelle
partitions-new-title = Nouvelle partition
partitions-resize-title = Redimensionner { $partition }
partitions-label-title = Étiquette de { $partition }
partitions-other-title = Point de montage de { $partition }
partitions-new-size = Nouvelle taille
partitions-position = Position
partitions-position-start = début de l'espace
partitions-position-end = fin de l'espace
partitions-bios-grub = bios_grub (non formatée)
partitions-cancel = Annuler
partitions-add = Ajouter
partitions-resize-button = Redimensionner
partitions-set = Définir
partitions-leaves = Laisse { $size } non alloués après elle
partitions-leaves-none = Ne laisse aucun espace non alloué après elle
partitions-takes = Prend { $size } de l'espace non alloué qui la suit
partitions-takes-all = Prend tout l'espace non alloué qui la suit
partitions-at-least = Il faut au moins { $size }
partitions-path-placeholder = Par exemple /data
partitions-path-whole = Un point de montage est un chemin complet depuis /, comme /data
partitions-path-twice = { $path } est déjà un point de montage
partitions-path-system = { $path } appartient au système
partitions-path-long = Un point de montage fait au plus 255 octets
partitions-path-spaces = Un point de montage ne contient ni espaces ni barres obliques inverses
partitions-path-under = Rien ne peut être monté dans { $path }
partitions-label-bytes = Une étiquette { $filesystem } fait au plus { $count ->
    [one] un octet
   *[other] { $count } octets
}
partitions-label-chars = Une étiquette { $filesystem } fait au plus { $count ->
    [one] un caractère
   *[other] { $count } caractères
}
partitions-label-slash = Une étiquette ne peut pas contenir « / »
partitions-why-table = Aucune table : choisissez cet espace pour en créer une
partitions-why-primaries = Un disque msdos contient au plus quatre partitions primaires
partitions-why-logical = Impossible de créer des partitions logiques ici
partitions-why-beside-extended = Rien ne peut être créé à côté d'une partition étendue
partitions-why-whole = Quelque chose occupe tout le disque : ni table ni partition possible
partitions-why-unmountable = { $filesystem } ne peut pas être monté : formatez-la pour l'utiliser
partitions-why-unformatted = Aucun système de fichiers : formatez-la pour l'utiliser
partitions-why-logical-delete = Supprimez d'abord les partitions logiques de numéro supérieur
partitions-primaries = { $disk } aurait plus de quatre partitions primaires
partitions-do-create = Créer
partitions-do-shrink = Réduire à { $size }
partitions-do-grow = Agrandir à { $size }

## The desktop step

button-done = Terminé
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = Basé sur GNOME
desktop-about-kinexin = Basé sur KDE Plasma
desktop-about-linexinbar = Conçu pour une manette
# Under a desktop: whether it has to be downloaded.
desktop-ready = Pas besoin d'Internet
desktop-needs-internet = Nécessite Internet
desktop-refused = { $desktop } nécessite Internet. Connectez-vous, ou choisissez Linexin.
desktop-checking = Vérification de la connexion à Internet…
desktop-checking-short = Vérification d'Internet…
desktop-offline = Pas d'Internet : ce qui en a besoin est laissé de côté.
desktop-updates = Installer les mises à jour
desktop-updates-on = Tout est mis à jour
desktop-updates-off = Tel que sur la clé USB
updates-refused = Les mises à jour nécessitent Internet. Elles pourront être installées plus tard.
desktop-options-title = Options
desktop-options-caption = Ce qui est installé en plus, et comment l'ordinateur démarre.
desktop-option-remove = Logiciels à retirer
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = Logiciels
desktop-flatpaks-title = Applications
desktop-bootloader-title = Démarrage
desktop-remove-caption = Désactivez ce que vous ne voulez pas sur le système installé.
desktop-remove-looking = Recherche des logiciels…
desktop-remove-unlisted = Les logiciels n'ont pas pu être listés
desktop-remove-none = Il n'y a rien à retirer
desktop-remove-nothing = Rien n'est retiré
desktop-remove-count = { $count ->
    [one] Un retiré
   *[other] { $count } retirés
}
desktop-option-flatpaks = Applications de Flathub
desktop-flatpaks-caption = Installées depuis Internet en même temps que Linexin.
# How many of the apps offered are switched on.
desktop-flatpaks-count = { $count } sur { $total } choisies
desktop-flatpak-on = Installée avec Linexin
desktop-flatpak-off = Laissée de côté
flatpaks-refused = Les applications de Flathub nécessitent Internet. Elles pourront être ajoutées plus tard.
desktop-option-bootloader = Gestionnaire de démarrage
desktop-bootloader-caption = Ce qui lance Linexin quand l'ordinateur s'allume.
desktop-option-drivers = Retirer les pilotes inutiles
desktop-drivers-on = Les pilotes inutiles sont retirés
desktop-drivers-off = Tous les pilotes sont gardés
desktop-option-snapshots = Points de restauration
desktop-snapshots-on = Créés avant chaque mise à jour
desktop-snapshots-off = Aucun n'est créé
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = À activer dans « Où installer »
desktop-snapshots-var = Pas avec un /var séparé
bootloader-automatic = Automatique
bootloader-automatic-note = Choisi pour convenir à cet ordinateur
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = Lance Linexin directement, sans menu
bootloader-grub = GRUB
bootloader-grub-note = Un menu au démarrage, sur tout ordinateur
bootloader-refind = rEFInd
bootloader-refind-note = Un menu de tous les systèmes de l'ordinateur
bootloader-unavailable = Inutilisable sur cet ordinateur
# The groups the software to remove is listed in.
package-group-applications = Applications
package-group-multimedia = Son et vidéo
package-group-network = Réseau
package-group-fonts = Polices
package-group-libraries = Bibliothèques
