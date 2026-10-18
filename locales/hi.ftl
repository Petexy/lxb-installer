# Linexin Installer — Hindi.
#
# Every message of en-GB.ftl, in Hindi. The comments are en-GB's: they say
# where a message appears and what it means. The words the family already
# uses — वापस, जारी रखें, विकल्प, बंद करें, चुनें, हो गया, इंस्टॉल करें,
# अभी पुनरारंभ करें, अभी नहीं, विवरण, उपयोगकर्ता नाम, पासवर्ड, पासवर्ड की
# पुष्टि करें, प्रशासक, खाता, कीबोर्ड, लेआउट, मानक, स्वचालित, अपडेट, स्टोरेज,
# सॉफ़्टवेयर, नेटवर्क, खाली, समय क्षेत्र, "फिर कोशिश करें" — are the shell's
# and the toolkit's, and so is the polite आप with its -ें imperatives. A part
# of a disk is a "हिस्सा", the plain word; डिस्क is feminine, as the shell
# writes it. A USB stick is a "पेन ड्राइव", the word India uses for it; spare
# memory is "अतिरिक्त मेमोरी" and start-up "स्टार्टअप". A count takes Hindi's
# own forms, one (which is also 0) and other. The username and computer-name
# rules say "अंग्रेज़ी अक्षर" outright, since a Hindi reader would otherwise
# try Devanagari first.

## The application itself

app-name = Linexin इंस्टॉलर

## The head of each screen: its title, and the one line under it

welcome-caption = कुछ सवाल, और Linexin इस कंप्यूटर पर आ जाएगा।
language-title = भाषा
language-caption = वह भाषा जिसमें यह कंप्यूटर बात करेगा।
timezone-title = समय क्षेत्र
timezone-caption = आप कहाँ हैं, ताकि घड़ी सही रहे।
keyboard-title = कीबोर्ड
keyboard-caption = वे कुंजियाँ जिन पर आप टाइप करते हैं।
target-title = कहाँ इंस्टॉल करें
target-caption = वह डिस्क जिस पर Linexin जाएगा।
desktop-title = डेस्कटॉप
desktop-caption = Linexin कैसा दिखेगा। हर एक को बाद में अपनी पसंद से बदला जा सकता है।
account-title = आप
account-caption = आपका नाम, और वह खाता जिससे आप साइन इन करेंगे।
review-title = Linexin इंस्टॉल करने के लिए तैयार
review-caption = एक बार देख लें। अभी तक कुछ भी बदला नहीं गया है।
installing-title = Linexin इंस्टॉल हो रहा है
installing-caption = इसमें कुछ समय लगता है। कुछ भी दबाने की ज़रूरत नहीं है।
done-title = Linexin इंस्टॉल हो गया
done-caption = इसे इस्तेमाल करने के लिए कंप्यूटर पुनरारंभ करें और पेन ड्राइव निकाल लें।
stopped-title = इंस्टॉलेशन रुक गया

## The verbs at the foot of a screen

button-continue = जारी रखें
button-install = इंस्टॉल करें
button-close = बंद करें
button-restart = अभी पुनरारंभ करें
button-not-now = अभी नहीं
button-restart-short = पुनरारंभ करें

## What the buttons do, in the legend at the foot of the panel

hint-choose = चुनें
hint-back = वापस
hint-quit = बाहर निकलें
hint-type = टाइप करें
hint-shift = Shift
hint-close = बंद करें
hint-done = हो गया
hint-options = विकल्प
hint-details = विवरण
hint-hide-details = विवरण छिपाएँ
hint-search = खोजें
hint-adjust = बदलें
hint-keep = रखें
hint-put-back = पहले जैसा करें
hint-cancel = रद्द करें

## The on-screen keyboard: what its face key turns to

board-letters = ABC
board-symbols = #+=
board-accents = àé

## Questions, asked with the safe answer first

quit-title = इंस्टॉलर से बाहर निकलें?
quit-body = इस कंप्यूटर पर कुछ भी बदला नहीं गया है।
quit-stay = यहीं रहें
quit-leave = बाहर निकलें
erase-title = मिटाकर इंस्टॉल करें?
erase-title-short = मिटाएँ?
install-title = अभी Linexin इंस्टॉल करें?
# The last question says one sentence for each thing that will be erased, then whether it can be undone.
erase-target-disk = { $disk } पर सब कुछ मिटा दिया जाएगा।
erase-target-part = { $disk } पर { $part } · { $size } मिटा दिया जाएगा।
erase-target-free = Linexin { $disk } की खाली जगह में जाएगा।
erase-home = { $part }, जहाँ आपकी फ़ाइलें रखी जाएँगी, मिटा दिया जाएगा।
erase-final = इसे वापस नहीं किया जा सकता।
erase-nothing = इस पर कुछ भी नहीं मिटेगा।
# The last question while the partitions made on Partitions are the choice
# in use: over the list of what the plan destroys (in GParted's words), and
# where it destroys nothing.
erase-plan = ये कार्य आपकी डिस्कों की सामग्री बदलते हैं:
erase-plan-nothing = आपकी डिस्कों पर कुछ भी नहीं मिटेगा।
erase-plan-erased = { $partition } ({ $filesystem }, { $size }) मिटाता है
erase-plan-esp = { $what } · EFI सिस्टम विभाजन
erase-plan-windows = { $what } · Windows
erase-plan-page = पृष्ठ { $page } / { $pages }
# A part of a disk called by the name it was given, in a sentence.
part-labelled = “{ $label }”
# A part of another disk than the one Linexin goes on, with that disk named.
part-on-disk = { $disk } पर { $part }
erase-back = वापस जाएँ
erase-confirm = मिटाकर इंस्टॉल करें
erase-confirm-short = मिटाएँ
button-next = आगे

