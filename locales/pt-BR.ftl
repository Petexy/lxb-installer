# Linexin Installer — Portuguese (Brazil).
#
# Every message of en-GB.ftl, in Brazilian Portuguese. The comments are
# en-GB's: they say where a message appears and what it means. The words the
# family already uses — Voltar, Continuar, Opções, Fechar, Escolher,
# Concluído, Instalar, Reiniciar agora, Agora não, Senha, Nome de usuário,
# Programas, computador, disco, "tente de novo" — are the shell's and the
# toolkit's, and so is "você". A part of a disk is a "parte", spare memory
# "memória extra" (as the shell's Storage page says it) and a USB stick a
# "pendrive". Where a value whose gender or number is not known (a part,
# what it holds) would have to agree with a verb or an adjective, the
# sentence is turned ("Tudo em", "conteúdo apagado") so that it never has
# to. A name a person or a disk gave stands in “ ”, as the shell sets it.

## The application itself

app-name = Instalador do Linexin

## The head of each screen: its title, and the one line under it

welcome-caption = Algumas perguntas, e o Linexin estará neste computador.
language-title = Idioma
language-caption = O idioma que este computador fala.
timezone-title = Fuso horário
timezone-caption = Onde você está, para o relógio ficar certo.
keyboard-title = Teclado
keyboard-caption = As teclas em que você digita.
target-title = Onde instalar
target-caption = O disco em que o Linexin vai ficar.
desktop-title = Área de trabalho
desktop-caption = A aparência do Linexin. Cada uma pode ser personalizada depois.
account-title = Você
account-caption = Seu nome e a conta com que você entra.
review-title = Pronto para instalar o Linexin
review-caption = Confira tudo. Nada foi alterado ainda.
installing-title = Instalando o Linexin
installing-caption = Isto leva um tempo. Não é preciso apertar nada.
done-title = O Linexin está instalado
done-caption = Reinicie o computador e tire o pendrive para começar a usá-lo.
stopped-title = A instalação parou

## The verbs at the foot of a screen

button-continue = Continuar
button-install = Instalar
button-close = Fechar
button-restart = Reiniciar agora
button-not-now = Agora não
button-restart-short = Reiniciar

## What the buttons do, in the legend at the foot of the panel

hint-choose = Escolher
hint-back = Voltar
hint-quit = Sair
hint-type = Digitar
hint-shift = Shift
hint-close = Fechar
hint-done = Concluído
hint-options = Opções
hint-details = Detalhes
hint-hide-details = Ocultar detalhes
hint-search = Buscar
hint-adjust = Ajustar
hint-keep = Confirmar
hint-put-back = Restaurar
hint-cancel = Cancelar

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = ãç

## Questions, asked with the safe answer first

quit-title = Sair do instalador?
quit-body = Nada foi alterado neste computador.
quit-stay = Ficar
quit-leave = Sair
erase-title = Apagar e instalar?
erase-title-short = Apagar?
install-title = Instalar o Linexin?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = Tudo em { $disk } será apagado.
erase-target-part = Tudo em { $part } · { $size }, de { $disk }, será apagado.
erase-target-free = O Linexin será instalado no espaço livre de { $disk }.
erase-home = Tudo em { $part }, onde seus arquivos vão ficar, será apagado.
erase-final = Isto não pode ser desfeito.
erase-nothing = Nada do que há nele será apagado.
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = Estas operações mudam o que há nos seus discos:
erase-plan-nothing = Nada nos seus discos será apagado.
erase-plan-erased = Apaga { $partition } ({ $filesystem }, { $size })
erase-plan-esp = { $what } · partição de sistema EFI
erase-plan-windows = { $what } · Windows
erase-plan-page = Página { $page } de { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = “{ $label }”
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $part } em { $disk }
erase-back = Voltar
erase-confirm = Apagar e instalar
erase-confirm-short = Apagar
button-next = Próximo

## A line beside the legend

