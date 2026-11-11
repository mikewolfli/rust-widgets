#!/usr/bin/env python3
"""Generate the four sample photographs the image gallery's snapshot shows.

# Why generated rather than downloaded

The repository ships no photographic assets, and the build environment has no network
access. The gallery's snapshot used to draw flat placeholder wells, which taught a reviewer
nothing about the image paint path. These scenes are built from fixed seeds so the result is
reproducible: running this script twice writes byte-identical JPEGs, which is what lets the
SVG snapshot stay a reviewable artifact.

They are **synthetic scenes**, not camera photographs. Replacing the four files under
`src/widget/sample_photos/` with real photographs of the same names needs no code change:
`sample_fill` decodes whatever bytes are there.

Usage (from the repository root):

    python3 tools/make_sample_photos.py
"""

from __future__ import annotations

import pathlib

import numpy as np
from PIL import Image, ImageFilter

WIDTH, HEIGHT = 240, 180
OUTPUT = pathlib.Path(__file__).resolve().parent.parent / "src" / "widget" / "sample_photos"


def value_noise(rng: np.random.Generator, width: int, height: int, cells: int) -> np.ndarray:
    """Bilinearly upsampled value noise in `0..1`."""
    coarse = rng.random((max(2, cells), max(2, cells))).astype(np.float32)
    image = Image.fromarray((coarse * 255).astype(np.uint8), mode="L")
    return np.asarray(image.resize((width, height), Image.BICUBIC), dtype=np.float32) / 255.0


def fbm(rng: np.random.Generator, width: int, height: int, octaves: int = 4) -> np.ndarray:
    """Sum of octaves, each twice as fine and half as strong."""
    total = np.zeros((height, width), dtype=np.float32)
    weight = 0.0
    gain = 1.0
    cells = 3
    for _ in range(octaves):
        total += gain * value_noise(rng, width, height, cells)
        weight += gain
        gain *= 0.5
        cells *= 2
    return total / weight


def vertical_ramp(stops: list[tuple[float, tuple[int, int, int]]], height: int = HEIGHT) -> np.ndarray:
    """A `height` x 1 RGB ramp through `stops` (position 0..1, colour)."""
    positions = np.asarray([stop[0] for stop in stops], dtype=np.float32)
    colours = np.asarray([stop[1] for stop in stops], dtype=np.float32)
    samples = np.linspace(0.0, 1.0, height, dtype=np.float32)
    channels = [np.interp(samples, positions, colours[:, index]) for index in range(3)]
    return np.stack(channels, axis=1).astype(np.float32)


def tint(ramp: np.ndarray, width: int) -> np.ndarray:
    """Broadcast a vertical ramp to `height x width x 3`."""
    return np.repeat(ramp[:, None, :], width, axis=1)


def finish(rgb: np.ndarray, rng: np.random.Generator, vignette: float = 0.22) -> Image.Image:
    """Film grain, vignette and a soft warm cast, then quantise."""
    height, width, _ = rgb.shape
    grain = (rng.standard_normal((height, width, 1)) * 5.0).astype(np.float32)
    yy, xx = np.mgrid[0:height, 0:width].astype(np.float32)
    dx = (xx - width / 2.0) / (width / 2.0)
    dy = (yy - height / 2.0) / (height / 2.0)
    falloff = 1.0 - vignette * np.clip(dx * dx + dy * dy, 0.0, 1.6) / 1.6
    shaded = (rgb + grain) * falloff[:, :, None]
    return Image.fromarray(np.clip(shaded, 0, 255).astype(np.uint8), mode="RGB")