## A line beside the legend

demo-note = एक काल्पनिक कंप्यूटर: यहाँ कुछ भी असली नहीं है, और कुछ नहीं बदलता।
install-unavailable = इस पर Linexin इंस्टॉल करने के लिए कंप्यूटर को Linexin पेन ड्राइव से चालू करें।
disks-unreadable = इस कंप्यूटर की डिस्क पढ़ी नहीं जा सकीं।
install-reading = इस कंप्यूटर की डिस्क अभी पढ़ी जा रही हैं। थोड़ी देर में फिर कोशिश करें।
installing-note = यह पूरा होने तक कंप्यूटर चालू रखें।
done-all-well = सब कुछ ठीक से हो गया।
done-warnings = { $count ->
    [one] Linexin इस्तेमाल के लिए तैयार है, लेकिन एक चीज़ ठीक से नहीं हुई:
   *[other] Linexin इस्तेमाल के लिए तैयार है, लेकिन कुछ चीज़ें ठीक से नहीं हुईं:
}
done-restart-failed = कंप्यूटर पुनरारंभ नहीं हो सका। सिस्टम मेनू से इसे पुनरारंभ करें।

## The fields

field-full-name = पूरा नाम
field-full-name-placeholder = आपका नाम, जैसा दूसरे देखेंगे
field-username = उपयोगकर्ता नाम
field-username-placeholder = जिससे आप साइन इन करते हैं
field-password = पासवर्ड
field-password-placeholder = एक पासवर्ड चुनें
field-repeat = पासवर्ड की पुष्टि करें
field-repeat-placeholder = वही पासवर्ड फिर से
field-hostname = कंप्यूटर का नाम
field-hostname-placeholder = नेटवर्क पर यह कंप्यूटर किस नाम से दिखे
field-root-password = प्रशासक पासवर्ड
field-root-password-placeholder = सिस्टम में बदलाव के लिए पासवर्ड
field-root-repeat = प्रशासक पासवर्ड की पुष्टि करें
# How strong a password looks, beside its field: advice, never a refusal.
strength-weak = कमज़ोर
strength-fair = ठीक-ठाक
strength-good = अच्छा
strength-strong = मज़बूत

## You

# A separate administrator (root) account, off unless asked for.
account-root = प्रशासक खाता
account-root-off = बंद: बदलाव आपके अपने पासवर्ड से होंगे
account-root-on = अपने अलग पासवर्ड के साथ, नीचे

## Sizes, as a box writes them

size-tb = { $amount } TB
size-gb = { $amount } GB
size-mb = { $amount } MB
size-kb = { $amount } kB
size-bytes = { $amount } बाइट

## Disks, and what is on them

# A disk on a row: its model, or what kind of disk it is, and its size.
disk-name = { $disk } · { $size }
disk-kind-nvme = तेज़ सॉलिड-स्टेट ड्राइव
disk-kind-sata = अंदरूनी ड्राइव
disk-kind-usb = USB ड्राइव
disk-kind-card = मेमोरी कार्ड
disk-kind-virtual = वर्चुअल डिस्क
disk-kind-other = डिस्क
disk-empty = खाली
disk-holds-more = { $first } और { $count ->
    [one] एक अन्य हिस्सा
   *[other] { $count } अन्य हिस्से
}
# What a disk holds where it was written over the whole of it, with no parts.
disk-holds-whole = { $what }, पूरी डिस्क पर

## What a part of a disk appears to hold

content-efi = स्टार्टअप फ़ाइलें
content-bios-boot = स्टार्टअप की जगह
content-reserved = Windows की रखी हुई जगह
content-recovery = Windows रिकवरी
content-windows = Windows
content-linux = Linux
content-swap = अतिरिक्त मेमोरी
content-data = फ़ाइलें
content-empty = खाली
content-other = कुछ और

## Why a disk cannot be chosen, said under it

blocked-installation-media = इंस्टॉलर इसी से चल रहा है
blocked-in-use = अभी उपयोग में है
blocked-swap = अतिरिक्त मेमोरी के रूप में उपयोग में है
blocked-holder = इस कंप्यूटर पर किसी और चीज़ के उपयोग में है
blocked-read-only = पढ़ी जा सकती है, पर इस पर लिखा नहीं जा सकता
blocked-too-small = Linexin के लिए बहुत छोटी है
blocked-unknown = इसका उपयोग नहीं हो सकता

## The review

# Each row names a step; under it, that step's answer.
review-where = कहाँ
review-desktop = डेस्कटॉप
review-region-name = भाषा और क्षेत्र
review-account-name = खाता
review-extras = अतिरिक्त
review-where-disk = { $disk } · इस पर सब कुछ मिट जाएगा
review-where-free = { $disk } · इसकी खाली जगह में
review-where-part = { $disk } · { $part } की जगह
# Beside where Linexin goes: the part the person's files will be on, and whether what is on it stays.
review-where-home-kept = आपकी फ़ाइलें { $part } पर, बिना मिटाए
review-where-home-erased = आपकी फ़ाइलें { $part } पर, मिटाकर
review-region = { $language } · { $city } · { $keyboard }
review-account = { $name } · { $username }
review-account-none = अभी नहीं चुना गया
review-extra-updates = अपडेट
review-extra-apps = { $count ->
    [one] एक ऐप
   *[other] { $count } ऐप
}
review-extra-snapshots = रीस्टोर पॉइंट
review-extra-swap = { $size } अतिरिक्त मेमोरी
review-extra-none = और कुछ नहीं
# Review's rows while the partitions made on Partitions are the choice in
# use: under each mount point the partition used there, its filesystem and
# its size — a partition the plan makes with the disk it is made on, one the
# disk has with whether it is formatted or kept — and how many operations
# will be applied. Devices, filesystems and sizes are passed as values.
review-part-new = { $partition } · { $filesystem } · { $size }, { $disk } पर
review-part-formatted = { $partition } · { $filesystem } · { $size } · फ़ॉर्मेट होगा
review-part-kept = { $partition } · { $filesystem } · { $size } · जैसा है वैसा
review-operations = { $count ->
    [one] एक कार्य लागू होगा
   *[other] { $count } कार्य लागू होंगे
}

