"""Makes a large RGBA 8-bit PNG for the decoding benchmark. See benches/README.md.

The image is written a few rows at a time, so it needs little memory however big
it is. Rows cycle through all five filter types, so every unfiltering path gets
timed, and the pixels are gradients with some noise, so the file compresses
about as well as a photo-like image rather than to almost nothing.

Needs numpy: `pip install numpy`.

Usage: python benches/generate.py [WIDTH HEIGHT] [OUT.png]
Default: 32768 x 32768 (1 gigapixel, 4 GiB decoded) to benches/data/max_rgba_8.png
"""

import struct
import sys
import time
import zlib
from pathlib import Path

import numpy as np

CHANNELS = 4
ROWS_PER_BLOCK = 64
IDAT_SIZE = 1 << 20
COMPRESSION_LEVEL = 6  # zlib's default, and what most encoders use


def chunk(kind, data):
    crc = zlib.crc32(data, zlib.crc32(kind))
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", crc)


def pixels(width, y0, rows, rng):
    """Rows y0..y0+rows as a (rows, width * 4) uint8 array."""
    x = np.arange(width, dtype=np.uint32)[None, :]
    y = np.arange(y0, y0 + rows, dtype=np.uint32)[:, None]
    # Noise that changes every 8 pixels, so neighbours are similar but not equal.
    noise = np.repeat(rng.integers(0, 12, (rows, width // 8 + 1), dtype=np.uint32), 8, axis=1)[:, :width]
    out = np.empty((rows, width, CHANNELS), dtype=np.uint8)
    out[..., 0] = (x // 64 + noise) & 255
    out[..., 1] = (y // 64 + noise) & 255
    out[..., 2] = ((x + y) // 128 + noise // 2) & 255
    out[..., 3] = 255 - (noise // 4)
    return out.reshape(rows, width * CHANNELS)


def filter_row(kind, row, prev):
    """Filters one row. `row` and `prev` are uint8 arrays; returns bytes with the type byte."""
    bpp = CHANNELS
    if kind == 0:
        out = row
    elif kind == 1:
        left = np.concatenate((np.zeros(bpp, np.uint8), row[:-bpp]))
        out = row - left
    elif kind == 2:
        out = row - prev
    elif kind == 3:
        left = np.concatenate((np.zeros(bpp, np.uint8), row[:-bpp]))
        out = row - ((left.astype(np.uint16) + prev) >> 1).astype(np.uint8)
    else:
        a = np.concatenate((np.zeros(bpp, np.uint8), row[:-bpp])).astype(np.int16)
        b = prev.astype(np.int16)
        c = np.concatenate((np.zeros(bpp, np.uint8), prev[:-bpp])).astype(np.int16)
        p = a + b - c
        pa, pb, pc = np.abs(p - a), np.abs(p - b), np.abs(p - c)
        predictor = np.where((pa <= pb) & (pa <= pc), a, np.where(pb <= pc, b, c)).astype(np.uint8)
        out = row - predictor
    return bytes([kind]) + out.tobytes()


def main():
    args = sys.argv[1:]
    if len(args) not in (0, 1, 2, 3) or len(args) == 1 and args[0].isdigit():
        sys.exit(__doc__)
    width, height = (int(args[0]), int(args[1])) if len(args) >= 2 else (32768, 32768)
    out_path = Path(args[-1] if len(args) in (1, 3) else Path(__file__).parent / "data" / "max_rgba_8.png")
    out_path.parent.mkdir(parents=True, exist_ok=True)

    rng = np.random.default_rng(1)
    compressor = zlib.compressobj(COMPRESSION_LEVEL)
    pending = bytearray()
    prev = np.zeros(width * CHANNELS, np.uint8)
    start = time.perf_counter()

    with open(out_path, "wb") as file:
        file.write(b"\x89PNG\r\n\x1a\n")
        file.write(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)))

        def flush(data):
            pending.extend(data)
            while len(pending) >= IDAT_SIZE:
                file.write(chunk(b"IDAT", bytes(pending[:IDAT_SIZE])))
                del pending[:IDAT_SIZE]

        for y0 in range(0, height, ROWS_PER_BLOCK):
            block = pixels(width, y0, min(ROWS_PER_BLOCK, height - y0), rng)
            filtered = bytearray()
            for i, row in enumerate(block):
                filtered += filter_row((y0 + i) % 5, row, prev)
                prev = row
            flush(compressor.compress(filtered))
            if y0 % (ROWS_PER_BLOCK * 64) == 0:
                print(f"\r  {y0 * 100 // height:3}%", end="", flush=True)

        flush(compressor.flush())
        if pending:
            file.write(chunk(b"IDAT", bytes(pending)))
        file.write(chunk(b"IEND", b""))

    decoded = width * height * CHANNELS
    size = out_path.stat().st_size
    print(f"\r{out_path}: {width}x{height} RGBA 8-bit, {size / 1e6:.1f} MB file, "
          f"{decoded / 2**30:.2f} GiB decoded, {time.perf_counter() - start:.0f} s")


if __name__ == "__main__":
    main()