def coast(rng: np.random.Generator) -> Image.Image:
    horizon = int(HEIGHT * 0.56)
    sky = tint(
        vertical_ramp(
            [
                (0.0, (24, 42, 78)),
                (0.45, (120, 96, 130)),
                (0.78, (236, 142, 92)),
                (1.0, (255, 206, 138)),
            ],
            horizon,
        ),
        WIDTH,
    )
    sun_x, sun_y = WIDTH * 0.62, horizon * 0.92
    yy, xx = np.mgrid[0:horizon, 0:WIDTH].astype(np.float32)
    distance = np.sqrt((xx - sun_x) ** 2 + (yy - sun_y) ** 2)
    glow = np.clip(1.0 - distance / (WIDTH * 0.45), 0.0, 1.0) ** 2
    sky += glow[:, :, None] * np.asarray([180, 120, 40], dtype=np.float32)
    sun = np.clip(1.0 - distance / 11.0, 0.0, 1.0)[:, :, None]
    sky = sky * (1 - sun) + np.asarray([255, 240, 205], dtype=np.float32) * sun

    sea = tint(
        vertical_ramp([(0.0, (92, 74, 96)), (0.4, (44, 58, 86)), (1.0, (14, 24, 44))], HEIGHT - horizon),
        WIDTH,
    )
    waves = fbm(rng, WIDTH, HEIGHT - horizon, octaves=5)
    sea += ((waves - 0.5) * 26.0)[:, :, None]
    sea_y, sea_x = np.mgrid[0 : HEIGHT - horizon, 0:WIDTH].astype(np.float32)
    reflection = np.clip(1.0 - np.abs(sea_x - sun_x) / 26.0, 0.0, 1.0)
    reflection = reflection * np.clip(1.0 - sea_y / (HEIGHT - horizon), 0.0, 1.0)
    sea += (reflection * 70.0)[:, :, None] * (0.5 + waves)[:, :, None]

    rgb = np.concatenate([sky, sea], axis=0)
    image = finish(rgb, rng)
    return image.filter(ImageFilter.SMOOTH)


