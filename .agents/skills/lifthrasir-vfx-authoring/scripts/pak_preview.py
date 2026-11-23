"""Find and preview content in assets/lifthrasir.pak without printing raw Korean names.

usage (from the repo root):
  python3 pak_preview.py find SUBSTRING [SUBSTRING ...]
      list entries containing every substring (Korean substrings are fine as input)
  python3 pak_preview.py show OUT.png ENTRY [ENTRY ...]
      swatch sheet, one 128 px tile per texture (.bmp/.tga/.png) or per SPR frame (.spr)

Names are printed ASCII-escaped: raw Korean file names in tool output have crashed
agent sessions. BMPs get the runtime's magenta key; tiles sit on a dark background,
which is how additive black-background textures read in game. BMP/TGA decoding
uses macOS `sips`.
"""
import os
import struct
import subprocess
import sys
import tempfile
import warnings
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from pngio import read_png, write_png  # noqa: E402

PAK = 'assets/lifthrasir.pak'
TILE = 128
GAP = 4
BACKGROUND = (0x30, 0x30, 0x38)

# Shared headers trip zipfile's overlap warning, which prints the raw (Korean) name.
warnings.filterwarnings('ignore', 'Overlapped entries')


def escaped(name):
    return name.encode('unicode_escape').decode()


def read_entry(pak, name):
    """Read an entry. The pak shares one local header between identical files,
    which zipfile rejects as a name mismatch, so retry under the header's name."""
    info = pak.getinfo(name)
    try:
        return pak.read(info)
    except zipfile.BadZipFile:
        pak.fp.seek(info.header_offset)
        header = pak.fp.read(30)
        name_len = struct.unpack_from('<H', header, 26)[0]
        info.orig_filename = pak.fp.read(name_len).decode('utf-8')
        return pak.read(info)


def image_frames(data, suffix):
    """Decode a BMP/TGA/PNG through sips into one (w, h, rgba_rows) frame."""
    with tempfile.TemporaryDirectory() as tmp:
        src, png = os.path.join(tmp, 'in' + suffix), os.path.join(tmp, 'out.png')
        Path(src).write_bytes(data)
        subprocess.run(['sips', '-s', 'format', 'png', src, '--out', png], check=True, capture_output=True)
        w, h, rows = read_png(png)
    if suffix == '.bmp':
        rows = [magenta_key(row) for row in rows]
    return [(w, h, rows)]


def magenta_key(row):
    """Mirror lifthrasir-assets `apply_magenta_transparency`."""
    out = bytearray(row)
    for i in range(0, len(out), 4):
        if out[i] >= 240 and out[i + 1] <= 15 and out[i + 2] >= 240:
            out[i + 3] = 0
    return bytes(out)


def spr_frames(data):
    """Decode every SPR frame: palette-indexed (index 0 transparent) and RGBA
    (stored bottom-up in ABGR order, like ro-formats' `parse_rgba_frame`)."""
    version, indexed = struct.unpack_from('<HH', data, 2)
    rgba_count, offset = (struct.unpack_from('<H', data, 6)[0], 8) if version >= 0x200 else (0, 6)
    palette, frames = data[-1024:], []
    for _ in range(indexed):
        w, h = struct.unpack_from('<HH', data, offset)
        offset += 4
        if version >= 0x201:
            size = struct.unpack_from('<H', data, offset)[0]
            packed, offset = data[offset + 2:offset + 2 + size], offset + 2 + size
            pixels, i = bytearray(), 0
            while i < len(packed):
                if packed[i] == 0:
                    pixels += bytes(packed[i + 1])
                    i += 2
                else:
                    pixels.append(packed[i])
                    i += 1
        else:
            pixels, offset = data[offset:offset + w * h], offset + w * h
        rgba = b''.join(palette[p * 4:p * 4 + 3] + (b'\0' if p == 0 else b'\xff') for p in pixels[:w * h])
        frames.append((w, h, [rgba[y * w * 4:(y + 1) * w * 4] for y in range(h)]))
    for _ in range(rgba_count):
        w, h = struct.unpack_from('<HH', data, offset)
        body, offset = data[offset + 4:offset + 4 + w * h * 4], offset + 4 + w * h * 4
        rows = [body[y * w * 4:(y + 1) * w * 4] for y in range(h)][::-1]
        frames.append((w, h, [b''.join(bytes((r[i + 3], r[i + 2], r[i + 1], r[i])) for i in range(0, len(r), 4)) for r in rows]))
    return frames


def tile(frame):
    """Fit a frame into a TILE x TILE RGB tile, alpha-composited on the background."""
    w, h, rows = frame
    scale = max(w, h) / TILE
    out = [bytearray(bytes(BACKGROUND) * TILE) for _ in range(TILE)]
    x_pad, y_pad = (TILE - int(w / scale)) // 2, (TILE - int(h / scale)) // 2
    for y in range(int(h / scale)):
        row = rows[min(h - 1, int(y * scale))]
        for x in range(int(w / scale)):
            px = row[min(w - 1, int(x * scale)) * 4:][:4]
            alpha = px[3] / 255
            d = (x_pad + x) * 3
            out[y_pad + y][d:d + 3] = bytes(int(px[c] * alpha + BACKGROUND[c] * (1 - alpha)) for c in range(3))
    return out


def show(out, entries):
    pak = zipfile.ZipFile(PAK)
    tiles = []
    for entry in entries:
        data = read_entry(pak, entry)
        suffix = Path(entry).suffix.lower()
        frames = spr_frames(data) if suffix == '.spr' else image_frames(data, suffix)
        w, h = frames[0][0], frames[0][1]
        print(f'tiles {len(tiles)}..{len(tiles) + len(frames) - 1}: {escaped(entry)} ({w}x{h}, {len(frames)} frame(s))')
        tiles += [tile(frame) for frame in frames]
    width = len(tiles) * (TILE + GAP)
    canvas = [bytearray(width * 3) for _ in range(TILE)]
    for index, t in enumerate(tiles):
        for y in range(TILE):
            start = index * (TILE + GAP) * 3
            canvas[y][start:start + TILE * 3] = t[y]
    write_png(out, width, TILE, canvas)


def find(needles):
    names = zipfile.ZipFile(PAK).namelist()
    hits = [n for n in names if all(s.lower() in n.lower() for s in needles)]
    for name in hits[:200]:
        print(escaped(name))
    print(f'{len(hits)} match(es)' + (' (first 200 shown)' if len(hits) > 200 else ''))


if __name__ == '__main__':
    if len(sys.argv) >= 3 and sys.argv[1] == 'find':
        find(sys.argv[2:])
    elif len(sys.argv) >= 4 and sys.argv[1] == 'show':
        show(sys.argv[2], sys.argv[3:])
    else:
        sys.exit(__doc__)
