# Linexin Installer — Chinese (Simplified).
#
# Every message of en-GB.ftl, in Simplified Chinese. The comments are
# en-GB's: they say where a message appears and what it means. The words the
# family already uses — 返回, 继续, 选项, 关闭, 选择, 完成, 安装, 立即重新启动,
# 暂不, 详情, 隐藏, 用户名, 密码, 确认密码, 管理员, 账户, 键盘, 布局, 标准,
# 自动, 更新, 安装更新, 存储, 软件, 网络, 手柄, 空, 关, 时区, 磁盘, 启动, 重试,
# 正在准备, 正在检查, 尚未选择 — are the shell's and the toolkit's, and so is
# the polite 您. A computer is a 电脑, as the shell's drive page says it;
# spare memory is 额外内存, as its Storage page says it. A part of a disk is a
# 分区, the word every Chinese reader already knows from their own drives, a
# USB stick a U 盘, and restore points 还原点. Chinese has no plural forms:
# a count takes the other form alone. Names stand in “ ”, as the shell sets
# them, and a space stands between Chinese and Latin words or numbers.

## The application itself

app-name = Linexin 安装程序

## The head of each screen: its title, and the one line under it

welcome-caption = 回答几个问题，就能把 Linexin 装到这台电脑上。
language-title = 语言
language-caption = 这台电脑使用的语言。
timezone-title = 时区
timezone-caption = 您所在的地方，好让时钟准确。
keyboard-title = 键盘
keyboard-caption = 您用来打字的键盘。
target-title = 安装位置
target-caption = Linexin 将安装到的磁盘。
desktop-title = 桌面
desktop-caption = Linexin 的样子。每一种以后都可以按喜好调整。
account-title = 关于您
account-caption = 您的名字，以及用来登录的账户。
review-title = 准备安装 Linexin
review-caption = 请检查一遍。目前还没有做任何更改。
installing-title = 正在安装 Linexin
installing-caption = 这需要一段时间。无需按任何键。
done-title = Linexin 已安装
done-caption = 重新启动电脑并拔出 U 盘，即可开始使用。
stopped-title = 安装已停止

## The verbs at the foot of a screen

button-continue = 继续
button-install = 安装
button-close = 关闭
button-restart = 立即重新启动
button-not-now = 暂不
button-restart-short = 重新启动

## What the buttons do, in the legend at the foot of the panel

hint-choose = 选择
hint-back = 返回
hint-quit = 退出
hint-type = 输入
hint-shift = Shift
hint-close = 关闭
hint-done = 完成
hint-options = 选项
hint-details = 详情
hint-hide-details = 隐藏详情
hint-search = 搜索
hint-adjust = 调整
hint-keep = 确定
hint-put-back = 还原
hint-cancel = 取消

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = àé

## Questions, asked with the safe answer first

quit-title = 要退出安装程序吗？
quit-body = 这台电脑上没有做任何更改。
quit-stay = 留下
quit-leave = 退出
erase-title = 要清除并安装吗？
erase-title-short = 要清除吗？
install-title = 要安装 Linexin 吗？
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = { $disk } 上的所有内容都将被清除。
erase-target-part = { $disk } 上的 { $part } · { $size } 将被清除。
erase-target-free = Linexin 将安装到 { $disk } 的可用空间中。
erase-home = { $part }（用于存放您的文件）将被清除。
erase-final = 此操作无法撤销。
erase-nothing = 其中的内容不会被清除。
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = 这些操作会更改您磁盘上的内容：
erase-plan-nothing = 您磁盘上的任何内容都不会被清除。
erase-plan-erased = 清除 { $partition }（{ $filesystem }，{ $size }）
erase-plan-esp = { $what } · EFI 系统分区
erase-plan-windows = { $what } · Windows
erase-plan-page = 第 { $page } 页，共 { $pages } 页
# A part of a disk called by the name it was given, in a sentence.
part-labelled = “{ $label }”
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $disk } 上的 { $part }
erase-back = 返回
erase-confirm = 清除并安装
erase-confirm-short = 清除
button-next = 下一页

## A line beside the legend

