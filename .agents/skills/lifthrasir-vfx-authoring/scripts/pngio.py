"""Minimal stdlib PNG read/write for the VFX preview scripts (no Pillow here)."""
import struct
import zlib


def read_png(path):
    """Return (width, height, rows) with every row as RGBA bytes. Non-interlaced 8-bit only."""
    data = open(path, 'rb').read()
    width, height, depth, ctype, _, _, interlace = struct.unpack_from('>IIBBBBB', data, 16)
    if depth != 8 or interlace:
        raise ValueError(f'{path}: only 8-bit non-interlaced PNG is supported')
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    idat, palette, trns, offset = b'', b'', b'', 8
    while offset < len(data):
        size = struct.unpack_from('>I', data, offset)[0]
        kind, body = data[offset + 4:offset + 8], data[offset + 8:offset + 8 + size]
        idat += body if kind == b'IDAT' else b''
        palette = body if kind == b'PLTE' else palette
        trns = body if kind == b'tRNS' else trns
        offset += 12 + size
    raw, stride = zlib.decompress(idat), width * channels
    previous, rows = bytearray(stride), []
    for y in range(height):
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        for x in range(stride):
            a = line[x - channels] if x >= channels else 0
            b = previous[x]
            c = previous[x - channels] if x >= channels else 0
            if kind == 1:
                line[x] = (line[x] + a) & 255
            elif kind == 2:
                line[x] = (line[x] + b) & 255
            elif kind == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(_to_rgba(bytes(line), ctype, palette, trns))
        previous = line
    return width, height, rows


def _to_rgba(line, ctype, palette, trns):
    if ctype == 6:
        return line
    if ctype == 2:
        return b''.join(line[i:i + 3] + b'\xff' for i in range(0, len(line), 3))
    if ctype == 0:
        return b''.join(bytes((v, v, v, 255)) for v in line)
    if ctype == 4:
        return b''.join(bytes((line[i], line[i], line[i], line[i + 1])) for i in range(0, len(line), 2))
    return b''.join(palette[i * 3:i * 3 + 3] + bytes((trns[i] if i < len(trns) else 255,)) for i in line)


def write_png(path, width, height, rgb_rows):
    """Write RGB rows (bytes of width * 3) as a PNG."""
    raw = b''.join(b'\0' + bytes(row) for row in rgb_rows)

    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))

    header = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    with open(path, 'wb') as out:
        out.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', header) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))