demo-note = Um computador fictício: nada aqui é real, e nada é alterado.
install-unavailable = Para instalar o Linexin, inicie este computador por um pendrive do Linexin.
disks-unreadable = Não foi possível ler os discos deste computador.
install-reading = Os discos deste computador ainda estão sendo lidos. Tente de novo em instantes.
installing-note = Mantenha o computador ligado até terminar.
done-all-well = Tudo saiu como planejado.
done-warnings = { $count ->
    [one] O Linexin está pronto para usar, mas uma coisa não saiu como planejado:
   *[other] O Linexin está pronto para usar, mas algumas coisas não saíram como planejado:
}
done-restart-failed = Não foi possível reiniciar o computador. Reinicie pelo menu do sistema.

## The fields

field-full-name = Nome completo
field-full-name-placeholder = Seu nome, como os outros vão ver
field-username = Nome de usuário
field-username-placeholder = O que você usa para entrar
field-password = Senha
field-password-placeholder = Escolha uma senha
field-repeat = Confirmar senha
field-repeat-placeholder = A mesma senha de novo
field-hostname = Nome do computador
field-hostname-placeholder = O nome deste computador na rede
field-root-password = Senha do administrador
field-root-password-placeholder = Senha para mudanças no sistema
field-root-repeat = Confirmar senha do administrador
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = Fraca
strength-fair = Razoável
strength-good = Boa
strength-strong = Forte

## You

# A separate administrator (root) account, off unless asked for.
account-root = Conta de administrador
account-root-off = Desligada: sua senha autoriza mudanças
account-root-on = Com uma senha própria, abaixo

## Sizes, as a box writes them

size-tb = { $amount } TB
size-gb = { $amount } GB
size-mb = { $amount } MB
size-kb = { $amount } kB
size-bytes = { $amount } bytes

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = SSD rápido
disk-kind-sata = Disco interno
disk-kind-usb = Unidade USB
disk-kind-card = Cartão de memória
disk-kind-virtual = Disco virtual
disk-kind-other = Disco
disk-empty = Vazio
disk-holds-more = { $first } e { $count ->
    [one] mais uma parte
   *[other] mais { $count } partes
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what }, ocupando o disco todo

## What a part of a disk appears to hold

content-efi = Arquivos de inicialização
content-bios-boot = Espaço de inicialização
content-reserved = Espaço reservado do Windows
content-recovery = Recuperação do Windows
content-windows = Windows
content-linux = Linux
content-swap = Memória extra
content-data = Arquivos
content-empty = Vazio
content-other = Outra coisa

## Why a disk cannot be chosen, said under it

blocked-installation-media = O instalador roda a partir dele
blocked-in-use = Em uso agora
blocked-swap = Em uso como memória extra
blocked-holder = Usado por outra coisa neste computador
blocked-read-only = Pode ser lido, mas não gravado
blocked-too-small = Pequeno demais para o Linexin
blocked-unknown = Não pode ser usado

## The review

# Each row names a step; under it, that step's answer.
review-where = Onde
review-desktop = Área de trabalho
review-region-name = Idioma e região
review-account-name = Conta
review-extras = Extras
review-where-disk = { $disk } · tudo nele é apagado
review-where-free = { $disk } · no espaço livre
review-where-part = { $disk } · no lugar de { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = seus arquivos em { $part }, conteúdo mantido
review-where-home-erased = seus arquivos em { $part }, conteúdo apagado
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = Nada escolhido ainda
review-extra-updates = Atualizações
review-extra-apps = { $count ->
    [one] Um aplicativo
   *[other] { $count } aplicativos
}
review-extra-snapshots = Pontos de restauração
review-extra-swap = { $size } de memória extra
review-extra-none = Nada mais
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size } em { $disk }
review-part-formatted = { $partition } · { $filesystem } · { $size } · formatada
review-part-kept = { $partition } · { $filesystem } · { $size } · mantida
review-operations = { $count ->
    [one] Uma operação será aplicada
   *[other] { $count } operações serão aplicadas
}

## The installation, while it runs

