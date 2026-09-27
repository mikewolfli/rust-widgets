# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT

"""Shows what the flat 0.6 em estimate does to a real word.

Run: python3 tools/word_spacing_probe.py
"""

UPEM = 2048
SIZE = 14.0
ESTIMATE = 0.6 * SIZE  # what the crate reserves for every Latin cluster

# Open Sans advance widths, from the face's own `hmtx`.
OPEN_SANS = {
    "w": 1500, "i": 517, "d": 1253, "g": 1130, "e": 1150, "t": 800,
    "W": 1600, "I": 570, "D": 1400, "G": 1350, "E": 1200, "T": 1100,
    " ": 560, "m": 1896, "l": 517, "a": 1138, "n": 1150, "r": 800, "o": 1130,
}


def px(units: int) -> float:
    return units / UPEM * SIZE


def report(word: str) -> None:
    print(f"\n=== {word!r} (14 px) ===")
    print(f"{'ch':>3} {'real(px)':>9} {'estimate':>9} {'drift':>8}   {'pen by real':>11} {'pen by estimate':>16}")
    pen_real = 0.0
    pen_est = 0.0
    for ch in word:
        real = px(OPEN_SANS.get(ch, 1000))
        print(
            f"{ch:>3} {real:>9.2f} {ESTIMATE:>9.2f} {real - ESTIMATE:>8.2f}"
            f"   {pen_real:>11.2f} {pen_est:>16.2f}"
        )
        pen_real += real
        pen_est += ESTIMATE
    print(f"{'tot':>3} {pen_real:>9.2f} {pen_est:>9.2f}   (real width of the word vs reserved)")


for word in ["widget", "Sample", "minimum", "ill"]:
    report(word)