## The installation, while it runs

installing-waiting = तैयारी हो रही है…
installing-count = { $steps } में से चरण { $step } · { $percent }%
# The first line of Details once an installation stopped: the step it was on, in the words of the step list below.
stopped-step = कहाँ रुका: { $step }
step-check = कंप्यूटर की जाँच हो रही है
step-resize = डिस्क पर बचे हिस्से छोटे किए जा रहे हैं
step-partition = डिस्क पर जगह बनाई जा रही है
step-format = डिस्क तैयार की जा रही है
step-copy = Linexin कॉपी हो रहा है
step-swap = अतिरिक्त मेमोरी अलग रखी जा रही है
step-system = सिस्टम सेट किया जा रहा है
step-region = भाषा, समय क्षेत्र और कीबोर्ड सेट किए जा रहे हैं
step-accounts = आपका खाता बनाया जा रहा है
step-desktop = डेस्कटॉप सेट किया जा रहा है
step-updates = अपडेट इंस्टॉल हो रहे हैं
step-packages = सॉफ़्टवेयर जोड़ा और हटाया जा रहा है
step-flatpaks = ऐप इंस्टॉल हो रहे हैं
step-drivers = ऐसे ड्राइवर हटाए जा रहे हैं जिनकी इस कंप्यूटर को ज़रूरत नहीं
step-snapshots = रीस्टोर पॉइंट सेट किए जा रहे हैं
step-initramfs = सिस्टम को शुरू होने के लिए तैयार किया जा रहा है
step-bootloader = कंप्यूटर को Linexin से शुरू होने लायक बनाया जा रहा है
step-finish = काम पूरा हो रहा है

## What did not work, said once Linexin is installed

warning-updates-offline = इंटरनेट न होने के कारण अपडेट छोड़ दिए गए। उन्हें बाद में इंस्टॉल किया जा सकता है।
warning-updates-failed = अपडेट इंस्टॉल नहीं हो सके। उन्हें बाद में इंस्टॉल किया जा सकता है।
warning-flatpaks-failed = कुछ ऐप इंस्टॉल नहीं हो सके। उन्हें बाद में जोड़ा जा सकता है।
warning-packages-kept = कुछ सॉफ़्टवेयर जिसे हटाना था, अभी भी मौजूद है।
warning-boot-entry = अगर Linexin शुरू न हो, तो कंप्यूटर के स्टार्टअप मेनू में उसे चुनें।
warning-snapshots = रीस्टोर पॉइंट सेट नहीं हो सके।
warning-drivers-kept = जिन ड्राइवरों की इस कंप्यूटर को ज़रूरत नहीं, वे वहीं छोड़ दिए गए।
warning-windows-check = अगली बार शुरू होने पर Windows अपनी डिस्क जाँचेगा। उसे पूरा होने दें — तब तक Linexin, Windows की फ़ाइलों को नहीं छूता।

## Why the installation stopped, and what can be done about it

