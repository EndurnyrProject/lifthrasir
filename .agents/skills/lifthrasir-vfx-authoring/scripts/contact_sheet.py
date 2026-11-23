"""Tile fx_capture screenshots side by side so one image read shows the whole timeline.

usage: python3 contact_sheet.py OUT.png SHOT_00.png SHOT_01.png ...

Each shot is cropped around the stand-in body (fx_capture centres it) and halved,
so a 960x960 retina screenshot becomes a 300x300 tile.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from pngio import read_png, write_png  # noqa: E402

CROP = 600
LIFT = 60  # the body sits a little below the window centre
SCALE = 2
GAP = 4


def main(out, shots):
    tile = CROP // SCALE
    width = len(shots) * (tile + GAP)
    canvas = [bytearray(width * 3) for _ in range(tile)]
    for index, shot in enumerate(shots):
        w, h, rows = read_png(shot)
        x0 = max(0, (w - CROP) // 2)
        y0 = max(0, (h - CROP) // 2 - LIFT)
        for y in range(tile):
            row = rows[min(h - 1, y0 + y * SCALE)]
            for x in range(tile):
                src = min(w - 1, x0 + x * SCALE) * 4
                dst = (index * (tile + GAP) + x) * 3
                canvas[y][dst:dst + 3] = row[src:src + 3]
    write_png(out, width, tile, canvas)


if __name__ == '__main__':
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2:])
