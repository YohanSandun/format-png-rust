"""Generates the PNG test fixtures in this directory.

Run from anywhere with `python tests/data/generate.py`. Only the standard library
is used, so the output is fully determined by this script.

valid/    well-formed PNGs covering every color type / bit depth, interlacing,
          filters, chunk layouts and ancillary chunks
invalid/  malformed PNGs, one defect each; the file name says what is wrong

Pixel values follow `sample()` so decoded output can be checked without storing
reference images.
"""

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).parent
SIGNATURE = b"\x89PNG\r\n\x1a\n"

GRAY, RGB, INDEXED, GRAY_ALPHA, RGBA = 0, 2, 3, 4, 6
CHANNELS = {GRAY: 1, RGB: 3, INDEXED: 1, GRAY_ALPHA: 2, RGBA: 4}
BIT_DEPTHS = {
    GRAY: [1, 2, 4, 8, 16],
    RGB: [8, 16],
    INDEXED: [1, 2, 4, 8],
    GRAY_ALPHA: [8, 16],
    RGBA: [8, 16],
}
NAMES = {GRAY: "gray", RGB: "rgb", INDEXED: "indexed", GRAY_ALPHA: "gray_alpha", RGBA: "rgba"}

# (x start, y start, x step, y step) for the 7 Adam7 passes
ADAM7 = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]


# ---------- pixels ----------

def sample(x, y, channel, color_type, bit_depth):
    """The value of one channel of pixel (x, y)."""
    if color_type == INDEXED:
        return (x + 3 * y) % (1 << bit_depth)
    return (x * 37 + y * 101 + channel * 53) % (1 << bit_depth)


def palette(bit_depth):
    """One distinct color per possible index."""
    size = 1 << bit_depth
    return [((i * 67) % 256, (i * 131) % 256, (i * 199) % 256) for i in range(size)]


def pixels(width, height, color_type, bit_depth):
    """Rows of pixels, each pixel a tuple of channel values."""
    channels = CHANNELS[color_type]
    return [
        [tuple(sample(x, y, c, color_type, bit_depth) for c in range(channels)) for x in range(width)]
        for y in range(height)
    ]


# ---------- scanlines ----------

