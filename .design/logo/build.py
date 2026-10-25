#!/usr/bin/env python3
"""Regenerate mission-control's logo SVGs with the letters outlined, so the files need no font.

    # Inter is OFL: https://github.com/rsms/inter (Debian/Ubuntu: fonts-inter)
    python3 -m pip install fonttools   # if missing
    python3 .design/logo/build.py [/path/to/Inter-SemiBold.otf]

It writes wordmark.svg, wordmark-dark.svg, mark.svg, mark-light.svg and favicon.svg next to this file.
Change the brackets, tiles or colors here, never by hand-editing the SVGs. Keep colors in sync with
../tokens.css (the signal is the warn state tone: something waits on you).

The mark is the beacon: a watched signal in brackets. The page's favicon recolors the signal with the
mission's health tone at runtime (see ../components.md, "Attention signals").
"""
import os
import sys

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_FONT = "/usr/share/fonts/opentype/inter/Inter-SemiBold.otf"
INK, PAPER = "#141414", "#f7f7f5"
# (bracket stroke, signal) per tile.
ON_INK = ("#ededea", "#e0a84a")
ON_PAPER = (INK, "#8a5a0c")
# Brackets inside a 64-unit tile, and the signal at the center.
BRACKETS = "M22 17h-6v30h6M42 17h6v30h-6"


def outline(font, text, size, track):
    """SVG path data for `text` with its baseline at y=0 (y down)."""
    gs, cmap, hmtx = font.getGlyphSet(), font.getBestCmap(), font["hmtx"]
    s, x, pen = size / font["head"].unitsPerEm, 0.0, SVGPathPen(None)
    for ch in text:
        g = cmap[ord(ch)]
        gs[g].draw(TransformPen(pen, (s, 0, 0, -s, x, 0)))
        x += hmtx[g][0] * s + track
    return pen.getCommands(), x


def beacon(colors, width, dot, halo=True):
    stroke, signal = colors
    glow = f'\n  <circle cx="32" cy="32" r="10.5" fill="{signal}" fill-opacity="0.22"/>' if halo else ""
    return (f'\n  <path d="{BRACKETS}" stroke="{stroke}" stroke-width="{width}" stroke-linecap="round" '
            f'stroke-linejoin="round" fill="none"/>{glow}\n  <circle cx="32" cy="32" r="{dot}" fill="{signal}"/>')


def mark(tile, colors, title, width=3.2, dot=5.5, halo=True):
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="mission">
  <title>{title}</title>
  <rect width="64" height="64" rx="14" fill="{tile}"/>{beacon(colors, width, dot, halo)}
</svg>
"""


def wordmark(word, ink, tile, colors, title):
    d, advance = word
    w = round(80 + advance + 8)  # tile, gap, the letters, a little air
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} 64" width="{w}" height="64" role="img" aria-label="mission-control">
  <title>{title}</title>
  <rect width="64" height="64" rx="14" fill="{tile}"/>{beacon(colors, 4, 7, halo=False)}
  <path transform="translate(80 46)" fill="{ink}" d="{d}"/>
</svg>
"""


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_FONT
    if not os.path.isfile(path):
        sys.exit(f"font not found: {path}\n\n{__doc__}")
    word = outline(TTFont(path), "mission-control", 44, -0.5)
    files = {
        "wordmark.svg": wordmark(word, INK, INK, ON_INK, "mission-control wordmark, for light backgrounds"),
        "wordmark-dark.svg": wordmark(word, PAPER, "#232325", ON_INK, "mission-control wordmark, for dark backgrounds"),
        "mark.svg": mark(INK, ON_INK, "mission-control mark (ink tile)"),
        "mark-light.svg": mark(PAPER, ON_PAPER, "mission-control mark (paper tile), for dark UIs"),
        "favicon.svg": mark(INK, ON_INK, "mission-control favicon (16-24 px)", width=6, dot=9, halo=False),
    }
    for name, svg in files.items():
        with open(os.path.join(HERE, name), "w") as f:
            f.write(svg)
        print(name)


if __name__ == "__main__":
    main()