demo-note = 一台虚构的电脑：这里的一切都不是真的，也不会做任何更改。
install-unavailable = 要在这台电脑上安装 Linexin，请用 Linexin U 盘启动它。
disks-unreadable = 无法读取这台电脑的磁盘。
install-reading = 仍在读取这台电脑的磁盘，请稍后再试。
installing-note = 完成之前请保持电脑开机。
done-all-well = 一切都按计划完成。
done-warnings = { $count ->
   *[other] Linexin 已经可以使用了，但有 { $count } 件事没有按计划完成：
}
done-restart-failed = 无法重新启动电脑。请从系统菜单重新启动。

## The fields

field-full-name = 姓名
field-full-name-placeholder = 其他人会看到的名字
field-username = 用户名
field-username-placeholder = 登录时使用的名字
field-password = 密码
field-password-placeholder = 设置一个密码
field-repeat = 确认密码
field-repeat-placeholder = 再输入一次同样的密码
field-hostname = 电脑名称
field-hostname-placeholder = 这台电脑在网络上显示的名称
field-root-password = 管理员密码
field-root-password-placeholder = 用于更改系统的密码
field-root-repeat = 确认管理员密码
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = 弱
strength-fair = 一般
strength-good = 良好
strength-strong = 强

## You

# A separate administrator (root) account, off unless asked for.
account-root = 管理员账户
account-root-off = 关：用您自己的密码进行更改
account-root-on = 使用单独的密码，见下方

## Sizes, as a box writes them

size-tb = { $amount } TB
size-gb = { $amount } GB
size-mb = { $amount } MB
size-kb = { $amount } KB
size-bytes = { $amount } 字节

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = 高速固态硬盘
disk-kind-sata = 内置硬盘
disk-kind-usb = USB 驱动器
disk-kind-card = 存储卡
disk-kind-virtual = 虚拟磁盘
disk-kind-other = 磁盘
disk-empty = 空
disk-holds-more = { $first } 和另外 { $count ->
   *[other] { $count } 个分区
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what }，占满整个磁盘

## What a part of a disk appears to hold

content-efi = 启动文件
content-bios-boot = 启动空间
content-reserved = Windows 保留空间
content-recovery = Windows 恢复
content-windows = Windows
content-linux = Linux
content-swap = 额外内存
content-data = 文件
content-empty = 空
content-other = 其他内容

## Why a disk cannot be chosen, said under it

blocked-installation-media = 安装程序正在从这里运行
blocked-in-use = 正在使用中
blocked-swap = 正用作额外内存
blocked-holder = 正被这台电脑上的其他程序使用
blocked-read-only = 只能读取，不能写入
blocked-too-small = 容量太小，装不下 Linexin
blocked-unknown = 无法使用

## The review

# Each row names a step; under it, that step's answer.
review-where = 位置
review-desktop = 桌面
review-region-name = 语言和地区
review-account-name = 账户
review-extras = 其他
review-where-disk = { $disk } · 其中所有内容都将被清除
review-where-free = { $disk } · 安装到可用空间
review-where-part = { $disk } · 替换 { $part }
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = 您的文件放在 { $part }，保留其内容
review-where-home-erased = 您的文件放在 { $part }，先清除其内容
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = 尚未选择
review-extra-updates = 更新
review-extra-apps = { $count ->
   *[other] { $count } 个应用
}
review-extra-snapshots = 还原点
review-extra-swap = { $size } 额外内存
review-extra-none = 没有其他内容
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $disk } 上的 { $partition } · { $filesystem } · { $size }
review-part-formatted = { $partition } · { $filesystem } · { $size } · 格式化
review-part-kept = { $partition } · { $filesystem } · { $size } · 保留
review-operations = { $count ->
   *[other] 将应用 { $count } 个操作
}

## The installation, while it runs

installing-waiting = 正在准备…
installing-count = 第 { $step } 步，共 { $steps } 步 · { $percent }%
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = 停止于：{ $step }
step-check = 正在检查电脑
step-resize = 正在缩小保留在磁盘上的分区
step-partition = 正在磁盘上腾出空间
step-format = 正在准备磁盘
step-copy = 正在复制 Linexin
step-swap = 正在预留额外内存
step-system = 正在设置系统
step-region = 正在设置语言、时区和键盘
step-accounts = 正在创建您的账户
step-desktop = 正在设置桌面
step-updates = 正在安装更新
step-packages = 正在添加和移除软件
step-flatpaks = 正在安装应用
step-drivers = 正在移除这台电脑不需要的驱动
step-snapshots = 正在设置还原点
step-initramfs = 正在准备系统启动
step-bootloader = 正在让电脑能够启动 Linexin
step-finish = 正在收尾