installing-waiting = Preparando…
installing-count = Etapa { $step } de { $steps } · { $percent }%
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = Onde parou: { $step }
step-check = Verificando o computador
step-resize = Reduzindo o que fica no disco
step-partition = Abrindo espaço no disco
step-format = Preparando o disco
step-copy = Copiando o Linexin
step-swap = Reservando memória extra
step-system = Configurando o sistema
step-region = Configurando idioma, fuso horário e teclado
step-accounts = Criando sua conta
step-desktop = Configurando a área de trabalho
step-updates = Instalando atualizações
step-packages = Adicionando e removendo programas
step-flatpaks = Instalando aplicativos
step-drivers = Removendo drivers de que este computador não precisa
step-snapshots = Configurando os pontos de restauração
step-initramfs = Preparando o sistema para iniciar
step-bootloader = Fazendo o computador iniciar o Linexin
step-finish = Finalizando

## What did not work, said once Linexin is installed

warning-updates-offline = As atualizações ficaram de fora porque não havia internet. Elas podem ser instaladas depois.
warning-updates-failed = Não foi possível instalar as atualizações. Elas podem ser instaladas depois.
warning-flatpaks-failed = Alguns aplicativos não puderam ser instalados. Eles podem ser adicionados depois.
warning-packages-kept = Alguns programas que seriam removidos continuam lá.
warning-boot-entry = Se o Linexin não iniciar, escolha-o no menu de inicialização do computador.
warning-snapshots = Não foi possível configurar os pontos de restauração.
warning-drivers-kept = Os drivers de que este computador não precisa foram mantidos.
warning-windows-check = O Windows vai verificar o disco ao iniciar. Deixe-o terminar: até lá, o Linexin não mexe nos arquivos dele.

## Why the installation stopped, and what can be done about it

