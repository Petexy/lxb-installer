# The Language step's flags

Each language on the Language step wears its country's flag, as the earlier
installer showed one before each name (an emoji drawn from Noto Color Emoji).
These are those flags: the waved flags of **Noto Color Emoji 2.051**
(`NotoColorEmoji.ttf`, Arch's `noto-fonts-emoji 1:2.051-1`, its 109 ppem
colour strike), cut by `scripts/flags-from-noto.py` to the box every waved
flag of the font stands in (126×95), pixel for pixel the font's. `white.png`
is the font's white flag, for a locale that names no country, as the earlier
installer gave one.

`<cc>.png` is the flag of the ISO 3166 country `CC`. The program carries
them (`src/ui/flags.rs`) and draws them as pictures; nothing here is
installed beside it.

Noto Color Emoji is Copyright 2013 Google LLC (its name table says 2022
Google Inc.), licensed under the SIL Open Font License, Version 1.1, which
[LICENSE](LICENSE) carries whole and under which these cuts of it are
distributed. The font reserves no name. The flag designs themselves come to
the font from the region-flags project, whose flags are in the public domain.