## What did not work, said once Linexin is installed

warning-updates-offline = 由于没有联网，更新被跳过了。之后可以再安装。
warning-updates-failed = 无法安装更新。之后可以再安装。
warning-flatpaks-failed = 部分应用未能安装。之后可以再添加。
warning-packages-kept = 部分本应移除的软件仍然存在。
warning-boot-entry = 如果 Linexin 无法启动，请在电脑的启动菜单中选择它。
warning-snapshots = 无法设置还原点。
warning-drivers-kept = 这台电脑不需要的驱动被保留了下来。
warning-windows-check = Windows 下次启动时会检查它的磁盘。请让检查完成：在此之前，Linexin 不会动 Windows 的文件。

## Why the installation stopped, and what can be done about it

failure-invalid-request = 安装程序请求了它无法完成的操作
failure-invalid-request-do = 请关闭安装程序，重新打开，然后再完成一遍各个步骤。
failure-consent-mismatch = 将被清除的内容发生了变化
failure-consent-mismatch-do = 没有清除任何内容。请重新完成各个步骤并检查磁盘。
failure-not-root = 安装程序没有获得权限
failure-not-root-do = 请重新打开安装程序，并在询问时允许。
failure-not-live = 这台电脑不是从安装程序启动的
failure-not-live-do = 请用 Linexin U 盘启动电脑，然后重试。
failure-secure-boot = 安全启动已开启
failure-secure-boot-do = 请在电脑的启动设置中关闭安全启动（Secure Boot），然后重试。
failure-already-running = 另一个安装正在进行
failure-already-running-do = 请等待它完成，或重新启动电脑后重试。
failure-missing-tool = 安装程序缺少所需的组件
failure-missing-tool-do = 请使用较新的 Linexin U 盘，然后重试。
failure-image-invalid = U 盘上的 Linexin 副本已损坏
failure-image-invalid-do = 请重新制作 U 盘，然后再试一次。
failure-disk-changed = 磁盘在选择后发生了变化
failure-disk-changed-do = 磁盘的部分内容可能已被清除。请重新完成各个步骤，并再次选择磁盘。
failure-disk-changed-untouched-do = 没有清除任何内容。请重新完成各个步骤，并再次选择磁盘。
failure-disk-busy = 磁盘正被其他程序使用
failure-disk-busy-do = 请关闭正在使用它的程序，或重新启动电脑，然后重试。
failure-disk-busy-untouched-do = 没有清除任何内容。请关闭正在使用该磁盘的程序，或重新启动电脑，然后重试。
failure-no-space = 空间不足
failure-no-space-do = 请选择更大的磁盘或更多可用空间，然后重试。
failure-esp-full = 磁盘的启动空间已满
failure-esp-full-do = 请改为清除整个磁盘，或先在其启动空间中腾出位置。
failure-shrink-refused = 无法缩小磁盘上的一个分区
failure-shrink-refused-do = 磁盘上的各个分区都还在，但正在缩小的那个可能已有改动。请让它所属的系统检查它，然后重试。
failure-shrink-refused-untouched-do = 没有更改任何内容。请重新打开安装程序，给该分区留出更多空间，或在它所属的系统中缩小它。
failure-windows-hibernated = Windows 没有完全关机
failure-windows-hibernated-do = 没有更改任何内容。请启动 Windows，关闭快速启动，再关机，然后重试。
failure-needs-check = 需要先检查磁盘上的一个分区
failure-needs-check-do = 磁盘上的各个分区都还在，但正在缩小的那个可能已有改动。请启动它所属的系统，让它检查磁盘，然后重试。
failure-needs-check-untouched-do = 没有更改任何内容。请启动该分区所属的系统，让它检查磁盘，然后重试。
failure-windows-in-use = 装有 Windows 的磁盘分区正在使用
failure-windows-in-use-do = 没有更改任何内容。请关闭打开了 Windows 文件的程序，或重新启动电脑，然后重试。
failure-windows-needs-check = Windows 需要先检查它的磁盘
failure-windows-needs-check-do = 没有更改任何内容。请启动 Windows，让它检查磁盘，再关机，然后重试。
failure-windows-unread = 无法读取装有 Windows 的磁盘分区
failure-windows-unread-do = 没有更改任何内容。请拔下不安装系统的磁盘后重试，或在 Windows 中自行缩小 Windows。
failure-bitlocker = 磁盘上的一个分区被 BitLocker 锁定
failure-bitlocker-do = 没有更改任何内容。请在 Windows 中关闭 BitLocker 后重试，或在 Windows 中自行缩小 Windows。
failure-needs-internet = 此选项需要联网
failure-needs-internet-do = 请连接网络，或选择无需联网的 Linexin 桌面。
failure-needs-internet-untouched-do = 没有清除任何内容。请连接网络，或选择无需联网的 Linexin 桌面。
failure-command = 安装过程中出了问题
failure-command-do = 请查看详情，然后重试。可能需要清除磁盘。
failure-internal = 安装程序遇到了问题
failure-internal-do = 请关闭后重试。如果再次发生，请保留详情以便报告问题。