failure-invalid-request = O instalador pediu algo que não consegue fazer
failure-invalid-request-do = Feche o instalador, abra-o de novo e passe pelas etapas mais uma vez.
failure-consent-mismatch = O que ia ser apagado mudou
failure-consent-mismatch-do = Nada foi apagado. Passe pelas etapas de novo e confira o disco.
failure-not-root = O instalador não recebeu permissão
failure-not-root-do = Abra o instalador de novo e permita quando for pedido.
failure-not-live = Este computador não foi iniciado pelo instalador
failure-not-live-do = Inicie o computador pelo pendrive do Linexin e tente de novo.
failure-secure-boot = O Secure Boot está ligado
failure-secure-boot-do = Desligue o Secure Boot nas configurações de inicialização do computador e tente de novo.
failure-already-running = Outra instalação já está em andamento
failure-already-running-do = Espere ela terminar, ou reinicie o computador e tente de novo.
failure-missing-tool = Falta algo de que o instalador precisa
failure-missing-tool-do = Use um pendrive do Linexin mais recente e tente de novo.
failure-image-invalid = A cópia do Linexin no pendrive está danificada
failure-image-invalid-do = Grave o pendrive de novo e tente mais uma vez.
failure-disk-changed = O disco mudou depois de ser escolhido
failure-disk-changed-do = Parte do disco pode já ter sido apagada. Passe pelas etapas de novo e escolha o disco mais uma vez.
failure-disk-changed-untouched-do = Nada foi apagado. Passe pelas etapas de novo e escolha o disco mais uma vez.
failure-disk-busy = O disco está sendo usado por outra coisa
failure-disk-busy-do = Feche o que estiver usando o disco, ou reinicie o computador, e tente de novo.
failure-disk-busy-untouched-do = Nada foi apagado. Feche o que estiver usando o disco, ou reinicie o computador, e tente de novo.
failure-no-space = Não há espaço suficiente
failure-no-space-do = Escolha um disco maior ou mais espaço livre e tente de novo.
failure-esp-full = O espaço de inicialização do disco está cheio
failure-esp-full-do = Em vez disso, apague o disco inteiro, ou antes libere o espaço de inicialização dele.
failure-shrink-refused = Não foi possível reduzir uma parte do disco
failure-shrink-refused-do = Todas as partes do disco continuam lá, mas a que estava sendo reduzida pode ter mudado. Deixe o sistema a que ela pertence verificá-la e tente de novo.
failure-shrink-refused-untouched-do = Nada foi alterado. Abra o instalador de novo e deixe mais espaço para essa parte, ou reduza-a pelo sistema a que ela pertence.
failure-windows-hibernated = O Windows não está totalmente desligado
failure-windows-hibernated-do = Nada foi alterado. Inicie o Windows, desative a inicialização rápida, desligue-o e tente de novo.
failure-needs-check = Uma parte do disco precisa ser verificada antes
failure-needs-check-do = Todas as partes do disco continuam lá, mas a que estava sendo reduzida pode ter mudado. Inicie o sistema a que ela pertence, deixe-o verificar o disco e tente de novo.
failure-needs-check-untouched-do = Nada foi alterado. Inicie o sistema a que essa parte pertence, deixe-o verificar o disco e tente de novo.
failure-windows-in-use = Uma parte de um disco com o Windows está em uso
failure-windows-in-use-do = Nada foi alterado. Feche o que estiver com arquivos do Windows abertos, ou reinicie o computador, e tente de novo.
failure-windows-needs-check = O Windows precisa verificar o disco antes
failure-windows-needs-check-do = Nada foi alterado. Inicie o Windows, deixe-o verificar o disco, desligue-o e tente de novo.
failure-windows-unread = Não foi possível ler uma parte de um disco com o Windows
failure-windows-unread-do = Nada foi alterado. Desconecte os discos em que você não vai instalar e tente de novo, ou reduza o Windows pelo próprio Windows.
failure-bitlocker = Uma parte de um disco está bloqueada pelo BitLocker
failure-bitlocker-do = Nada foi alterado. Desative o BitLocker no Windows e tente de novo, ou reduza o Windows pelo próprio Windows.
failure-needs-internet = Esta escolha precisa de internet
failure-needs-internet-do = Conecte-se à internet, ou escolha a área de trabalho Linexin, que não precisa dela.
failure-needs-internet-untouched-do = Nada foi apagado. Conecte-se à internet, ou escolha a área de trabalho Linexin, que não precisa dela.
failure-command = Algo deu errado durante a instalação
failure-command-do = Veja os detalhes e tente de novo. Pode ser preciso apagar o disco.
failure-internal = O instalador encontrou um problema
failure-internal-do = Feche-o e tente de novo. Se acontecer outra vez, guarde os detalhes para relatar o erro.

## Why a choice cannot be installed, said beside it

invalid-username-empty = Escolha um nome de usuário
invalid-username-shape = Letras minúsculas, dígitos, - e _, começando com uma letra
invalid-username-reserved = Esse nome pertence ao sistema
invalid-full-name = Um nome não pode ter : nem ,
invalid-hostname = Letras, dígitos e -, no máximo 63
invalid-password-empty = Escolha uma senha
invalid-password-mismatch = As duas senhas não são iguais
invalid-password-shape = No máximo 128 caracteres, sem quebras de linha
invalid-root-password-empty = Escolha uma senha para a conta de administrador
invalid-locale = Escolha um idioma
invalid-timezone = Escolha um fuso horário
invalid-keyboard = Escolha um teclado
invalid-disk-blocked = Este disco não pode ser usado
invalid-target-missing = Escolha onde instalar
invalid-target-small = Não há espaço suficiente para o Linexin ali
table-full = Este disco não tem lugar para outra parte. Substitua uma delas.
invalid-target-esp = Essa parte é a que inicia o computador
target-protected = O Windows reserva essa parte para si
invalid-home = Essa parte não pode guardar seus arquivos
invalid-swap = A memória extra é maior que o espaço que há ali
invalid-bootloader = Esse gerenciador de inicialização não pode ser usado neste computador
invalid-source = A cópia do Linexin a instalar não foi encontrada
invalid-layout = Suas partições não podem ser usadas como foram planejadas

