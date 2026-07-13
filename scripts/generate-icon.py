#!/usr/bin/env python3
"""Generate a simple SessionDex app icon source PNG.

The output is intentionally dependency-free so contributors can regenerate the
Tauri icon set without installing image tooling.
"""

from __future__ import annotations

import math
import struct
import sys
import zlib
from pathlib import Path


SIZE = 1024


def clamp(value: float, minimum: float = 0.0, maximum: float = 1.0) -> float:
    return max(minimum, min(maximum, value))


def mix(left: int, right: int, amount: float) -> int:
    return round(left + (right - left) * amount)


def blend_pixel(
    pixels: bytearray,
    x: int,
    y: int,
    color: tuple[int, int, int, int],
) -> None:
    if x < 0 or y < 0 or x >= SIZE or y >= SIZE:
        return

    index = (y * SIZE + x) * 4
    source_alpha = color[3] / 255.0
    destination_alpha = pixels[index + 3] / 255.0
    output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha)

    if output_alpha <= 0:
        return

    for channel in range(3):
        source = color[channel] / 255.0
        destination = pixels[index + channel] / 255.0
        output = (
            source * source_alpha
            + destination * destination_alpha * (1.0 - source_alpha)
        ) / output_alpha
        pixels[index + channel] = round(output * 255)

    pixels[index + 3] = round(output_alpha * 255)


def rounded_rect_alpha(
    x: int,
    y: int,
    left: float,
    top: float,
    width: float,
    height: float,
    radius: float,
) -> float:
    px = x + 0.5 - (left + width / 2.0)
    py = y + 0.5 - (top + height / 2.0)
    qx = abs(px) - (width / 2.0 - radius)
    qy = abs(py) - (height / 2.0 - radius)
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    inside = min(max(qx, qy), 0.0)
    distance = outside + inside - radius
    return clamp(0.5 - distance)


def draw_rounded_rect(
    pixels: bytearray,
    left: int,
    top: int,
    width: int,
    height: int,
    radius: int,
    color: tuple[int, int, int, int],
) -> None:
    for y in range(max(0, top - 2), min(SIZE, top + height + 2)):
        for x in range(max(0, left - 2), min(SIZE, left + width + 2)):
            alpha = rounded_rect_alpha(x, y, left, top, width, height, radius)
            if alpha <= 0:
                continue
            blend_pixel(
                pixels,
                x,
                y,
                (color[0], color[1], color[2], round(color[3] * alpha)),
            )


def draw_gradient_background(pixels: bytearray) -> None:
    left, top, width, height, radius = 32, 32, 960, 960, 210
    for y in range(SIZE):
        for x in range(SIZE):
            alpha = rounded_rect_alpha(x, y, left, top, width, height, radius)
            if alpha <= 0:
                continue

            linear = (x * 0.65 + y * 0.35) / SIZE
            r = mix(15, 30, linear)
            g = mix(23, 64, linear)
            b = mix(42, 175, linear)

            spotlight = clamp(1.0 - math.hypot(x - 760, y - 230) / 760)
            r = mix(r, 45, spotlight * 0.45)
            g = mix(g, 212, spotlight * 0.35)
            b = mix(b, 191, spotlight * 0.3)

            blend_pixel(pixels, x, y, (r, g, b, round(255 * alpha)))


def draw_shadow(
    pixels: bytearray,
    left: int,
    top: int,
    width: int,
    height: int,
    radius: int,
) -> None:
    for spread, opacity in [(30, 28), (20, 34), (10, 42)]:
        draw_rounded_rect(
            pixels,
            left - spread // 2,
            top + 28 - spread // 2,
            width + spread,
            height + spread,
            radius + spread // 2,
            (2, 6, 23, opacity),
        )


def distance_to_segment(
    px: float,
    py: float,
    ax: float,
    ay: float,
    bx: float,
    by: float,
) -> float:
    dx = bx - ax
    dy = by - ay
    length_squared = dx * dx + dy * dy
    if length_squared == 0:
        return math.hypot(px - ax, py - ay)

    t = clamp(((px - ax) * dx + (py - ay) * dy) / length_squared)
    closest_x = ax + t * dx
    closest_y = ay + t * dy
    return math.hypot(px - closest_x, py - closest_y)


def draw_line(
    pixels: bytearray,
    start: tuple[int, int],
    end: tuple[int, int],
    width: int,
    color: tuple[int, int, int, int],
) -> None:
    left = min(start[0], end[0]) - width
    right = max(start[0], end[0]) + width
    top = min(start[1], end[1]) - width
    bottom = max(start[1], end[1]) + width

    for y in range(max(0, top), min(SIZE, bottom)):
        for x in range(max(0, left), min(SIZE, right)):
            distance = distance_to_segment(x + 0.5, y + 0.5, *start, *end)
            alpha = clamp((width / 2.0 + 0.75) - distance)
            if alpha <= 0:
                continue
            blend_pixel(
                pixels,
                x,
                y,
                (color[0], color[1], color[2], round(color[3] * alpha)),
            )


def draw_icon(pixels: bytearray) -> None:
    draw_gradient_background(pixels)

    draw_rounded_rect(pixels, 252, 220, 538, 420, 62, (148, 163, 184, 118))
    draw_rounded_rect(pixels, 210, 280, 604, 430, 68, (203, 213, 225, 172))

    draw_shadow(pixels, 166, 342, 692, 400, 78)
    draw_rounded_rect(pixels, 166, 342, 692, 400, 78, (248, 250, 252, 255))
    draw_rounded_rect(pixels, 232, 300, 210, 90, 36, (226, 232, 240, 255))

    draw_rounded_rect(pixels, 228, 646, 110, 28, 14, (203, 213, 225, 255))
    draw_rounded_rect(pixels, 374, 646, 110, 28, 14, (203, 213, 225, 255))
    draw_rounded_rect(pixels, 520, 646, 110, 28, 14, (203, 213, 225, 255))

    draw_line(pixels, (292, 478), (368, 532), 34, (15, 23, 42, 255))
    draw_line(pixels, (292, 586), (368, 532), 34, (15, 23, 42, 255))
    draw_line(pixels, (438, 546), (642, 546), 34, (20, 184, 166, 255))
    draw_line(pixels, (438, 618), (586, 618), 20, (100, 116, 139, 255))


def png_bytes(pixels: bytearray) -> bytes:
    rows = []
    stride = SIZE * 4
    for y in range(SIZE):
        rows.append(b"\x00" + bytes(pixels[y * stride : (y + 1) * stride]))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
        + chunk(b"IEND", b"")
    )


def main() -> int:
    output = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/sessiondex-icon.png")
    output.parent.mkdir(parents=True, exist_ok=True)
    pixels = bytearray(SIZE * SIZE * 4)
    draw_icon(pixels)
    output.write_bytes(png_bytes(pixels))
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

