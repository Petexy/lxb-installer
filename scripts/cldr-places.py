#!/usr/bin/env python3
"""Write data/places/names.txt: Time zone's places and their countries in
every language the Language step offers, as CLDR names them.

    python3 -I scripts/cldr-places.py

Run by hand when tzdata or ICU has moved on, and the result committed: the
installer carries the names in its binary (`model::places`), so nothing of
this runs when it is built or used, and nothing on the live medium is asked.

Where the names come from: CLDR's exemplar cities (the name a language
gives a time zone's city: Warszawa, Нью-Йорк, 纽约) and its territory names,
as this machine's ICU gives them — a date format with the pattern "VVV" for
a city, `uloc_getDisplayCountry` for a country — and the order each
language sorts them in, by ICU's collator for that language. The zones are
those of tzdata's zone.tab (and zone1970.tab's few more), the languages
those of `LANGUAGES` in src/model/catalogs.rs.

Reads only: /usr/share/zoneinfo, src/model/catalogs.rs and ICU's libraries
(libicui18n, libicuuc, through ctypes; no headers, no compiler). Writes
data/places/names.txt and nothing else.

The file, line by line (UTF-8):

    # comments
    [zones]             every zone, sorted, a line each: its tz name and
                        the code of its country, a tab between; a zone is
                        named below by its place here, from 0
    [en]                the base: English
    12                  a zone, in the order English sorts them
    13<TAB>Knox, Indiana    ... and its name, where tzdata's city differs
    AR<TAB>Argentina    a country by its code, every one zone.tab names
    [pl_PL]             a language, by its locale without the encoding
    301<TAB>Warszawa    every zone, in the order Polish sorts them, the name
                        given where it differs from the base's
    PL<TAB>Polska       a country, where it differs from the base's
    [en_US=en]          a language that says everything as another does

A name is a language's own where CLDR has one; elsewhere ICU falls back as
any program asking it would (to the parent language, then to the city in
the tz name), and that fallback is what is written.
"""

import ctypes
import glob
import os
import re
import sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ZONEINFO = "/usr/share/zoneinfo"
OUT = os.path.join(HERE, "data", "places", "names.txt")


def icu():
    """ICU's two libraries and the suffix its symbols carry (`_78`)."""
    found = sorted(
        glob.glob("/usr/lib/libicui18n.so.[0-9]*"),
        key=lambda path: [int(part) for part in path.split(".so.")[1].split(".")],
    )
    if not found:
        sys.exit("cldr-places: no libicui18n.so.N under /usr/lib")
    major = found[-1].split(".so.")[1].split(".")[0]
    i18n = ctypes.CDLL(f"libicui18n.so.{major}")
    common = ctypes.CDLL(f"libicuuc.so.{major}")
    return i18n, common, f"_{major}"


I18N, COMMON, SUFFIX = icu()


def function(library, name, restype, argtypes):
    found = getattr(library, name + SUFFIX)
    found.restype = restype
    found.argtypes = argtypes
    return found


Error = ctypes.c_int
P = ctypes.c_void_p
udat_open = function(
    I18N,
    "udat_open",
    P,
    [ctypes.c_int, ctypes.c_int, ctypes.c_char_p, P, ctypes.c_int32, P,
     ctypes.c_int32, ctypes.POINTER(Error)],
)
udat_format = function(
    I18N,
    "udat_format",
    ctypes.c_int32,
    [P, ctypes.c_double, P, ctypes.c_int32, P, ctypes.POINTER(Error)],
)
udat_close = function(I18N, "udat_close", None, [P])
ucol_open = function(I18N, "ucol_open", P, [ctypes.c_char_p, ctypes.POINTER(Error)])
ucol_getSortKey = function(
    I18N, "ucol_getSortKey", ctypes.c_int32, [P, P, ctypes.c_int32, P, ctypes.c_int32]
)
ucol_close = function(I18N, "ucol_close", None, [P])
uloc_getDisplayCountry = function(
    COMMON,
    "uloc_getDisplayCountry",
    ctypes.c_int32,
    [ctypes.c_char_p, ctypes.c_char_p, P, ctypes.c_int32, ctypes.POINTER(Error)],
)
u_getVersion = function(COMMON, "u_getVersion", None, [P])
ulocdata_getCLDRVersion = function(
    I18N, "ulocdata_getCLDRVersion", None, [P, ctypes.POINTER(Error)]
)

UDAT_PATTERN = -2


def utf16(text):
    raw = text.encode("utf-16-le") + b"\0\0"
    return ctypes.create_string_buffer(raw, len(raw)), len(text.encode("utf-16-le")) // 2


def failed(error):
    return error.value > 0


def version(call, *more):
    info = (ctypes.c_uint8 * 4)()
    call(info, *more)
    parts = list(info)
    while len(parts) > 2 and parts[-1] == 0:
        parts.pop()
    return ".".join(str(part) for part in parts)


def city(locale, zone):
    """The city a language names `zone` by: ICU's "VVV"."""
    error = Error(0)
    zone16, zone_len = utf16(zone)
    pattern, pattern_len = utf16("VVV")
    formatter = udat_open(
        UDAT_PATTERN, UDAT_PATTERN, locale.encode(), zone16, zone_len, pattern,
        pattern_len, ctypes.byref(error),
    )
    if failed(error) or not formatter:
        sys.exit(f"cldr-places: no date format for {locale} {zone}: {error.value}")
    out = ctypes.create_string_buffer(1024)
    error = Error(0)
    length = udat_format(formatter, 0.0, out, 512, None, ctypes.byref(error))
    udat_close(formatter)
    if failed(error):
        sys.exit(f"cldr-places: {locale} {zone} not formatted: {error.value}")
    return out.raw[: length * 2].decode("utf-16-le")