## The time zone step

timezone-heading-region = Parte do mundo
timezone-heading-city = Cidade mais próxima
timezone-heading-found = Lugares encontrados
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] Um lugar
   *[other] { $count } lugares
}
zone-region-africa = África
zone-region-america = Américas
zone-region-antarctica = Antártida
zone-region-arctic = Ártico
zone-region-asia = Ásia
zone-region-atlantic = Oceano Atlântico
zone-region-australia = Austrália
zone-region-europe = Europa
zone-region-indian = Oceano Índico
zone-region-pacific = Oceano Pacífico
zone-region-utc = Hora universal
# The one place under Universal time: the clock no country keeps.
zone-utc = Tempo Universal Coordenado

## The keyboard step

keyboard-heading-layout = Layout
keyboard-heading-variant = Tipo
keyboard-heading-found = Tipos encontrados
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] Só o padrão
    [one] O padrão e mais um
   *[other] O padrão e mais { $count }
}
# Under the plain layout, first among its kinds.
keyboard-plain = Padrão
keyboard-test = Teste seu teclado
keyboard-test-placeholder = Digite algo para testar as teclas

## The search over a long list (Language, Time zone, Keyboard)

search-field = Buscar
search-language-placeholder = Um idioma, também em inglês
search-timezone-placeholder = Cidade, país ou parte do mundo
search-keyboard-placeholder = Um layout ou um tipo dele
search-nothing = Nada encontrado para “{ $query }”

## The step of where to install

button-options = Opções
target-heading-disks = Discos
target-erase = Apagar este disco
target-erase-note = Tudo nele é apagado
target-free = Espaço livre · { $size }
target-free-note = Nada no disco é apagado
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = Substituir { $what } · { $size }
target-replace-note = O conteúdo é apagado
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = Substituir “{ $label }” · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · conteúdo apagado
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = “{ $label }” · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = Espaço livre
target-no-disk = Não há nenhum disco em que o Linexin possa ser instalado.
target-options = Opções
# Each option with the state it is in.
target-option-storage = Armazenamento · { $value }
target-option-swap = Memória extra · { $value }
target-option-home = Seus arquivos · { $value }
target-storage = Armazenamento
target-storage-standard = Padrão
target-storage-snapshots = Com pontos de restauração
target-storage-snapshots-short = Pontos de restauração
target-swap = Memória extra
target-swap-off = Desligada
target-home = Seus arquivos
target-home-system = Junto com o sistema
target-home-kept = { $part }, conteúdo mantido
target-home-erased = { $part }, conteúdo apagado
target-home-keep = Manter o conteúdo
target-home-erase = Apagar o conteúdo
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = Usando suas partições · { $count ->
    [one] uma operação
   *[other] { $count } operações
}
target-partitions-let-go = Um disco mudou, então suas partições foram descartadas. Escolha de novo.

## Partições