failure-invalid-request = इंस्टॉलर ने कुछ ऐसा माँगा जो वह नहीं कर सकता
failure-invalid-request-do = इंस्टॉलर बंद करें, उसे फिर से खोलें और सभी चरण एक बार और पूरे करें।
failure-consent-mismatch = जो मिटाया जाना था, वह बदल गया
failure-consent-mismatch-do = कुछ भी नहीं मिटाया गया। चरण फिर से पूरे करें और डिस्क जाँच लें।
failure-not-root = इंस्टॉलर को अनुमति नहीं मिली
failure-not-root-do = इंस्टॉलर फिर से खोलें और पूछे जाने पर अनुमति दें।
failure-not-live = यह कंप्यूटर इंस्टॉलर से शुरू नहीं किया गया था
failure-not-live-do = कंप्यूटर को Linexin पेन ड्राइव से चालू करें और फिर कोशिश करें।
failure-secure-boot = Secure Boot चालू है
failure-secure-boot-do = कंप्यूटर की स्टार्टअप सेटिंग में Secure Boot बंद करें, फिर कोशिश करें।
failure-already-running = एक और इंस्टॉलेशन पहले से चल रहा है
failure-already-running-do = उसके पूरा होने तक रुकें, या कंप्यूटर पुनरारंभ करके फिर कोशिश करें।
failure-missing-tool = इंस्टॉलर को जिस चीज़ की ज़रूरत है, वह मौजूद नहीं है
failure-missing-tool-do = नई Linexin पेन ड्राइव इस्तेमाल करें और फिर कोशिश करें।
failure-image-invalid = पेन ड्राइव पर Linexin की कॉपी खराब है
failure-image-invalid-do = पेन ड्राइव फिर से बनाएँ और एक बार और कोशिश करें।
failure-disk-changed = चुनने के बाद डिस्क बदल गई
failure-disk-changed-do = हो सकता है डिस्क का कुछ हिस्सा पहले ही मिट चुका हो। चरण फिर से पूरे करें और डिस्क दोबारा चुनें।
failure-disk-changed-untouched-do = कुछ भी नहीं मिटाया गया। चरण फिर से पूरे करें और डिस्क दोबारा चुनें।
failure-disk-busy = डिस्क का उपयोग कोई और चीज़ कर रही है
failure-disk-busy-do = जो भी इसका उपयोग कर रहा है उसे बंद करें, या कंप्यूटर पुनरारंभ करें, और फिर कोशिश करें।
failure-disk-busy-untouched-do = कुछ भी नहीं मिटाया गया। जो भी डिस्क का उपयोग कर रहा है उसे बंद करें, या कंप्यूटर पुनरारंभ करें, और फिर कोशिश करें।
failure-no-space = पर्याप्त जगह नहीं है
failure-no-space-do = बड़ी डिस्क या ज़्यादा खाली जगह चुनें और फिर कोशिश करें।
failure-esp-full = डिस्क की स्टार्टअप जगह भर गई है
failure-esp-full-do = इसके बजाय पूरी डिस्क मिटाएँ, या पहले उसकी स्टार्टअप जगह खाली करें।
failure-shrink-refused = डिस्क का एक हिस्सा छोटा नहीं हो सका
failure-shrink-refused-do = डिस्क के सभी हिस्से अब भी मौजूद हैं, पर जो हिस्सा छोटा हो रहा था वह बदल गया हो सकता है। जिस सिस्टम का वह है, उसे इसकी जाँच करने दें, फिर कोशिश करें।
failure-shrink-refused-untouched-do = कुछ भी नहीं बदला गया। इंस्टॉलर फिर से खोलें और उस हिस्से के लिए ज़्यादा जगह छोड़ें, या उसे उसी सिस्टम से छोटा करें जिसका वह है।
failure-windows-hibernated = Windows पूरी तरह बंद नहीं हुआ है
failure-windows-hibernated-do = कुछ भी नहीं बदला गया। Windows शुरू करें, फ़ास्ट स्टार्टअप बंद करें, उसे बंद करें, फिर कोशिश करें।
failure-needs-check = पहले डिस्क के एक हिस्से की जाँच ज़रूरी है
failure-needs-check-do = डिस्क के सभी हिस्से अब भी मौजूद हैं, पर जो हिस्सा छोटा हो रहा था वह बदल गया हो सकता है। जिस सिस्टम का वह है उसे शुरू करें, उसे अपनी डिस्क जाँचने दें, फिर कोशिश करें।
failure-needs-check-untouched-do = कुछ भी नहीं बदला गया। जिस सिस्टम का वह हिस्सा है उसे शुरू करें, उसे अपनी डिस्क जाँचने दें, फिर कोशिश करें।
failure-windows-in-use = Windows वाली डिस्क का एक हिस्सा उपयोग में है
failure-windows-in-use-do = कुछ भी नहीं बदला गया। जिसने Windows की फ़ाइलें खोल रखी हैं उसे बंद करें, या कंप्यूटर पुनरारंभ करें, फिर कोशिश करें।
failure-windows-needs-check = Windows को पहले अपनी डिस्क जाँचनी है
failure-windows-needs-check-do = कुछ भी नहीं बदला गया। Windows शुरू करें, उसे अपनी डिस्क जाँचने दें, उसे बंद करें, फिर कोशिश करें।
failure-windows-unread = Windows वाली डिस्क का एक हिस्सा पढ़ा नहीं जा सका
failure-windows-unread-do = कुछ भी नहीं बदला गया। जिन डिस्क पर इंस्टॉल नहीं कर रहे उन्हें निकालें और फिर कोशिश करें, या Windows को Windows से ही छोटा करें।
failure-bitlocker = डिस्क का एक हिस्सा BitLocker से लॉक है
failure-bitlocker-do = कुछ भी नहीं बदला गया। Windows में BitLocker बंद करें और फिर कोशिश करें, या Windows को Windows से ही छोटा करें।
failure-needs-internet = इस विकल्प के लिए इंटरनेट चाहिए
failure-needs-internet-do = इंटरनेट से जुड़ें, या Linexin डेस्कटॉप चुनें, जिसे इंटरनेट नहीं चाहिए।
failure-needs-internet-untouched-do = कुछ भी नहीं मिटाया गया। इंटरनेट से जुड़ें, या Linexin डेस्कटॉप चुनें, जिसे इंटरनेट नहीं चाहिए।
failure-command = इंस्टॉल करते समय कुछ गड़बड़ हो गई
failure-command-do = विवरण देखें, फिर कोशिश करें। हो सकता है डिस्क को मिटाना पड़े।
failure-internal = इंस्टॉलर में कोई समस्या आ गई
failure-internal-do = इसे बंद करें और फिर कोशिश करें। अगर फिर से ऐसा हो, तो गड़बड़ी की रिपोर्ट के लिए विवरण सँभालकर रखें।

## Why a choice cannot be installed, said beside it

