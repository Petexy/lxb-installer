# Languages

[Project home](../README.md) · [Design](design.md)

The installer speaks the ten languages LineXinBar does: **German**, **English
(UK)**, **English (US)**, **Spanish**, **French**, **Hindi**, **Polish**,
**Brazilian Portuguese**, **Russian** and **Simplified Chinese**. All of them
are compiled into the binary, and so are the faces they are written in: a live
medium has few fonts of its own, and there is nothing to install, no message
directory and no font to find.

Those ten are the installer's own words. The **sixty-five languages** on the
Language screen are a different list: the languages the installed system can be
set to, each in its own name, written into its `locale.gen` and `locale.conf`.
Most of them have no catalog here, and choosing one is still a complete answer
for the installed system.

## Which language it speaks

**At first, the session's**, decided as it starts: the first of `LC_ALL`,
`LC_MESSAGES` and `LANG` that says anything names it. `pl_PL.UTF-8` and `pl-PL`
alike are Polish, `fr_CA` is French, `es_MX` Spanish, `de_AT` German, `pt_PT`
reads the Brazilian catalog and `zh_TW` the Simplified one, and anything else —
including `C`, `POSIX` and an empty environment — is British English.

**Then, the person's.** This is the one place the installer differs from the
family's other applications, which keep the language they started in. The
first thing a person tells an installer is which language they read, so when
the language chosen on the Language screen is one the ten catalogs speak, every
screen after it is in that language at once — the headline, the buttons, the
legend, the questions, the progress and Done. A language no catalog speaks
leaves the installer in the one it was speaking, as the earlier installer did.
The time zone and keyboard are guessed from the same choice until the person
picks them.

**Time zone's places follow the language chosen**, whichever of the
sixty-five it is, even one no catalog speaks: Italian chosen, the installer
goes on in the language it was speaking and Time zone says Varsavia and
Polonia. Those names are not in the catalogs: they are CLDR's, carried in
`data/places/names.txt` (`scripts/cldr-places.py` makes it from ICU), so no
translator writes a city. **Welcome's greeting** is not in them either: it
takes turns in the earlier installer's twenty languages, each in its own
words (`src/ui/screens/welcome.rs`), whichever language is in force, starting
on the person's own.

What stays in the session's language is the toolkit's own words, of which the
installer raises none.

## Where the words are

* `locales/en-GB.ftl` — the whole catalog, and the fallback every other
  language falls back to. A message missing from a translation is answered from
  here; one missing from here would be drawn as its own identifier.
* `locales/en-US.ftl` — an **overlay** of what America writes differently, and
  nothing else: a message there must be one `en-GB.ftl` has, take the same
  variables, and actually say something else. Everything not in it is answered
  from `en-GB.ftl`.
* `locales/de.ftl`, `es.ftl`, `fr.ftl`, `hi.ftl`, `pl.ftl`, `pt-BR.ftl`,
  `ru.ftl`, `zh-CN.ftl` — the eight translations, each complete.
* `src/i18n.rs` — the catalogs it embeds, the current language, and `text`,
  `message!` and `decimal`.
* `data/lxb-installer.desktop` (`Name[xx]`, `GenericName[xx]`, `Comment[xx]`),
  the AppStream metadata and the polkit actions' descriptions and messages carry
  their own translations, because a menu, a software centre and the password
  question read those files and not the catalogs.

**Nothing the worker says is in a language.** The worker runs as root and
reports steps, warnings and failures as identifiers; the interface words them
from the catalogs in whatever language it is speaking. The worker's log lines
are technical and appear only behind Details and in the log file.

**Names are values, never messages.** A disk's model, a partition's label, a
person's name and a computer's name are passed into a message as values and
drawn as they are; a disk whose model happens to be `{ $size }` is still a disk.
A count is passed as a number, so each language chooses its own plural form —
Polish and Russian tell one, few and many apart, Chinese has a single form.

**Decimal marks follow the installer's language**, not the session's: German,
Spanish, French, Polish, Portuguese and Russian write "1,5 TB", the two
Englishes, Hindi and Chinese "1.5 TB".

## What holds them together

Four kinds of unit test read every catalog:

* `the_catalogs_carry_the_same_messages` — every language carries every
  message with the same variables, every one formats, and the overlay obeys
  its three rules.
* `every_message_this_program_asks_for_is_one_the_catalogs_have` — every
  message id the source asks for exists, so no identifier can reach the screen.
* `every_word_the_catalogs_carry_can_be_drawn` — every line of every catalog is
  shaped through the faces the toolkit ships, and the first character none of
  them has fails the test. A Hindi or Chinese label that would come out as a row
  of empty boxes on a live medium fails here instead.
* `src/ui/fit.rs` — every word of every catalog measured against the room it
  is drawn in on the narrowest panel the installer is laid out for (a 16:9
  display standing on its side, 1080×1920; narrower than a handheld's 800×1280)
  and across a landscape one, by the shaper the toolkit's renderer measures
  with and with no GPU, its lines broken as the drawing breaks them (between
  Chinese characters too): a button's word, a menu row, the last question's
  title and answers, the legend, a row's name on at most two lines and its
  note on at most three, a title and a caption on at most two, a field's label
  and placeholder, a card's words. A translation that would be cut fails,
  naming the language, the place and the words, as does a line wider than its
  room (a word with nowhere to break).

**Words fit or say less, never run out.** On screen nothing runs past its
button, row, card, menu or field: a note too long for its line wraps and its
row grows, a long title takes a second line, and on a narrow dialog the erase
answer is `erase-confirm-short`. The Options menu says each option's setting
beside its name in every language: the toolkit's menu grows to its widest
row, and the test holds every row, with every setting it can say, to the
widest menu a handheld standing on its side allows — and, on the narrowest
display, to `target-storage-snapshots-short`, which the row says there in
place of "with restore points". The test holds Time zone's places and
countries in each catalog's language too, and Welcome's twenty greetings and
their button. Only what a person, a disk or a file named —
a disk's model, a partition's label — and, on a display standing on its
side, a long place or country on a row the light is not on, is ever cut, with
an ellipsis, and a
name in the last question is never broken across two of its lines. So keep a translation about as long as English: where it has to
be longer, `cargo test fit` says whether it still fits.

## Seeing another language

`--shot` and `--demo` take the language from the environment like the window
does, so a picture of any screen in any of the ten needs only `LC_ALL`:

```sh
LC_ALL=pl_PL.UTF-8 lxb-installer --shot review-pl.png --scene review
LC_ALL=hi_IN.UTF-8 lxb-installer --shot target-hi.png --scene target
LC_ALL=en_US.UTF-8 lxb-installer --demo
```

Those locales need not be generated on the machine: the catalog is chosen by
the name, not by libc. The pictures in the README are taken with
`LC_ALL=en_GB.UTF-8`, so they do not depend on the language of whoever takes
them; the commands are in [design.md](design.md#the-pictures).

## Adding a language

Copy `locales/en-GB.ftl` to `<tag>.ftl`, translate the values, and add it to
`RESOURCES` in `src/i18n.rs`; the language must also be one
`lxb_toolkit::i18n::LANGUAGES` knows, so that a session locale finds it, and its
script must be one the toolkit ships a face for. Translate the desktop entry,
the AppStream metadata and both polkit actions. Keep every identifier and variable
name as English has them, put the variables in the order the language wants,
give it as many plural forms as it has, and write in plain words: what a person
reads says what they can do, never a device, a filesystem or a command.