## Why a choice cannot be installed, said beside it

invalid-username-empty = 请设置用户名
invalid-username-shape = 小写英文字母、数字、- 和 _，以字母开头
invalid-username-reserved = 这个名字是系统专用的
invalid-full-name = 名字中不能包含 : 或 ,
invalid-hostname = 英文字母、数字和 -，最多 63 个
invalid-password-empty = 请设置密码
invalid-password-mismatch = 两次输入的密码不一致
invalid-password-shape = 最多 128 个字符，不能换行
invalid-root-password-empty = 请为管理员账户设置密码
invalid-locale = 请选择语言
invalid-timezone = 请选择时区
invalid-keyboard = 请选择键盘
invalid-disk-blocked = 无法使用此磁盘
invalid-target-missing = 请选择安装位置
invalid-target-small = 那里没有足够空间安装 Linexin
table-full = 此磁盘无法再添加分区。请改为替换一个现有分区。
invalid-target-esp = 那个分区用于启动电脑
target-protected = 那个分区是 Windows 自己保留的
invalid-home = 那个分区不能存放您的文件
invalid-swap = 额外内存比那里的空间还大
invalid-bootloader = 那个启动管理器无法在这台电脑上使用
invalid-source = 找不到要安装的 Linexin 副本
invalid-layout = 无法按计划使用您的分区

## The time zone step

timezone-heading-region = 区域
timezone-heading-city = 最近的城市
timezone-heading-found = 找到的地点
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
   *[other] { $count } 个地点
}
zone-region-africa = 非洲
zone-region-america = 美洲
zone-region-antarctica = 南极洲
zone-region-arctic = 北极
zone-region-asia = 亚洲
zone-region-atlantic = 大西洋
zone-region-australia = 澳大利亚
zone-region-europe = 欧洲
zone-region-indian = 印度洋
zone-region-pacific = 太平洋
zone-region-utc = 世界时
# The one place under Universal time: the clock no country keeps.
zone-utc = 协调世界时

## The keyboard step

keyboard-heading-layout = 布局
keyboard-heading-variant = 类型
keyboard-heading-found = 找到的类型
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] 仅标准
   *[other] 标准及另外 { $count } 种
}
# Under the plain layout, first among its kinds.
keyboard-plain = 标准
keyboard-test = 试试您的键盘
keyboard-test-placeholder = 输入一些内容来试试按键

## The search over a long list (Language, Time zone, Keyboard)

search-field = 搜索
search-language-placeholder = 语言名称，用其本身的文字或英文
search-timezone-placeholder = 城市、国家或大洲
search-keyboard-placeholder = 布局或其类型
search-nothing = 没有找到“{ $query }”

## The step of where to install

button-options = 选项
target-heading-disks = 磁盘
target-erase = 清除此磁盘
target-erase-note = 其中所有内容都将被清除
target-free = 可用空间 · { $size }
target-free-note = 磁盘上的内容不会被清除
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = 替换 { $what } · { $size }
target-replace-note = 其中的内容将被清除
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = 替换“{ $label }” · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · 将被清除
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = “{ $label }” · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = 可用空间
target-no-disk = 没有可以安装 Linexin 的磁盘。
target-options = 选项
# Each option with the state it is in.
target-option-storage = 存储 · { $value }
target-option-swap = 额外内存 · { $value }
target-option-home = 您的文件 · { $value }
target-storage = 存储
target-storage-standard = 标准
target-storage-snapshots = 带还原点
target-storage-snapshots-short = 还原点
target-swap = 额外内存
target-swap-off = 关
target-home = 您的文件
target-home-system = 与系统放在一起
target-home-kept = { $part }，保留内容
target-home-erased = { $part }，清除内容
target-home-keep = 保留其中的内容
target-home-erase = 清除它
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = 正在使用您的分区 · { $count ->
   *[other] { $count } 个操作
}
target-partitions-let-go = 有磁盘发生了变化，您的分区已被清除。请重新选择。

