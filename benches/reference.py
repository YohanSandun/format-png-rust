"""Reference timings for comparing with the Rust benchmark. See benches/README.md.

Times C zlib (through Python) decompressing the same IDAT stream, and Pillow
decoding the whole file. Needs Pillow: `pip install pillow`.

Usage: python benches/reference.py FILE.png [FILE.png ...]
"""

import collections
import statistics
import struct
import sys
import time
import zlib

from PIL import Image

Image.MAX_IMAGE_PIXELS = None  # allow very large benchmark images

RUNS = 7
FILTER_NAMES = ["None", "Sub", "Up", "Average", "Paeth"]


def bench(name, fn):
    fn()  # warm-up
    times = []
    for _ in range(RUNS):
        start = time.perf_counter()
        fn()
        times.append(time.perf_counter() - start)
    times.sort()
    print(f"  {name:<44} min {times[0] * 1e3:9.2f} ms   median {statistics.median(times) * 1e3:9.2f} ms")


def idat_stream(data):
    stream = bytearray()
    i = 8
    while i < len(data):
        length, = struct.unpack(">I", data[i:i + 4])
        if data[i + 4:i + 8] == b"IDAT":
            stream += data[i + 8:i + 8 + length]
        i += 12 + length
    return bytes(stream)


def filter_counts(data, raw):
    """Filter type of each row. Only meaningful for non-interlaced images."""
    width, height, bit_depth, color_type, _, _, interlace = struct.unpack(">IIBBBBB", data[16:29])
    if interlace:
        return "n/a (interlaced)"
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color_type]
    stride = (width * channels * bit_depth + 7) // 8 + 1
    counts = collections.Counter(raw[y * stride] for y in range(height))
    return {FILTER_NAMES[k] if k < 5 else k: v for k, v in sorted(counts.items())}


def pillow_decode(path):
    with Image.open(path) as image:
        image.load()


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    print(f"zlib {zlib.ZLIB_RUNTIME_VERSION}, Pillow {Image.__version__}\n")

    for path in sys.argv[1:]:
        data = open(path, "rb").read()
        stream = idat_stream(data)
        raw = zlib.decompress(stream)
        print(f"{path}: {len(data) / 1e6:.1f} MB file, filters {filter_counts(data, raw)}")
        bench("zlib decompression only (C zlib)", lambda: zlib.decompress(stream))
        bench("Pillow full decode", lambda: pillow_decode(path))
        print()


if __name__ == "__main__":
    main()