def pack_row(row, bit_depth):
    """Packs one row of pixels into bytes, most significant bits first."""
    if bit_depth == 16:
        return b"".join(struct.pack(">H", v) for px in row for v in px)
    if bit_depth == 8:
        return bytes(v for px in row for v in px)
    out, acc, bits = bytearray(), 0, 0
    for px in row:
        for v in px:
            acc = (acc << bit_depth) | v
            bits += bit_depth
            if bits == 8:
                out.append(acc)
                acc, bits = 0, 0
    if bits:
        out.append(acc << (8 - bits))
    return bytes(out)


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def filter_row(filter_type, row, prev, bpp):
    """Applies one PNG filter; `bpp` is bytes per complete pixel (at least 1)."""
    out = bytearray()
    for i, x in enumerate(row):
        a = row[i - bpp] if i >= bpp else 0
        b = prev[i]
        c = prev[i - bpp] if i >= bpp else 0
        predictor = [0, a, b, (a + b) // 2, paeth(a, b, c)][filter_type]
        out.append((x - predictor) & 0xFF)
    return bytes([filter_type]) + bytes(out)


def filtered(rows, color_type, bit_depth, filters):
    """Filters packed rows; `filters(y)` picks the filter type for row y."""
    bpp = max(1, CHANNELS[color_type] * bit_depth // 8)
    out, prev = bytearray(), None
    for y, row in enumerate(rows):
        packed = pack_row(row, bit_depth)
        prev = prev or bytes(len(packed))
        out += filter_row(filters(y), packed, prev, bpp)
        prev = packed
    return bytes(out)


def image_data(width, height, color_type, bit_depth, interlace=0, filters=lambda y: 0):
    """The uncompressed IDAT stream for the standard test image."""
    image = pixels(width, height, color_type, bit_depth)
    if not interlace:
        return filtered(image, color_type, bit_depth, filters)
    out = bytearray()
    for x0, y0, dx, dy in ADAM7:
        sub = [row[x0::dx] for row in image[y0::dy]]
        if sub and sub[0]:  # empty passes contribute nothing
            out += filtered(sub, color_type, bit_depth, filters)
    return bytes(out)


# ---------- chunks ----------

def chunk(chunk_type, data=b"", crc=None):
    if crc is None:
        crc = zlib.crc32(chunk_type + data)
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def ihdr_data(width, height, bit_depth, color_type, compression=0, filter_method=0, interlace=0):
    return struct.pack(">IIBBBBB", width, height, bit_depth, color_type, compression, filter_method, interlace)


def ihdr(*args, **kwargs):
    return chunk(b"IHDR", ihdr_data(*args, **kwargs))


def plte(bit_depth):
    return chunk(b"PLTE", b"".join(bytes(rgb) for rgb in palette(bit_depth)))


IEND = chunk(b"IEND")


def png(width, height, color_type, bit_depth, interlace=0, filters=lambda y: 0, level=9, extra=b""):
    """A complete, valid PNG of the standard test image. `extra` goes before IDAT."""
    raw = image_data(width, height, color_type, bit_depth, interlace, filters)
    body = ihdr(width, height, bit_depth, color_type, interlace=interlace)
    if color_type == INDEXED:
        body += plte(bit_depth)
    body += extra
    body += chunk(b"IDAT", zlib.compress(raw, level))
    return SIGNATURE + body + IEND


# ---------- fixtures ----------

def valid():
    files = {}

    # Every color type and bit depth, plain and interlaced. 13x7 is not a multiple
    # of 8, so sub-byte rows end in padding bits and some Adam7 passes are narrow.
    for color_type, depths in BIT_DEPTHS.items():
        for depth in depths:
            name = f"{NAMES[color_type]}_{depth}"
            files[f"{name}.png"] = png(13, 7, color_type, depth)
            files[f"{name}_adam7.png"] = png(13, 7, color_type, depth, interlace=1)

    # Tiny images: 1x1 leaves six of the seven Adam7 passes empty.
    files["rgba_8_1x1.png"] = png(1, 1, RGBA, 8)
    files["rgba_8_1x1_adam7.png"] = png(1, 1, RGBA, 8, interlace=1)
    files["gray_1_1x1.png"] = png(1, 1, GRAY, 1)

    # Each filter type on every row, then a mix of all five.
    for filter_type, name in enumerate(["none", "sub", "up", "average", "paeth"]):
        files[f"filter_{name}.png"] = png(16, 16, RGB, 8, filters=lambda y, f=filter_type: f)
        files[f"filter_{name}_16.png"] = png(16, 16, RGBA, 16, filters=lambda y, f=filter_type: f)
    files["filter_mixed.png"] = png(16, 16, RGB, 8, filters=lambda y: y % 5)
    files["filter_mixed_adam7.png"] = png(16, 16, RGB, 8, interlace=1, filters=lambda y: y % 5)
    files["filter_mixed_gray_2.png"] = png(16, 16, GRAY, 2, filters=lambda y: y % 5)

    # Compression: stored (uncompressed) deflate blocks, and a larger image.
    files["compression_stored.png"] = png(32, 32, RGBA, 8, level=0)
    files["large_rgba_8.png"] = png(256, 256, RGBA, 8)

    # IDAT split over several chunks, including a zero-length one.
    raw = zlib.compress(image_data(16, 16, RGB, 8), 9)
    parts = [raw[:1], raw[1:10], b"", raw[10:]]
    files["idat_split.png"] = SIGNATURE + ihdr(16, 16, 8, RGB) + b"".join(chunk(b"IDAT", p) for p in parts) + IEND
    one_byte = b"".join(chunk(b"IDAT", raw[i:i + 1]) for i in range(len(raw)))
    files["idat_one_byte_each.png"] = SIGNATURE + ihdr(16, 16, 8, RGB) + one_byte + IEND

    # Transparency.
    files["indexed_8_trns.png"] = png(13, 7, INDEXED, 8, extra=chunk(b"tRNS", bytes(range(0, 256, 2))))
    files["gray_8_trns.png"] = png(13, 7, GRAY, 8, extra=chunk(b"tRNS", struct.pack(">H", 37)))
    files["rgb_16_trns.png"] = png(13, 7, RGB, 16, extra=chunk(b"tRNS", struct.pack(">HHH", 0, 53, 106)))

    # Ancillary chunks, public and private, before and after IDAT.
    ancillary = (
        chunk(b"gAMA", struct.pack(">I", 45455))
        + chunk(b"pHYs", struct.pack(">IIB", 2835, 2835, 1))
        + chunk(b"tEXt", b"Title\x00format-png test image")
        + chunk(b"zTXt", b"Comment\x00\x00" + zlib.compress(b"compressed text"))
        + chunk(b"ruSt", b"private safe-to-copy chunk")
    )
    files["ancillary_chunks.png"] = png(13, 7, RGBA, 8, extra=ancillary)
    after_idat = png(13, 7, RGBA, 8)
    files["ancillary_after_idat.png"] = after_idat[:-len(IEND)] + chunk(b"tEXt", b"Author\x00format-png") + IEND

    return files


def invalid():
    good = png(13, 7, RGBA, 8)
    ihdr_end = len(SIGNATURE) + 25
    idat = chunk(b"IDAT", zlib.compress(image_data(13, 7, RGBA, 8)))

    def with_ihdr(data):
        """A PNG whose IHDR chunk (correct CRC) carries `data`."""
        return SIGNATURE + chunk(b"IHDR", data) + idat + IEND

    def flip_last_byte(data):
        return data[:-1] + bytes([data[-1] ^ 0xFF])

    files = {
        # Signature
        "empty.png": b"",
        "signature_truncated.png": SIGNATURE[:5],
        "signature_wrong.png": b"\x89PNG\r\n\x1a\x0b" + good[8:],
        "signature_only.png": SIGNATURE,
        "not_png.png": b"GIF89a" + bytes(40),

        # Chunk framing
        "first_chunk_not_ihdr.png": SIGNATURE + chunk(b"gAMA", struct.pack(">I", 45455)) + good[8:],
        "chunk_length_too_long.png": SIGNATURE + struct.pack(">I", 0x8000_0000) + b"IHDR" + good[16:],
        "chunk_type_invalid.png": SIGNATURE + chunk(b"IH1R", ihdr_data(13, 7, 8, RGBA)) + good[ihdr_end:],
        "ihdr_crc_wrong.png": flip_last_byte(good[:ihdr_end]) + good[ihdr_end:],
        "idat_crc_wrong.png": good[:ihdr_end] + flip_last_byte(idat) + IEND,
        "ihdr_truncated.png": good[:20],
        "ihdr_missing_crc.png": good[:ihdr_end - 4],

        # IHDR contents (all with correct CRCs)
        "ihdr_length_12.png": with_ihdr(ihdr_data(13, 7, 8, RGBA)[:12]),
        "ihdr_length_14.png": with_ihdr(ihdr_data(13, 7, 8, RGBA) + b"\x00"),
        "ihdr_width_zero.png": with_ihdr(ihdr_data(0, 7, 8, RGBA)),
        "ihdr_height_zero.png": with_ihdr(ihdr_data(13, 0, 8, RGBA)),
        "ihdr_width_too_large.png": with_ihdr(ihdr_data(0x8000_0000, 7, 8, RGBA)),
        "ihdr_color_type_5.png": with_ihdr(ihdr_data(13, 7, 8, 5)),
        "ihdr_bit_depth_3.png": with_ihdr(ihdr_data(13, 7, 3, GRAY)),
        "ihdr_rgb_bit_depth_4.png": with_ihdr(ihdr_data(13, 7, 4, RGB)),
        "ihdr_indexed_bit_depth_16.png": with_ihdr(ihdr_data(13, 7, 16, INDEXED)),
        "ihdr_compression_1.png": with_ihdr(ihdr_data(13, 7, 8, RGBA, compression=1)),
        "ihdr_filter_method_1.png": with_ihdr(ihdr_data(13, 7, 8, RGBA, filter_method=1)),
        "ihdr_interlace_2.png": with_ihdr(ihdr_data(13, 7, 8, RGBA, interlace=2)),

        # Later chunks (valid IHDR), for when decoding goes past the header
        "idat_missing.png": SIGNATURE + ihdr(13, 7, 8, RGBA) + IEND,
        "idat_truncated.png": good[:-len(IEND) - 10],
        "iend_missing.png": good[:-len(IEND)],
        "indexed_plte_missing.png": SIGNATURE + ihdr(13, 7, 8, INDEXED)
            + chunk(b"IDAT", zlib.compress(image_data(13, 7, INDEXED, 8))) + IEND,
        "zlib_corrupt.png": SIGNATURE + ihdr(13, 7, 8, RGBA)
            + chunk(b"IDAT", b"\x78\x9c\xff\xff\xff\xff\xff\xff") + IEND,
        "filter_type_5.png": SIGNATURE + ihdr(13, 7, 8, RGBA)
            + chunk(b"IDAT", zlib.compress(b"\x05" + image_data(13, 7, RGBA, 8)[1:])) + IEND,
        "image_data_too_short.png": SIGNATURE + ihdr(13, 7, 8, RGBA)
            + chunk(b"IDAT", zlib.compress(image_data(13, 7, RGBA, 8)[:-20])) + IEND,
    }
    return files


def write(directory, files):
    directory.mkdir(parents=True, exist_ok=True)
    for old in directory.glob("*.png"):
        old.unlink()
    for name, data in sorted(files.items()):
        (directory / name).write_bytes(data)
    print(f"{directory.relative_to(ROOT)}/: {len(files)} files")


if __name__ == "__main__":
    write(ROOT / "valid", valid())
    write(ROOT / "invalid", invalid())