## 分区

button-advanced = 高级
partitions-title = 分区
partitions-not-yet = 此版本尚不能应用分区。
partitions-not-yet-fresh-table = 此版本尚不能应用新的分区表。
partitions-not-yet-keep-table = 此版本尚不能应用创建或删除分区。
partitions-not-yet-change-entries = 此版本尚不能应用格式化分区。
partitions-not-yet-kept-home = 此版本尚不能应用保留的 /home。
partitions-not-yet-several-disks = 此版本尚不能应用跨多个磁盘的方案。
partitions-not-yet-bios = 此版本尚不能在 BIOS 启动的电脑上应用分区。
partitions-not-yet-shrink = 此版本尚不能应用缩小分区。
partitions-unallocated = 未分配
partitions-unknown = 未知
partitions-no-table = 无分区表
partitions-partition = 分区
partitions-filesystem = 文件系统
partitions-label = 卷标
partitions-size = 大小
partitions-used = 已用
partitions-mount-point = 挂载点
partitions-flags = 标志
partitions-pending = 待执行
partitions-start = 起始
partitions-end = 结束
partitions-reading = 正在读取…
partitions-menu-mount = 挂载点 · { $value }
partitions-menu-format = 格式化为 · { $value }
partitions-format-as = 格式化为
partitions-none = 无
partitions-keep = 保留
partitions-delete = 删除
partitions-undo-part = 撤销对 { $partition } 的更改
partitions-undo = 撤销更改
partitions-create-table = 创建分区表
partitions-create-table-kind = 创建 { $table } 分区表
partitions-table = 分区表
partitions-undo-last = 撤销上一个操作
partitions-clear = 清除所有操作
partitions-pending-list = 待执行的操作（{ $count }）…
partitions-swap-file = 交换文件 · { $value }
partitions-swap-file-title = 交换文件
partitions-pending-title = 待执行的操作
partitions-pending-short = 待执行
partitions-pending-none = 没有待执行的操作
partitions-pending-earlier = { $count ->
   *[other] 更早的 { $count } 个操作
}
partitions-tile-format = 格式化
partitions-tile-deleted = { $count ->
   *[other] 已删除 { $partitions }
}
partitions-do-format = 格式化为 { $filesystem }
partitions-do-mount = 挂载到 { $path }
partitions-do-delete = 删除 { $partition }
partitions-op-delete = 删除 { $partition }（{ $filesystem }，{ $size }）
partitions-op-format = 将 { $partition } 格式化为 { $filesystem }
partitions-op-mount = 将 { $partition } 的挂载点设为 { $path }
partitions-op-table = 在 { $disk } 上创建 { $table } 分区表
partitions-op-create = 在 { $disk } 上创建主分区 #{ $number }（{ $filesystem }，{ $size }）
partitions-op-create-at = 在 { $disk } 上创建主分区 #{ $number }（{ $filesystem }，{ $size }，{ $path }）
partitions-op-shrink = 将 { $partition } 从 { $from } 缩小到 { $to }
partitions-op-grow = 将 { $partition } 从 { $from } 扩大到 { $to }
partitions-op-label = 将 { $partition } 的文件系统卷标设为“{ $label }”
partitions-op-label-clear = 清除 { $partition } 的文件系统卷标
partitions-missing-root = 缺少：挂载到 / 的分区
partitions-missing-efi = 缺少：/boot 或 /efi 上的 EFI 分区
partitions-missing-boot = 请添加 /boot，或将 EFI 分区挂载到 /boot
partitions-missing-bios-grub = 缺少：{ $disk } 上的 bios_grub 分区
partitions-too-small = { $path } 至少需要 { $size }
partitions-must-format = 要将 { $partition } 用作 { $path }，请先将它格式化
partitions-needs = { $path } 需要 { $filesystem }
partitions-needs-linux = { $path } 需要 ext4、btrfs 或 xfs
partitions-needs-root = { $path } 需要 ext4 或 btrfs
partitions-twice = { $path } 被设给了多个分区
partitions-blocked = { $disk } 上的内容不能更改
partitions-refused-mismatch = { $disk } 的方案与磁盘不符
partitions-refused-unaligned = { $partition } 必须在整 MiB 处开始和结束
partitions-refused-outside = { $partition } 超出了所在磁盘的空间
partitions-refused-overlap = { $partition } 与另一分区重叠
partitions-refused-beside-extended = 不能在 { $disk } 的扩展分区旁创建分区
partitions-refused-logical = 请先删除 { $partition } 之后的逻辑分区
partitions-refused-entries = { $disk } 上将有 128 个以上的分区
partitions-refused-shrink = { $partition } 不能缩小
partitions-refused-grow = { $partition } 被保留，因此不能扩大
partitions-refused-bios-grub = { $partition } 在这里不能作为 bios_grub 分区
partitions-refused-label = { $partition } 的卷标不适合其文件系统
partitions-refused-unmountable = { $partition } 上的 { $filesystem } 无法挂载
partitions-refused-inside-kept = { $path } 被保留，不能在其中挂载任何内容
partitions-refused-fs-small = { $partition } 对 { $filesystem } 来说太小
partitions-refused-restore-points = 单独的 /var 旁不能保留还原点
partitions-refused-boot-disk = /boot 必须与 { $path } 在同一磁盘上
partitions-refused-efi-bios = 只有由 UEFI 启动电脑时才使用 /efi
partitions-refused-grub-gap = GRUB 需要 { $disk } 的第一个 MiB 保持空闲
partitions-refused-bootloader = { $loader } 无法从这些分区启动 Linexin
partitions-refused-not-read = 尚不知道 { $partition } 已用多少空间
partitions-refused-shrink-below = { $partition } 不能小于 { $size }
partitions-refused-hibernated = { $partition } 不能缩小：Windows 处于休眠状态
partitions-refused-needs-check = { $partition } 须先检查才能缩小
partitions-refused-encrypted = { $partition } 已加密，因此不能缩小
partitions-refused-in-use = { $partition } 正在使用，因此不能缩小
partitions-refused-unreadable = { $partition } 无法读取，因此不能缩小
partitions-refused-no-hiberfile = { $partition } 无法在此缩小：请在 Windows 中缩小
partitions-refused-windows-unread = 无法缩小：无法读取 { $other } 上的 Windows
partitions-refused-bitlocker = 无法缩小：{ $other } 已被 BitLocker 锁定
partitions-why-hibernated = { $other } 上的 Windows：关闭快速启动后再关机
partitions-why-windows-unread = 须能读取 { $other } 上的 Windows 才能缩小
partitions-why-bitlocker = 关闭 { $other } 上的 BitLocker 后才能缩小
partitions-why-no-hiberfile = 请用 Windows 的“磁盘管理”缩小
partitions-refused-esp-full = { $partition } 需要 { $size } 可用空间
partitions-refused-image-unknown = 尚不知道 Linexin 会在 { $path } 中放入多少
partitions-refused-unread = 无法读取 { $partition } 已用多少空间
partitions-refused-image-unread = 无法读取 Linexin 会在 { $path } 中放入多少
partitions-new = 新建…
partitions-resize = 调整大小…
partitions-relabel = 卷标…
partitions-other = 其他…
partitions-new-name = 新分区 #{ $number }
partitions-tile-new = 新
partitions-new-title = 新建分区
partitions-resize-title = 调整 { $partition } 的大小
partitions-label-title = { $partition } 的卷标
partitions-other-title = { $partition } 的挂载点
partitions-new-size = 新大小
partitions-position = 位置
partitions-position-start = 空间开头
partitions-position-end = 空间末尾
partitions-bios-grub = bios_grub（未格式化）
partitions-cancel = 取消
partitions-add = 添加
partitions-resize-button = 调整大小
partitions-set = 设定
partitions-leaves = 其后留出 { $size } 未分配空间
partitions-leaves-none = 其后不留未分配空间
partitions-takes = 占用其后 { $size } 的未分配空间
partitions-takes-all = 占用其后全部未分配空间
partitions-at-least = 至少需要 { $size }
partitions-path-placeholder = 例如 /data
partitions-path-whole = 挂载点须是从 / 开始的完整路径，例如 /data
partitions-path-twice = { $path } 已经是挂载点
partitions-path-system = { $path } 属于系统本身
partitions-path-long = 挂载点最长 255 字节
partitions-path-spaces = 挂载点不能含有空格或反斜杠
partitions-path-under = 不能在 { $path } 内挂载任何内容
partitions-label-bytes = { $filesystem } 卷标最多 { $count ->
   *[other] { $count } 字节
}
partitions-label-chars = { $filesystem } 卷标最多 { $count ->
   *[other] { $count } 个字符
}
partitions-label-slash = 卷标不能包含“/”
partitions-why-table = 尚无分区表：选择此空间以创建分区表
partitions-why-primaries = msdos 磁盘最多容纳四个主分区
partitions-why-logical = 此处不能创建逻辑分区
partitions-why-beside-extended = 不能在扩展分区旁创建分区
partitions-why-whole = 整个磁盘已被占用：不能创建分区表或分区
partitions-why-unmountable = { $filesystem } 无法挂载：请格式化后使用
partitions-why-unformatted = 没有文件系统：请格式化后使用
partitions-why-logical-delete = 请先删除编号在其后的逻辑分区
partitions-primaries = { $disk } 上将有四个以上的主分区
partitions-do-create = 创建
partitions-do-shrink = 缩小到 { $size }
partitions-do-grow = 扩大到 { $size }

