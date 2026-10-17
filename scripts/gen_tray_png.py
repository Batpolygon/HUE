"""Generate icons/hue-tray.png (32x32) matching the HUE mascot."""
from __future__ import annotations

import struct
import zlib
from pathlib import Path

W = H = 32


def main() -> None:
    buf = [(0, 0, 0, 0)] * (W * H)

    def px(x: int, y: int, r: int, g: int, b: int, a: int = 255) -> None:
        if 0 <= x < W and 0 <= y < H:
            buf[y * W + x] = (r, g, b, a)

    def fill_rect(
        x0: int, y0: int, x1: int, y1: int, rgba: tuple[int, int, int, int], radius: int = 0
    ) -> None:
        for y in range(y0, y1):
            for x in range(x0, x1):
                if radius > 0:
                    if x < x0 + radius and y < y0 + radius:
                        if (x - (x0 + radius)) ** 2 + (y - (y0 + radius)) ** 2 > radius**2:
                            continue
                    elif x >= x1 - radius and y < y0 + radius:
                        if (x - (x1 - 1 - radius)) ** 2 + (y - (y0 + radius)) ** 2 > radius**2:
                            continue
                    elif x < x0 + radius and y >= y1 - radius:
                        if (x - (x0 + radius)) ** 2 + (y - (y1 - 1 - radius)) ** 2 > radius**2:
                            continue
                    elif x >= x1 - radius and y >= y1 - radius:
                        if (x - (x1 - 1 - radius)) ** 2 + (
                            y - (y1 - 1 - radius)
                        ) ** 2 > radius**2:
                            continue
                px(x, y, *rgba)

    def fill_ellipse(
        cx: float, cy: float, rx: float, ry: float, rgba: tuple[int, int, int, int]
    ) -> None:
        for y in range(int(cy - ry) - 1, int(cy + ry) + 2):
            for x in range(int(cx - rx) - 1, int(cx + rx) + 2):
                if ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1.0:
                    px(x, y, *rgba)

    fill_rect(0, 0, 32, 32, (18, 21, 28, 255), radius=6)
    fill_rect(5, 6, 27, 26, (42, 47, 58, 255), radius=4)
    fill_rect(8, 9, 24, 19, (11, 13, 18, 255), radius=2)
    fill_ellipse(13, 14, 2.4, 2.8, (255, 138, 31, 255))
    fill_ellipse(19, 14, 2.4, 2.8, (255, 138, 31, 255))
    fill_rect(11, 22, 21, 24, (255, 138, 31, 200), radius=1)

    raw = b"".join(bytes(p) for p in buf)
    rows = b""
    for y in range(H):
        rows += b"\x00" + raw[y * W * 4 : (y + 1) * W * 4]
    comp = zlib.compress(rows, 9)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", comp)
    png += chunk(b"IEND", b"")

    out = Path(__file__).resolve().parents[1] / "src-tauri" / "icons" / "hue-tray.png"
    out.write_bytes(png)
    print(f"wrote {out} ({len(png)} bytes)")


if __name__ == "__main__":
    main()
