# Writes an intentionally ugly stand-in app icon (magenta/green checkerboard)
# so it can never pass for real art. Replace icons/ with the owner's icon.
import struct, sys, zlib
size, cell = 512, 64
rows = []
for y in range(size):
    row = bytearray([0])
    for x in range(size):
        on = ((x // cell) + (y // cell)) % 2
        row += bytes([255, 0, 255, 255]) if on else bytes([0, 255, 0, 255])
    rows.append(bytes(row))
def chunk(t, d):
    c = t + d
    return struct.pack(">I", len(d)) + c + struct.pack(">I", zlib.crc32(c) & 0xffffffff)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)) \
    + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