def forest(rng: np.random.Generator) -> Image.Image:
    rgb = tint(
        vertical_ramp(
            [
                (0.0, (154, 188, 112)),
                (0.35, (74, 120, 66)),
                (0.72, (34, 72, 42)),
                (1.0, (14, 34, 22)),
            ]
        ),
        WIDTH,
    )
    noise = fbm(rng, WIDTH, HEIGHT, octaves=5)
    rgb += ((noise - 0.5) * 34.0)[:, :, None]
    # Trunks: a few vertical bands with their own bark texture.
    for _ in range(9):
        x = int(rng.integers(0, WIDTH))
        width = int(rng.integers(4, 11))
        lean = int(rng.integers(-6, 7)) * (HEIGHT // HEIGHT)
        bark = fbm(rng, width, HEIGHT, octaves=3)
        for row in range(HEIGHT):
            shift = int(lean * row / HEIGHT)
            start = max(0, x + shift)
            end = min(WIDTH, start + width)
            if start >= end:
                continue
            shade = 30 + bark[row, 0] * 26.0
            rgb[row, start:end] = rgb[row, start:end] * 0.35 + np.asarray(
                [shade * 0.9, shade * 0.75, shade * 0.6], dtype=np.float32
            )
    # Light shafts through the canopy.
    yy, xx = np.mgrid[0:HEIGHT, 0:WIDTH].astype(np.float32)
    for offset in (0.18, 0.42, 0.67, 0.86):
        centre = WIDTH * offset + (yy - HEIGHT * 0.2) * 0.55
        shaft = np.clip(1.0 - np.abs(xx - centre) / 16.0, 0.0, 1.0)
        shaft *= np.clip(1.0 - yy / HEIGHT, 0.0, 1.0) * 0.45
        rgb += shaft[:, :, None] * np.asarray([120, 130, 80], dtype=np.float32)
    return finish(rgb, rng)


def city(rng: np.random.Generator) -> Image.Image:
    sky = tint(
        vertical_ramp(
            [
                (0.0, (8, 12, 30)),
                (0.55, (22, 30, 64)),
                (0.8, (70, 58, 96)),
                (1.0, (150, 96, 96)),
            ]
        ),
        WIDTH,
    )
    stars = rng.random((HEIGHT, WIDTH)) > 0.995
    sky[stars] = np.asarray([235, 235, 220], dtype=np.float32)

    # Skyline: rectangles of varying height, lit windows, then a water reflection.
    baseline = int(HEIGHT * 0.66)
    for _ in range(18):
        width = int(rng.integers(8, 24))
        x = int(rng.integers(-4, WIDTH))
        top = int(rng.integers(int(HEIGHT * 0.22), baseline))
        shade = float(rng.integers(8, 22))
        x0, x1 = max(0, x), min(WIDTH, x + width)
        if x0 >= x1:
            continue
        sky[top:baseline, x0:x1] = np.asarray([shade, shade + 2, shade + 12], dtype=np.float32)
        for window_y in range(top + 3, baseline - 2, 4):
            for window_x in range(x0 + 2, x1 - 1, 3):
                if rng.random() < 0.35:
                    glow = np.asarray([255, 214, 130], dtype=np.float32)
                    sky[window_y : window_y + 2, window_x : window_x + 2] = glow

    water = sky[baseline:].copy()
    water = np.flipud(water)
    ripple = fbm(rng, WIDTH, HEIGHT - baseline, octaves=4)
    water += ((ripple - 0.5) * 46.0)[:, :, None]
    water *= 0.72
    rgb = np.concatenate([sky[:baseline], water], axis=0)
    return finish(rgb, rng)


def desert(rng: np.random.Generator) -> Image.Image:
    horizon = int(HEIGHT * 0.5)
    sky = tint(
        vertical_ramp(
            [
                (0.0, (58, 78, 128)),
                (0.5, (168, 140, 150)),
                (0.85, (232, 168, 118)),
                (1.0, (250, 208, 150)),
            ],
            horizon,
        ),
        WIDTH,
    )
    yy, xx = np.mgrid[0:HEIGHT, 0:WIDTH].astype(np.float32)
    sun = np.clip(1.0 - np.sqrt((xx - WIDTH * 0.32) ** 2 + (yy - horizon * 0.7) ** 2) / 22.0, 0, 1)
    sky += sun[:horizon, :, None] * np.asarray([110, 80, 20], dtype=np.float32)

    # Three dune bands, each a smooth ridge with a lit and a shaded side. The buffer starts
    # as the back dune's own colour so no band leaves a black gap at the horizon.
    dunes = np.tile(
        np.asarray([198, 146, 92], dtype=np.float32), (HEIGHT - horizon, WIDTH, 1)
    )
    for index, (base, amplitude, colour) in enumerate(
        [
            (0.02, 0.10, (198, 146, 92)),
            (0.40, 0.14, (168, 116, 70)),
            (0.74, 0.18, (124, 82, 52)),
        ]
    ):
        ridge = base * (HEIGHT - horizon)
        wave = np.sin(np.linspace(0, np.pi * (1.4 + index * 0.5), WIDTH) + index) * amplitude
        wave += (fbm(rng, WIDTH, HEIGHT - horizon, octaves=3).mean(axis=0) - 0.5) * 0.04
        for column in range(WIDTH):
            top = int(np.clip(ridge + wave[column] * (HEIGHT - horizon), 0, HEIGHT - horizon))
            lit = np.clip(1.2 - column / WIDTH, 0.6, 1.15)
            dunes[top:, column] = np.asarray(colour, dtype=np.float32) * lit
            dunes[top, column] = np.asarray(colour, dtype=np.float32) * 1.22
    rgb = np.concatenate([sky, dunes], axis=0)
    return finish(rgb, rng)


def main() -> None:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    scenes = {
        "coast.jpg": (coast, 11),
        "forest.jpg": (forest, 23),
        "city.jpg": (city, 37),
        "desert.jpg": (desert, 51),
    }
    for name, (render, seed) in scenes.items():
        image = render(np.random.default_rng(seed))
        path = OUTPUT / name
        image.save(path, format="JPEG", quality=86, optimize=False)
        print(f"wrote {path} ({path.stat().st_size} bytes, {image.size[0]}x{image.size[1]})")


if __name__ == "__main__":
    main()