button-advanced = Avançado
partitions-title = Partições
partitions-not-yet = As partições ainda não podem ser aplicadas nesta versão.
partitions-not-yet-fresh-table = Uma nova tabela de partições ainda não pode ser aplicada nesta versão.
partitions-not-yet-keep-table = Criar ou excluir partições ainda não pode ser aplicado nesta versão.
partitions-not-yet-change-entries = Formatar uma partição ainda não pode ser aplicado nesta versão.
partitions-not-yet-kept-home = Manter um /home ainda não pode ser aplicado nesta versão.
partitions-not-yet-several-disks = Um plano em vários discos ainda não pode ser aplicado nesta versão.
partitions-not-yet-bios = Partições em um computador com BIOS ainda não podem ser aplicadas nesta versão.
partitions-not-yet-shrink = Reduzir uma partição ainda não pode ser aplicado nesta versão.
partitions-unallocated = não alocado
partitions-unknown = desconhecido
partitions-no-table = sem tabela de partições
partitions-partition = Partição
partitions-filesystem = Sistema de arquivos
partitions-label = Rótulo
partitions-size = Tamanho
partitions-used = Usado
partitions-mount-point = Ponto de montagem
partitions-flags = Sinalizadores
partitions-pending = Pendente
partitions-start = Início
partitions-end = Fim
partitions-reading = Lendo…
partitions-menu-mount = Ponto de montagem · { $value }
partitions-menu-format = Formatar como · { $value }
partitions-format-as = Formatar como
partitions-none = nenhum
partitions-keep = manter
partitions-delete = Excluir
partitions-undo-part = Desfazer as alterações em { $partition }
partitions-undo = Desfazer as alterações
partitions-create-table = Criar tabela de partições
partitions-create-table-kind = Criar tabela de partições { $table }
partitions-table = Tabela de partições
partitions-undo-last = Desfazer a última operação
partitions-clear = Limpar todas as operações
partitions-pending-list = Operações pendentes ({ $count })…
partitions-swap-file = Arquivo de troca · { $value }
partitions-swap-file-title = Arquivo de troca
partitions-pending-title = Operações pendentes
partitions-pending-short = Pendentes
partitions-pending-none = Nenhuma operação pendente
partitions-pending-earlier = { $count ->
    [one] Uma operação anterior
   *[other] { $count } operações anteriores
}
partitions-tile-format = formatar
partitions-tile-deleted = { $count ->
    [one] { $partitions } excluída
   *[other] { $partitions } excluídas
}
partitions-do-format = Formatar como { $filesystem }
partitions-do-mount = Montar em { $path }
partitions-do-delete = Excluir { $partition }
partitions-op-delete = Excluir { $partition } ({ $filesystem }, { $size })
partitions-op-format = Formatar { $partition } como { $filesystem }
partitions-op-mount = Definir o ponto de montagem de { $partition } como { $path }
partitions-op-table = Criar tabela de partições { $table } em { $disk }
partitions-op-create = Criar a partição primária nº { $number } ({ $filesystem }, { $size }) em { $disk }
partitions-op-create-at = Criar a partição primária nº { $number } ({ $filesystem }, { $size }, { $path }) em { $disk }
partitions-op-shrink = Reduzir { $partition } de { $from } para { $to }
partitions-op-grow = Aumentar { $partition } de { $from } para { $to }
partitions-op-label = Definir o rótulo “{ $label }” no sistema de arquivos de { $partition }
partitions-op-label-clear = Remover o rótulo do sistema de arquivos de { $partition }
partitions-missing-root = Falta uma partição montada em /
partitions-missing-efi = Falta uma partição EFI em /boot ou /efi
partitions-missing-boot = Adicione /boot ou monte a partição EFI em /boot
partitions-missing-bios-grub = Falta uma partição bios_grub em { $disk }
partitions-too-small = { $path } precisa de pelo menos { $size }
partitions-must-format = Formate { $partition } para usá-la em { $path }
partitions-needs = { $path } precisa de { $filesystem }
partitions-needs-linux = { $path } precisa de ext4, btrfs ou xfs
partitions-needs-root = { $path } precisa de ext4 ou btrfs
partitions-twice = { $path } está definido em mais de uma partição
partitions-blocked = Nada pode ser alterado em { $disk }
partitions-refused-mismatch = O plano para { $disk } não corresponde ao disco
partitions-refused-unaligned = { $partition } deve estar alinhada ao MiB
partitions-refused-outside = { $partition } passa do espaço do seu disco
partitions-refused-overlap = { $partition } se sobrepõe a outra partição
partitions-refused-beside-extended = { $disk }: nada ao lado da partição estendida
partitions-refused-logical = Exclua antes as partições lógicas após { $partition }
partitions-refused-entries = { $disk } teria mais de 128 partições
partitions-refused-shrink = { $partition } não pode ser reduzida
partitions-refused-grow = { $partition } é mantida, então não pode crescer
partitions-refused-bios-grub = { $partition } não pode ser uma partição bios_grub aqui
partitions-refused-label = { $partition }: rótulo inválido
partitions-refused-unmountable = { $partition }: { $filesystem } não é montável
partitions-refused-inside-kept = { $path } é mantida: nada pode ser montado dentro
partitions-refused-fs-small = { $partition } é pequena demais para { $filesystem }
partitions-refused-restore-points = Não há pontos de restauração com um /var separado
partitions-refused-boot-disk = /boot deve estar no mesmo disco que { $path }
partitions-refused-efi-bios = /efi só é usado quando o UEFI inicia o computador
partitions-refused-grub-gap = O GRUB precisa do primeiro MiB de { $disk } livre
partitions-refused-bootloader = { $loader } não pode iniciar o Linexin destas partições
partitions-refused-not-read = Ainda não se sabe quanto { $partition } ocupa
partitions-refused-shrink-below = { $partition } não pode ficar menor que { $size }
partitions-refused-hibernated = { $partition }: Windows hibernado, sem redução
partitions-refused-needs-check = { $partition } só pode ser reduzida após verificação
partitions-refused-encrypted = { $partition }: criptografada, sem redução
partitions-refused-in-use = { $partition }: em uso, sem redução
partitions-refused-unreadable = { $partition }: ilegível, sem redução
partitions-refused-no-hiberfile = { $partition }: sem redução aqui, reduza-a no Windows
partitions-refused-windows-unread = Sem redução: o Windows em { $other } não foi lido
partitions-refused-bitlocker = Sem redução: { $other } tem BitLocker
partitions-why-hibernated = Windows em { $other }: desligue sem inicialização rápida
partitions-why-windows-unread = Só reduz quando o Windows em { $other } for lido
partitions-why-bitlocker = Para reduzir, desative o BitLocker em { $other }
partitions-why-no-hiberfile = Para reduzir, use o Gerenciamento de Disco do Windows
partitions-refused-esp-full = { $partition } precisa de { $size } livres
partitions-refused-image-unknown = Ainda não se sabe quanto o Linexin põe em { $path }
partitions-refused-unread = Não foi possível ler quanto { $partition } ocupa
partitions-refused-image-unread = Não foi possível ler quanto o Linexin põe em { $path }
partitions-new = Nova…
partitions-resize = Redimensionar…
partitions-relabel = Rótulo…
partitions-other = Outro…
partitions-new-name = Nova partição nº { $number }
partitions-tile-new = nova
partitions-new-title = Nova partição
partitions-resize-title = Redimensionar { $partition }
partitions-label-title = Rótulo de { $partition }
partitions-other-title = Ponto de montagem de { $partition }
partitions-new-size = Novo tamanho
partitions-position = Posição
partitions-position-start = início do espaço
partitions-position-end = fim do espaço
partitions-bios-grub = bios_grub (sem formatação)
partitions-cancel = Cancelar
partitions-add = Adicionar
partitions-resize-button = Redimensionar
partitions-set = Definir
partitions-leaves = Deixa { $size } não alocados depois dela
partitions-leaves-none = Não deixa espaço não alocado depois dela
partitions-takes = Ocupa { $size } do espaço não alocado depois dela
partitions-takes-all = Ocupa todo o espaço não alocado depois dela
partitions-at-least = Precisa de pelo menos { $size }
partitions-path-placeholder = Por exemplo, /data
partitions-path-whole = Um ponto de montagem é um caminho completo a partir de /, como /data
partitions-path-twice = { $path } já é um ponto de montagem
partitions-path-system = { $path } pertence ao sistema
partitions-path-long = Um ponto de montagem tem no máximo 255 bytes
partitions-path-spaces = Um ponto de montagem não tem espaços nem barras invertidas
partitions-path-under = Nada pode ser montado dentro de { $path }
partitions-label-bytes = Rótulos { $filesystem } têm no máximo { $count ->
    [one] um byte
   *[other] { $count } bytes
}
partitions-label-chars = Rótulos { $filesystem } têm no máximo { $count ->
    [one] um caractere
   *[other] { $count } caracteres
}
partitions-label-slash = Um rótulo não pode conter “/”
partitions-why-table = Ainda não há tabela de partições: escolha este espaço para criar uma
partitions-why-primaries = Um disco msdos comporta no máximo quatro partições primárias
partitions-why-logical = Não é possível criar partições lógicas aqui
partitions-why-beside-extended = Nada pode ser criado ao lado de uma partição estendida
partitions-why-whole = Algo ocupa o disco todo: não é possível criar tabela nem partição
partitions-why-unmountable = { $filesystem } não pode ser montado: formate para usar
partitions-why-unformatted = Sem sistema de arquivos: formate para usar
partitions-why-logical-delete = Exclua antes as partições lógicas de número maior
partitions-primaries = { $disk } teria mais de quatro partições primárias
partitions-do-create = Criar
partitions-do-shrink = Reduzir para { $size }
partitions-do-grow = Aumentar para { $size }