invalid-username-empty = एक उपयोगकर्ता नाम चुनें
invalid-username-shape = छोटे अंग्रेज़ी अक्षर, अंक, - और _, किसी अक्षर से शुरू
invalid-username-reserved = यह नाम सिस्टम का है
invalid-full-name = नाम में : या , नहीं हो सकता
invalid-hostname = अंग्रेज़ी अक्षर, अंक और -, ज़्यादा से ज़्यादा 63
invalid-password-empty = एक पासवर्ड चुनें
invalid-password-mismatch = दोनों पासवर्ड एक जैसे नहीं हैं
invalid-password-shape = ज़्यादा से ज़्यादा 128 अक्षर, बिना नई पंक्ति के
invalid-root-password-empty = प्रशासक खाते के लिए एक पासवर्ड चुनें
invalid-locale = एक भाषा चुनें
invalid-timezone = एक समय क्षेत्र चुनें
invalid-keyboard = एक कीबोर्ड चुनें
invalid-disk-blocked = इस डिस्क का उपयोग नहीं हो सकता
invalid-target-missing = चुनें कि कहाँ इंस्टॉल करना है
invalid-target-small = वहाँ Linexin के लिए पर्याप्त जगह नहीं है
table-full = इस डिस्क पर एक और हिस्से के लिए जगह नहीं है। इसके बजाय किसी एक की जगह लें।
invalid-target-esp = उसी हिस्से से कंप्यूटर शुरू होता है
target-protected = Windows यह हिस्सा अपने लिए रखता है
invalid-home = उस हिस्से में आपकी फ़ाइलें नहीं रखी जा सकतीं
invalid-swap = अतिरिक्त मेमोरी वहाँ की जगह से बड़ी है
invalid-bootloader = उस स्टार्टअप मैनेजर का इस कंप्यूटर पर उपयोग नहीं हो सकता
invalid-source = इंस्टॉल करने के लिए Linexin की कॉपी नहीं मिल रही
invalid-layout = आपके विभाजन योजना के अनुसार उपयोग नहीं किए जा सकते

## The time zone step

timezone-heading-region = दुनिया का हिस्सा
timezone-heading-city = सबसे पास का शहर
timezone-heading-found = मिली जगहें
timezone-found-note = { $place } · { $region }
# Under each part of the world: how many places it holds.
timezone-places = { $count ->
    [one] एक जगह
   *[other] { $count } जगहें
}
zone-region-africa = अफ़्रीका
zone-region-america = अमेरिका महाद्वीप
zone-region-antarctica = अंटार्कटिका
zone-region-arctic = आर्कटिक
zone-region-asia = एशिया
zone-region-atlantic = अटलांटिक महासागर
zone-region-australia = ऑस्ट्रेलिया
zone-region-europe = यूरोप
zone-region-indian = हिंद महासागर
zone-region-pacific = प्रशांत महासागर
zone-region-utc = विश्व समय
# The one place under Universal time: the clock no country keeps.
zone-utc = समन्वित विश्व समय

## The keyboard step

keyboard-heading-layout = लेआउट
keyboard-heading-variant = प्रकार
keyboard-heading-found = मिले प्रकार
# Under each layout: how many kinds of it there are besides the plain one.
keyboard-others = { $count ->
    [0] केवल मानक
    [one] मानक और एक अन्य
   *[other] मानक और { $count } अन्य
}
# Under the plain layout, first among its kinds.
keyboard-plain = मानक
keyboard-test = अपना कीबोर्ड आज़माएँ
keyboard-test-placeholder = कुंजियाँ आज़माने के लिए कुछ टाइप करें

## The search over a long list (Language, Time zone, Keyboard)

search-field = खोज
search-language-placeholder = कोई भाषा, उसके अपने शब्दों में या अंग्रेज़ी में
search-timezone-placeholder = कोई शहर, देश या दुनिया का हिस्सा
search-keyboard-placeholder = कोई लेआउट या उसका प्रकार
search-nothing = “{ $query }” के लिए कुछ नहीं मिला

## The step of where to install

button-options = विकल्प
target-heading-disks = डिस्क
target-erase = यह डिस्क मिटाएँ
target-erase-note = इस पर सब कुछ मिट जाएगा
target-free = खाली जगह · { $size }
target-free-note = डिस्क पर कुछ भी नहीं मिटेगा
# A part of the disk Linexin can take the place of: what it holds and its size.
target-replace = { $what } की जगह · { $size }
target-replace-note = इस पर जो है, वह मिट जाएगा
# The same part, called by the name it was given (passed as it is), and its size.
target-replace-labelled = “{ $label }” की जगह · { $size }
# Under a part called by its name: what it holds.
target-replace-holds = { $what } · मिट जाएगा
# A part of the disk in a menu: what it holds and its size, its name first where it has one.
target-part = { $what } · { $size }
target-part-named = “{ $label }” · { $what } · { $size }
target-key-linexin = Linexin
target-key-free = खाली जगह
target-no-disk = ऐसी कोई डिस्क नहीं है जिस पर Linexin इंस्टॉल हो सके।
target-options = विकल्प
# Each option with the state it is in.
target-option-storage = स्टोरेज · { $value }
target-option-swap = अतिरिक्त मेमोरी · { $value }
target-option-home = आपकी फ़ाइलें · { $value }
target-storage = स्टोरेज
target-storage-standard = मानक
target-storage-snapshots = रीस्टोर पॉइंट के साथ
target-storage-snapshots-short = रीस्टोर पॉइंट
target-swap = अतिरिक्त मेमोरी
target-swap-off = बंद
target-home = आपकी फ़ाइलें
target-home-system = सिस्टम के साथ
target-home-kept = { $part }, बिना मिटाए
target-home-erased = { $part }, मिटाकर
target-home-keep = इस पर जो है, उसे रखें
target-home-erase = इसे मिटाएँ
# Where to install's foot while the partitions made on Partitions are the
# choice in use, with how many operations they have; and once they were
# cleared because a disk they change was not as it was when read again.
target-using-partitions = आपके विभाजन इस्तेमाल हो रहे हैं · { $count ->
    [one] एक कार्य
   *[other] { $count } कार्य
}
target-partitions-let-go = एक डिस्क बदल गई, इसलिए आपके विभाजन हटा दिए गए। फिर से चुनें।

## विभाजन

