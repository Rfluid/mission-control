#!/usr/bin/env python3
"""Check mission-control's design tokens: WCAG contrast of the pairs the page uses, and that every copy of the
tokens (tokens.css, web/style.css) matches tokens.json.

    python3 .design/check.py        # prints every pair; exits 1 on a failure or a mismatch

Text pairs need 4.5:1. Glyph-only pairs (marked "glyph": dots, borders that carry meaning) need 3:1.
Add a pair here when the page starts using a new combination.
"""
import json, os, re, sys

D = os.path.dirname(os.path.abspath(__file__))
TONES = ["todo", "prog", "rev", "block", "done", "warn"]
# (fg, bg, kind): kind is "text" (4.5) or "glyph" (3.0)
PAIRS = ([("ink", "bg", "text"), ("ink", "panel", "text"), ("ink2", "panel", "text"), ("ink2", "bg", "text"),
          ("ink3", "bg", "text"), ("ink3", "panel", "text"), ("focus", "bg", "glyph"), ("focus", "panel", "glyph"),
          ("ink", "sunk", "text"), ("ink2", "sunk", "text"), ("ink3", "sunk", "text")]  # wells: rec box, current phase
         + [(t, f"{t}-bg", "text") for t in TONES]          # pills, banners, the need recommendation box
         + [("ink", f"{t}-bg", "text") for t in TONES]       # body text inside tinted boxes
         + [(t, "panel", "glyph") for t in TONES])           # health dot, need/banner left border
# Files that carry a copy of the tokens: (path from .design/, [(theme, block start, block end)]).
COPIES = [
    ("tokens.css", [("light", ":root {", ":root[data-theme"), ("dark", ':root[data-theme="dark"]', "@media"),
                    ("dark", ':root:not([data-theme="light"])', "\n  }")]),
    ("../web/style.css", [("light", ":root {", ":root[data-theme"), ("dark", ':root[data-theme="dark"]', "@media"),
                          ("dark", ':root:not([data-theme="light"])', "\n  }")]),
]


def rgb(s):
    s = s.strip()
    return tuple(int(s[i:i + 2], 16) for i in (1, 3, 5))


def luminance(h):
    f = lambda v: v / 12.92 if v <= .04045 else ((v + .055) / 1.055) ** 2.4
    r, g, b = (f(v / 255) for v in rgb(h))
    return .2126 * r + .7152 * g + .0722 * b


def ratio(a, b):
    x, y = sorted([luminance(a), luminance(b)])
    return (y + .05) / (x + .05)


def main():
    t = json.load(open(os.path.join(D, "tokens.json")))["color"]
    bad = 0
    for th in ("light", "dark"):
        c = t[th]
        for f, b, kind in PAIRS:
            r, need = ratio(c[f], c[b]), 4.5 if kind == "text" else 3.0
            ok = r >= need; bad += not ok
            print(f"{'ok  ' if ok else 'FAIL'} {th:5} {f} on {b} ({kind}): {r:.2f}")
    for path, blocks in COPIES:
        text = open(os.path.join(D, path)).read()
        for th, start, end in blocks:
            if start not in text:
                bad += 1; print(f"MISSING {path}: no `{start}` block"); continue
            i = text.index(start)
            blk = text[i:text.index(end, i + len(start))]
            found = dict(re.findall(r"--([a-z0-9-]+):\s*([^;]+);", blk))
            for k, v in t[th].items():
                if k not in found:
                    bad += 1; print(f"MISSING {path} {th} --{k}")
                elif v.lower() != found[k].strip().lower():
                    bad += 1; print(f"MISMATCH {path} {th} --{k}: {found[k].strip()} (tokens.json: {v})")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