def country(locale, code):
    """The name a language gives the country `code`, or None."""
    out = ctypes.create_string_buffer(1024)
    error = Error(0)
    length = uloc_getDisplayCountry(
        f"und_{code}".encode(), locale.encode(), out, 512, ctypes.byref(error)
    )
    if failed(error):
        return None
    name = out.raw[: length * 2].decode("utf-16-le")
    return None if name == code else name


class Collator:
    """Sort keys of a language's collation."""

    def __init__(self, locale):
        error = Error(0)
        self.handle = ucol_open(locale.encode(), ctypes.byref(error))
        if failed(error) or not self.handle:
            sys.exit(f"cldr-places: no collator for {locale}: {error.value}")

    def key(self, text):
        text16, length = utf16(text)
        size = ucol_getSortKey(self.handle, text16, length, None, 0)
        out = ctypes.create_string_buffer(size)
        ucol_getSortKey(self.handle, text16, length, out, size)
        return out.raw

    def close(self):
        ucol_close(self.handle)


def zones():
    """Every zone of zone.tab and zone1970.tab, with its country's code."""
    found = {}
    for name in ("zone.tab", "zone1970.tab"):
        with open(os.path.join(ZONEINFO, name), encoding="utf-8") as file:
            for line in file:
                if line.startswith("#") or not line.strip():
                    continue
                fields = line.rstrip("\n").split("\t")
                found.setdefault(fields[2], fields[0].split(",")[0])
    return sorted(found.items())


def locales():
    """The locales `LANGUAGES` offers, without the encoding, in its order."""
    with open(os.path.join(HERE, "src", "model", "catalogs.rs"), encoding="utf-8") as file:
        source = file.read()
    start = source.index("static LANGUAGES")
    end = source.index("\n];", start)
    found = re.findall(r'"([a-z]{2,3}_[A-Z]{2})\.UTF-8"', source[start:end])
    if len(found) != len(set(found)) or not found:
        sys.exit("cldr-places: LANGUAGES could not be read")
    return found


def tz_city(zone):
    """The city as the installer reads it out of the tz name."""
    return zone.rsplit("/", 1)[-1].replace("_", " ")


def tzdata_version():
    with open(os.path.join(ZONEINFO, "tzdata.zi"), encoding="utf-8") as file:
        first = file.readline()
    return first.removeprefix("# version").strip().removesuffix("-dirty")


def section(locale, listed, base_names, base_countries, codes):
    """A language's lines: its zones in its own order, its countries."""
    unknown = city(locale, "Etc/Unknown")
    names = {}
    for zone, _ in listed:
        name = city(locale, zone)
        names[zone] = base_names.get(zone, tz_city(zone)) if name == unknown else name
    collator = Collator(locale)
    order = sorted(
        range(len(listed)),
        key=lambda at: (collator.key(names[listed[at][0]]), listed[at][0]),
    )
    collator.close()
    lines = []
    for at in order:
        zone = listed[at][0]
        shown = base_names.get(zone, tz_city(zone))
        if names[zone] == shown:
            lines.append(f"{at}")
        else:
            lines.append(f"{at}\t{names[zone]}")
    for code in codes:
        name = country(locale, code)
        if name is not None and name != base_countries.get(code):
            lines.append(f"{code}\t{name}")
    return names, lines


def main():
    listed = zones()
    codes = sorted({code for _, code in listed})
    for name, _ in listed:
        if not re.fullmatch(r"[A-Za-z0-9_+\-/]+", name):
            sys.exit(f"cldr-places: a zone named {name!r}")
    # The base: English, in its own order, each name that differs from the
    # tz name's city, and every country.
    english, base_lines = section("en", listed, {}, {}, [])
    base_names = {zone: name for zone, name in english.items() if name != tz_city(zone)}
    english_countries = {code: country("en", code) or code for code in codes}
    bare = [line.split("\t")[0] for line in base_lines]
    base_lines += [f"{code}\t{english_countries[code]}" for code in codes]

    out = [
        "# Time zone's places and countries in the languages the Language step",
        "# offers: CLDR's exemplar cities and territory names, in the order each",
        "# language sorts them. Made by scripts/cldr-places.py, never by hand.",
        f"# CLDR {version(ulocdata_getCLDRVersion, ctypes.byref(Error(0)))}"
        f" as ICU {version(u_getVersion)} gives it; tzdata {tzdata_version()}'s zones.",
        "# The names are CLDR's: Unicode License v3, data/places/LICENSE.",
        "[zones]",
    ]
    out += [f"{name}\t{code}" for name, code in listed]
    out.append("[en]")
    out += base_lines
    written = {}
    for locale in locales():
        _, lines = section(locale, listed, base_names, english_countries, codes)
        same = "en" if lines == bare else None
        same = same or next((other for other, theirs in written.items() if theirs == lines), None)
        if same is not None:
            out.append(f"[{locale}={same}]")
            continue
        out.append(f"[{locale}]")
        out += lines
        written[locale] = lines
    text = "\n".join(out) + "\n"
    with open(OUT + ".new", "w", encoding="utf-8") as file:
        file.write(text)
    os.replace(OUT + ".new", OUT)
    print(f"cldr-places: {len(listed)} zones, {len(codes)} countries, "
          f"{len(locales())} languages, {len(text.encode())} bytes -> {OUT}")


if __name__ == "__main__":
    main()