button-advanced = उन्नत
partitions-title = विभाजन
partitions-not-yet = इस संस्करण में विभाजन अभी लागू नहीं किए जा सकते।
partitions-not-yet-fresh-table = इस संस्करण में नई विभाजन तालिका अभी लागू नहीं की जा सकती।
partitions-not-yet-keep-table = इस संस्करण में विभाजन बनाना या हटाना अभी लागू नहीं किया जा सकता।
partitions-not-yet-change-entries = इस संस्करण में विभाजन फ़ॉर्मेट करना अभी लागू नहीं किया जा सकता।
partitions-not-yet-kept-home = इस संस्करण में /home रखना अभी लागू नहीं किया जा सकता।
partitions-not-yet-several-disks = इस संस्करण में कई डिस्कों पर योजना अभी लागू नहीं की जा सकती।
partitions-not-yet-bios = इस संस्करण में BIOS वाले कंप्यूटर पर विभाजन अभी लागू नहीं किए जा सकते।
partitions-not-yet-shrink = इस संस्करण में विभाजन छोटा करना अभी लागू नहीं किया जा सकता।
partitions-unallocated = अनाबंटित
partitions-unknown = अज्ञात
partitions-no-table = कोई विभाजन तालिका नहीं
partitions-partition = विभाजन
partitions-filesystem = फ़ाइल सिस्टम
partitions-label = लेबल
partitions-size = आकार
partitions-used = प्रयुक्त
partitions-mount-point = माउंट बिंदु
partitions-flags = फ़्लैग
partitions-pending = लंबित
partitions-start = आरंभ
partitions-end = अंत
partitions-reading = पढ़ा जा रहा है…
partitions-menu-mount = माउंट बिंदु · { $value }
partitions-menu-format = फ़ॉर्मेट · { $value }
partitions-format-as = फ़ॉर्मेट
partitions-none = कोई नहीं
partitions-keep = रखें
partitions-delete = हटाएँ
partitions-undo-part = { $partition } के बदलाव पूर्ववत करें
partitions-undo = बदलाव पूर्ववत करें
partitions-create-table = विभाजन तालिका बनाएँ
partitions-create-table-kind = { $table } विभाजन तालिका बनाएँ
partitions-table = विभाजन तालिका
partitions-undo-last = आख़िरी कार्य पूर्ववत करें
partitions-clear = सभी कार्य हटाएँ
partitions-pending-list = लंबित कार्य ({ $count })…
partitions-swap-file = स्वैप फ़ाइल · { $value }
partitions-swap-file-title = स्वैप फ़ाइल
partitions-pending-title = लंबित कार्य
partitions-pending-short = लंबित
partitions-pending-none = कोई लंबित कार्य नहीं
partitions-pending-earlier = { $count ->
    [one] एक पहले का कार्य
   *[other] { $count } पहले के कार्य
}
partitions-tile-format = फ़ॉर्मेट
partitions-tile-deleted = { $count ->
   *[other] { $partitions } हटाया गया
}
partitions-do-format = { $filesystem } में फ़ॉर्मेट
partitions-do-mount = { $path } पर माउंट
partitions-do-delete = { $partition } हटाएँ
partitions-op-delete = { $partition } हटाएँ ({ $filesystem }, { $size })
partitions-op-format = { $partition } को { $filesystem } में फ़ॉर्मेट करें
partitions-op-mount = { $partition } का माउंट बिंदु { $path } रखें
partitions-op-table = { $disk } पर { $table } विभाजन तालिका बनाएँ
partitions-op-create = { $disk } पर प्राथमिक विभाजन #{ $number } ({ $filesystem }, { $size }) बनाएँ
partitions-op-create-at = { $disk } पर प्राथमिक विभाजन #{ $number } ({ $filesystem }, { $size }, { $path }) बनाएँ
partitions-op-shrink = { $partition } को { $from } से { $to } तक छोटा करें
partitions-op-grow = { $partition } को { $from } से { $to } तक बड़ा करें
partitions-op-label = { $partition } के फ़ाइल सिस्टम का लेबल “{ $label }” रखें
partitions-op-label-clear = { $partition } के फ़ाइल सिस्टम का लेबल हटाएँ
partitions-missing-root = कमी: / पर माउंट किया गया विभाजन
partitions-missing-efi = कमी: /boot या /efi पर EFI विभाजन
partitions-missing-boot = /boot जोड़ें, या EFI विभाजन को /boot पर माउंट करें
partitions-missing-bios-grub = कमी: { $disk } पर एक bios_grub विभाजन
partitions-too-small = { $path } के लिए कम से कम { $size } चाहिए
partitions-must-format = { $path } पर उपयोग के लिए { $partition } को फ़ॉर्मेट करें
partitions-needs = { $path } के लिए { $filesystem } चाहिए
partitions-needs-linux = { $path } के लिए ext4, btrfs या xfs चाहिए
partitions-needs-root = { $path } के लिए ext4 या btrfs चाहिए
partitions-twice = { $path } एक से अधिक विभाजनों पर सेट है
partitions-blocked = { $disk } पर कुछ भी नहीं बदला जा सकता
partitions-refused-mismatch = { $disk } की योजना डिस्क से मेल नहीं खाती
partitions-refused-unaligned = { $partition } को पूरे MiB पर शुरू और खत्म होना चाहिए
partitions-refused-outside = { $partition } अपनी डिस्क की जगह से बाहर जाता है
partitions-refused-overlap = { $partition } दूसरे विभाजन पर चढ़ता है
partitions-refused-beside-extended = { $disk } पर विस्तारित विभाजन के पास कुछ नहीं बन सकता
partitions-refused-logical = पहले { $partition } के बाद के लॉजिकल विभाजन हटाएँ
partitions-refused-entries = { $disk } पर 128 से अधिक विभाजन होंगे
partitions-refused-shrink = { $partition } को छोटा नहीं किया जा सकता
partitions-refused-grow = { $partition } रखा जा रहा है, इसलिए बड़ा नहीं हो सकता
partitions-refused-bios-grub = { $partition } यहाँ bios_grub विभाजन नहीं हो सकता
partitions-refused-label = { $partition } का लेबल उसके फ़ाइल सिस्टम में नहीं आता
partitions-refused-unmountable = { $partition } पर { $filesystem } माउंट नहीं हो सकता
partitions-refused-inside-kept = { $path } रखा जा रहा है: इसके अंदर कुछ माउंट नहीं हो सकता
partitions-refused-fs-small = { $partition } { $filesystem } के लिए बहुत छोटा है
partitions-refused-restore-points = अलग /var के साथ रीस्टोर पॉइंट नहीं रखे जा सकते
partitions-refused-boot-disk = /boot उसी डिस्क पर होना चाहिए जिस पर { $path } है
partitions-refused-efi-bios = /efi तभी उपयोग होता है जब कंप्यूटर UEFI से चालू हो
partitions-refused-grub-gap = GRUB को { $disk } का पहला MiB खाली चाहिए
partitions-refused-bootloader = { $loader } इन विभाजनों से Linexin शुरू नहीं कर सकता
partitions-refused-not-read = { $partition } में कितना भरा है, यह अभी पता नहीं
partitions-refused-shrink-below = { $partition } { $size } से छोटा नहीं हो सकता
partitions-refused-hibernated = { $partition } छोटा नहीं हो सकता: Windows हाइबरनेट है
partitions-refused-needs-check = जाँच के बाद ही { $partition } छोटा हो सकता है
partitions-refused-encrypted = { $partition } एन्क्रिप्टेड है, इसलिए छोटा नहीं हो सकता
partitions-refused-in-use = { $partition } उपयोग में है, इसलिए छोटा नहीं हो सकता
partitions-refused-unreadable = { $partition } पढ़ा नहीं जा सकता, इसलिए छोटा नहीं हो सकता
partitions-refused-no-hiberfile = { $partition }: इसे यहाँ नहीं, Windows में छोटा करें
partitions-refused-windows-unread = छोटा नहीं हो सकता: { $other } का Windows अपठनीय
partitions-refused-bitlocker = छोटा नहीं हो सकता: { $other } BitLocker से लॉक है
partitions-why-hibernated = { $other } पर Windows को फ़ास्ट स्टार्टअप के बिना बंद करें
partitions-why-windows-unread = { $other } पर Windows पढ़े जाने तक छोटा नहीं हो सकता
partitions-why-bitlocker = छोटा करने के लिए { $other } पर BitLocker बंद करें
partitions-why-no-hiberfile = छोटा करने के लिए Windows में डिस्क प्रबंधन का उपयोग करें
partitions-refused-esp-full = { $partition } में { $size } खाली जगह चाहिए
partitions-refused-image-unknown = Linexin { $path } में कितना रखता है, यह अभी पता नहीं
partitions-refused-unread = { $partition } में कितना भरा है, यह पढ़ा नहीं जा सका
partitions-refused-image-unread = Linexin { $path } में कितना रखता है, यह पढ़ा नहीं जा सका
partitions-new = नया…
partitions-resize = आकार बदलें…
partitions-relabel = लेबल…
partitions-other = अन्य…
partitions-new-name = नया विभाजन #{ $number }
partitions-tile-new = नया
partitions-new-title = नया विभाजन
partitions-resize-title = { $partition } का आकार बदलें
partitions-label-title = { $partition } का लेबल
partitions-other-title = { $partition } का माउंट बिंदु
partitions-new-size = नया आकार
partitions-position = स्थिति
partitions-position-start = स्थान का आरंभ
partitions-position-end = स्थान का अंत
partitions-bios-grub = bios_grub (बिना फ़ॉर्मेट)
partitions-cancel = रद्द करें
partitions-add = जोड़ें
partitions-resize-button = आकार बदलें
partitions-set = सेट करें
partitions-leaves = इसके बाद { $size } अनाबंटित रहता है
partitions-leaves-none = इसके बाद कोई अनाबंटित स्थान नहीं रहता
partitions-takes = इसके बाद के अनाबंटित स्थान में से { $size } लेता है
partitions-takes-all = इसके बाद का सारा अनाबंटित स्थान लेता है
partitions-at-least = कम से कम { $size } चाहिए
partitions-path-placeholder = उदाहरण के लिए /data
partitions-path-whole = माउंट बिंदु / से शुरू होने वाला पूरा पथ होता है, जैसे /data
partitions-path-twice = { $path } पहले से एक माउंट बिंदु है
partitions-path-system = { $path } सिस्टम का हिस्सा है
partitions-path-long = माउंट बिंदु अधिकतम 255 बाइट का होता है
partitions-path-spaces = माउंट बिंदु में स्पेस या बैकस्लैश नहीं होते
partitions-path-under = { $path } के अंदर कुछ भी माउंट नहीं किया जा सकता
partitions-label-bytes = { $filesystem } लेबल में अधिकतम { $count ->
   *[other] { $count } बाइट
}
partitions-label-chars = { $filesystem } लेबल में अधिकतम { $count ->
   *[other] { $count } अक्षर
}
partitions-label-slash = लेबल में “/” नहीं हो सकता
partitions-why-table = अभी कोई विभाजन तालिका नहीं: बनाने के लिए यह स्थान चुनें
partitions-why-primaries = msdos डिस्क में अधिकतम चार प्राथमिक विभाजन होते हैं
partitions-why-logical = यहाँ तार्किक विभाजन नहीं बनाए जा सकते
partitions-why-beside-extended = विस्तारित विभाजन के पास कुछ नहीं बनाया जा सकता
partitions-why-whole = पूरी डिस्क पर कुछ लिखा है: तालिका या विभाजन नहीं बनाया जा सकता
partitions-why-unmountable = { $filesystem } माउंट नहीं हो सकता: उपयोग के लिए फ़ॉर्मेट करें
partitions-why-unformatted = कोई फ़ाइल सिस्टम नहीं: उपयोग के लिए फ़ॉर्मेट करें
partitions-why-logical-delete = पहले इसके बाद की संख्या वाले लॉजिकल विभाजन हटाएँ
partitions-primaries = { $disk } पर चार से अधिक प्राथमिक विभाजन हो जाएँगे
partitions-do-create = बनाएँ
partitions-do-shrink = { $size } तक छोटा करें
partitions-do-grow = { $size } तक बड़ा करें