## The desktop step

button-done = 完成
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = 基于 GNOME
desktop-about-kinexin = 基于 KDE Plasma
desktop-about-linexinbar = 为手柄打造
# Under a desktop: whether it has to be downloaded.
desktop-ready = 无需联网
desktop-needs-internet = 需要联网
desktop-refused = { $desktop } 需要联网。请连接网络，或选择 Linexin。
desktop-checking = 正在检查网络连接…
desktop-checking-short = 正在检查网络…
desktop-offline = 没有网络，需要联网的内容已略过。
desktop-updates = 安装更新
desktop-updates-on = 全部更新到最新
desktop-updates-off = 保持 U 盘上的版本
updates-refused = 更新需要联网。之后可以再安装。
desktop-options-title = 选项
desktop-options-caption = 还要安装什么，以及电脑如何启动。
desktop-option-remove = 要移除的软件
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = 软件
desktop-flatpaks-title = 应用
desktop-bootloader-title = 启动
desktop-remove-caption = 关闭您不想在安装后的系统中保留的软件。
desktop-remove-looking = 正在查看软件…
desktop-remove-unlisted = 无法列出软件
desktop-remove-none = 没有可移除的软件
desktop-remove-nothing = 不移除任何软件
desktop-remove-count = { $count ->
   *[other] 移除 { $count } 个
}
desktop-option-flatpaks = 来自 Flathub 的应用
desktop-flatpaks-caption = 与 Linexin 一起从网上安装。
# How many of the apps offered are switched on.
desktop-flatpaks-count = 已选 { $count } 个，共 { $total } 个
desktop-flatpak-on = 随 Linexin 一起安装
desktop-flatpak-off = 不安装
flatpaks-refused = 来自 Flathub 的应用需要联网。之后可以再添加。
desktop-option-bootloader = 启动管理器
desktop-bootloader-caption = 电脑开机时用什么来启动 Linexin。
desktop-option-drivers = 移除不需要的驱动
desktop-drivers-on = 用不到的会被移除
desktop-drivers-off = 保留所有驱动
desktop-option-snapshots = 还原点
desktop-snapshots-on = 每次更新前保存
desktop-snapshots-off = 不保存
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = 在“安装位置”中设置
desktop-snapshots-var = 单独的 /var 旁不保存
bootloader-automatic = 自动
bootloader-automatic-note = 根据这台电脑自动选择
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = 直接启动 Linexin，不显示菜单
bootloader-grub = GRUB
bootloader-grub-note = 开机时显示菜单，适用于任何电脑
bootloader-refind = rEFInd
bootloader-refind-note = 列出电脑上所有系统的菜单
bootloader-unavailable = 无法在这台电脑上使用
# The groups the software to remove is listed in.
package-group-applications = 应用
package-group-multimedia = 影音
package-group-network = 网络
package-group-fonts = 字体
package-group-libraries = 运行库
