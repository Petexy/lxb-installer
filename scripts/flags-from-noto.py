#!/usr/bin/env python3
"""The Language step's flags, cut out of Noto Color Emoji.

The earlier installer wrote each language's flag as an emoji before its name
(`language_widget.py`, `country_code_to_emoji`), which GNOME drew from Noto
Color Emoji. The installer draws the same flags, but as pictures it carries
(`data/flags/`, `src/ui/flags.rs`): the toolkit's text renderer does draw a
colour emoji from that font, but at full strength whatever the words around
it are faded to, so a flag drawn as a letter stood alone on a page still
coming in.

This writes `data/flags/<cc>.png` for each country code named on the command
line, or, with none, for every flag already there — the waved flag of the
font's 109 ppem strike (136x128), cut to the box every waved flag of the font
stands in (126x95), so the flag itself is as tall as the picture — and
`white.png`, the white flag the earlier installer showed for a locale naming
no country, cut to its own bounds. Pixel for pixel the font's, re-encoded.

    python3 -I scripts/flags-from-noto.py                # every flag again
    python3 -I scripts/flags-from-noto.py MX AR          # two more
    python3 -I scripts/flags-from-noto.py --font FILE    # another copy

Needs fontTools and Pillow (python-fonttools, python-pillow), at build time
only: the program carries the result. The font is under the SIL Open Font
License 1.1, which `data/flags/LICENSE` carries.
"""

import argparse
import io
import pathlib
import sys

from fontTools.ttLib import TTFont
from PIL import Image

HERE = pathlib.Path(__file__).resolve().parent.parent
FLAGS = HERE / "data" / "flags"
FONT = "/usr/share/fonts/noto/NotoColorEmoji.ttf"
# Where every waved flag of the font stands in its 136x128 picture, measured
# on 2.051: (left, top, right, bottom). Hungary's shadow reaches its last row.
WAVED = (5, 17, 131, 112)
WHITE_FLAG = 0x1F3F3


def ligatures(font):
    """Every ligature of the font, by the glyphs it is made of."""
    found = {}
    for lookup in font["GSUB"].table.LookupList.Lookup:
        for table in lookup.SubTable:
            if table.LookupType == 7:
                table = table.ExtSubTable
            for first, ligs in getattr(table, "ligatures", {}).items():
                for lig in ligs:
                    found[tuple([first] + lig.Component)] = lig.LigGlyph
    return found


def picture(font, glyph):
    """The PNG the font's colour strike carries for `glyph`."""
    data = font["CBDT"].strikeData[0][glyph].imageData
    return Image.open(io.BytesIO(data))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("codes", nargs="*", help="ISO 3166 country codes")
    parser.add_argument("--font", default=FONT)
    args = parser.parse_args()

    codes = [code.upper() for code in args.codes] or sorted(
        path.stem.upper() for path in FLAGS.glob("*.png") if path.stem != "white"
    )
    font = TTFont(args.font)
    cmap = font.getBestCmap()
    joined = ligatures(font)
    FLAGS.mkdir(parents=True, exist_ok=True)

    def regional(letter):
        return cmap[0x1F1E6 + ord(letter) - ord("A")]

    for code in codes:
        if len(code) != 2 or not code.isascii() or not code.isalpha():
            sys.exit(f"flags-from-noto: {code!r} is not a two-letter country code")
        glyph = joined.get((regional(code[0]), regional(code[1])))
        if glyph is None:
            sys.exit(f"flags-from-noto: the font has no flag for {code}")
        whole = picture(font, glyph)
        left, top, right, bottom = whole.convert("RGBA").getchannel("A").getbbox()
        if left < WAVED[0] or top < WAVED[1] or right > WAVED[2] or bottom > WAVED[3]:
            sys.exit(f"flags-from-noto: {code}'s flag reaches past {WAVED}")
        cut = whole.crop(WAVED)
        cut.save(FLAGS / f"{code.lower()}.png", "PNG", optimize=True)
    white = picture(font, cmap[WHITE_FLAG])
    white.crop(white.convert("RGBA").getchannel("A").getbbox()).save(
        FLAGS / "white.png", "PNG", optimize=True
    )
    print(f"flags-from-noto: {len(codes)} flags and the white flag in {FLAGS}")


if __name__ == "__main__":
    main()