## The desktop step

button-done = हो गया
desktop-linexin = Linexin
desktop-kinexin = Kinexin
desktop-linexinbar = LineXinBar
desktop-about-linexin = GNOME पर बना
desktop-about-kinexin = KDE Plasma पर बना
desktop-about-linexinbar = कंट्रोलर के लिए बना
# Under a desktop: whether it has to be downloaded.
desktop-ready = इंटरनेट की ज़रूरत नहीं
desktop-needs-internet = इंटरनेट चाहिए
desktop-refused = { $desktop } के लिए इंटरनेट चाहिए। इंटरनेट से जुड़ें, या Linexin चुनें।
desktop-checking = इंटरनेट कनेक्शन की जाँच हो रही है…
desktop-checking-short = इंटरनेट की जाँच हो रही है…
desktop-offline = इंटरनेट नहीं है, इसलिए जिसे इसकी ज़रूरत है वह छोड़ दिया गया है।
desktop-updates = अपडेट इंस्टॉल करें
desktop-updates-on = सब कुछ अप-टू-डेट होगा
desktop-updates-off = जैसा पेन ड्राइव पर है
updates-refused = अपडेट के लिए इंटरनेट चाहिए। उन्हें बाद में इंस्टॉल किया जा सकता है।
desktop-options-title = विकल्प
desktop-options-caption = और क्या इंस्टॉल होगा, और कंप्यूटर कैसे शुरू होगा।
desktop-option-remove = हटाने के लिए सॉफ़्टवेयर
# The heads of the pages of Options, short enough to stand beside the step's dots.
desktop-remove-title = सॉफ़्टवेयर
desktop-flatpaks-title = ऐप
desktop-bootloader-title = स्टार्टअप
desktop-remove-caption = जो आप इंस्टॉल किए गए सिस्टम पर नहीं चाहते, उसे बंद करें।
desktop-remove-looking = सॉफ़्टवेयर देखा जा रहा है…
desktop-remove-unlisted = सॉफ़्टवेयर की सूची नहीं बन सकी
desktop-remove-none = हटाने के लिए कुछ नहीं है
desktop-remove-nothing = कुछ भी नहीं हटाया जाएगा
desktop-remove-count = { $count ->
    [one] एक छोड़ा गया
   *[other] { $count } छोड़े गए
}
desktop-option-flatpaks = Flathub से ऐप
desktop-flatpaks-caption = Linexin के साथ इंटरनेट से इंस्टॉल होंगे।
# How many of the apps offered are switched on.
desktop-flatpaks-count = { $total } में से { $count } चुने गए
desktop-flatpak-on = Linexin के साथ इंस्टॉल होगा
desktop-flatpak-off = छोड़ दिया गया
flatpaks-refused = Flathub के ऐप के लिए इंटरनेट चाहिए। उन्हें बाद में जोड़ा जा सकता है।
desktop-option-bootloader = स्टार्टअप मैनेजर
desktop-bootloader-caption = कंप्यूटर चालू होने पर Linexin को क्या शुरू करेगा।
desktop-option-drivers = अनुपयोगी ड्राइवर हटाएँ
desktop-drivers-on = अनुपयोगी ड्राइवर हटाए जाएँगे
desktop-drivers-off = हर ड्राइवर रखा जाएगा
desktop-option-snapshots = रीस्टोर पॉइंट
desktop-snapshots-on = हर अपडेट से पहले सहेजे जाएँगे
desktop-snapshots-off = कोई नहीं सहेजा जाएगा
# Under Restore points while the storage chosen under Where to install keeps none.
desktop-snapshots-needs = “कहाँ इंस्टॉल करें” में सेट करें
desktop-snapshots-var = अलग /var के साथ नहीं
bootloader-automatic = स्वचालित
bootloader-automatic-note = इस कंप्यूटर के हिसाब से चुना जाएगा
bootloader-systemd-boot = systemd-boot
bootloader-systemd-boot-note = बिना मेनू के, सीधे Linexin शुरू करता है
bootloader-grub = GRUB
bootloader-grub-note = शुरू होते समय एक मेनू, किसी भी कंप्यूटर पर
bootloader-refind = rEFInd
bootloader-refind-note = कंप्यूटर के हर सिस्टम का एक मेनू
bootloader-unavailable = इस कंप्यूटर पर उपयोग नहीं हो सकता
# The groups the software to remove is listed in.
package-group-applications = ऐप
package-group-multimedia = ऑडियो और वीडियो
package-group-network = नेटवर्क
package-group-fonts = फ़ॉन्ट
package-group-libraries = लाइब्रेरी