## The desktop step

button-done = Concluído
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = Baseado no GNOME
desktop-about-kinexin = Baseado no KDE Plasma
desktop-about-linexinbar = Feito para um controle
# Under a desktop: whether it has to be downloaded.
desktop-ready = Não precisa de internet
desktop-needs-internet = Precisa de internet
desktop-refused = { $desktop } precisa de internet. Conecte-se, ou escolha o Linexin.
desktop-checking = Verificando a conexão com a internet…
desktop-checking-short = Verificando a internet…
desktop-offline = Não há internet, então o que precisa dela fica de fora.
desktop-updates = Instalar atualizações
desktop-updates-on = Tudo atualizado
desktop-updates-off = Como está no pendrive
updates-refused = As atualizações precisam de internet. Elas podem ser instaladas depois.
desktop-options-title = Opções
desktop-options-caption = O que mais é instalado e como o computador inicia.
desktop-option-remove = Programas a remover
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = Programas
desktop-flatpaks-title = Aplicativos
desktop-bootloader-title = Inicialização
desktop-remove-caption = Desligue o que você não quer no sistema instalado.
desktop-remove-looking = Verificando os programas…
desktop-remove-unlisted = Não foi possível listar os programas
desktop-remove-none = Não há nada para remover
desktop-remove-nothing = Nada é removido
desktop-remove-count = { $count ->
    [one] Um fica de fora
   *[other] { $count } ficam de fora
}
desktop-option-flatpaks = Aplicativos do Flathub
desktop-flatpaks-caption = Instalados da internet junto com o Linexin.
# How many of the apps offered are switched on.
desktop-flatpaks-count = { $count } de { $total } escolhidos
desktop-flatpak-on = Instalado com o Linexin
desktop-flatpak-off = Fica de fora
flatpaks-refused = Os aplicativos do Flathub precisam de internet. Eles podem ser adicionados depois.
desktop-option-bootloader = Gerenciador de inicialização
desktop-bootloader-caption = O que inicia o Linexin quando o computador é ligado.
desktop-option-drivers = Remover drivers sem uso
desktop-drivers-on = Os sem uso são removidos
desktop-drivers-off = Todos os drivers são mantidos
desktop-option-snapshots = Pontos de restauração
desktop-snapshots-on = Salvos antes de cada atualização
desktop-snapshots-off = Nenhum é salvo
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = Ative em “Onde instalar”
desktop-snapshots-var = Não com um /var separado
bootloader-automatic = Automático
bootloader-automatic-note = Escolhido para este computador
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = Inicia o Linexin direto, sem menu
bootloader-grub = GRUB
bootloader-grub-note = Um menu ao iniciar, em qualquer computador
bootloader-refind = rEFInd
bootloader-refind-note = Um menu com todos os sistemas do computador
bootloader-unavailable = Não pode ser usado neste computador
# The groups the software to remove is listed in.
package-group-applications = Aplicativos
package-group-multimedia = Som e vídeo
package-group-network = Rede
package-group-fonts = Fontes
package-group-libraries = Bibliotecas
