import os
import struct
import zlib

ROOT = os.path.dirname(os.path.dirname(__file__))
TEXTURE_DIR = os.path.join(ROOT, "assets", "textures")
TILE = 32
CELL = 34


def clamp(value, low=0, high=255):
    return max(low, min(high, int(value)))


def mix(a, b, t):
    return tuple(clamp(a[i] + (b[i] - a[i]) * t) for i in range(4))


def shade(color, amount):
    return (
        clamp(color[0] + amount),
        clamp(color[1] + amount),
        clamp(color[2] + amount),
        color[3],
    )


def noise(x, y, seed):
    value = (x * 374761393 + y * 668265263 + seed * 1442695040888963407) & 0xFFFFFFFF
    value ^= value >> 13
    value = (value * 1274126177) & 0xFFFFFFFF
    value ^= value >> 16
    return value & 0xFFFF


def blank(color):
    return [[color for _ in range(TILE)] for _ in range(TILE)]


def put(tile, x, y, color):
    if 0 <= x < TILE and 0 <= y < TILE:
        tile[y][x] = color


def dirt():
    tile = blank((107, 72, 45, 255))
    for y in range(TILE):
        for x in range(TILE):
            n = noise(x, y, 11)
            color = (112, 75, 47, 255)
            color = shade(color, (n % 31) - 15)
            if noise(x // 2, y // 2, 17) % 9 == 0:
                color = mix(color, (76, 49, 34, 255), 0.45)
            if noise(x, y, 23) % 41 == 0:
                color = mix(color, (151, 110, 76, 255), 0.35)
            tile[y][x] = color
    return tile


def grass_top():
    tile = blank((73, 142, 58, 255))
    for y in range(TILE):
        for x in range(TILE):
            n = noise(x, y, 31)
            color = (76, 145, 60, 255)
            color = shade(color, (n % 39) - 16)
            if noise(x // 3, y // 3, 37) % 5 == 0:
                color = mix(color, (112, 177, 78, 255), 0.35)
            if noise(x // 2, y // 2, 41) % 7 == 0:
                color = mix(color, (38, 91, 43, 255), 0.28)
            tile[y][x] = color
    return tile


def grass_side():
    tile = dirt()
    for y in range(9):
        for x in range(TILE):
            edge = 5 + noise(x, 0, 53) % 5
            if y <= edge:
                color = (68, 139, 54, 255)
                color = shade(color, (noise(x, y, 59) % 35) - 12)
                tile[y][x] = color
    for x in range(0, TILE, 3):
        root = 7 + noise(x, 0, 61) % 5
        length = 2 + noise(x, 0, 67) % 5
        for y in range(root, min(TILE, root + length)):
            put(tile, x, y, mix(tile[y][x], (54, 105, 44, 255), 0.55))
    return tile


def stone():
    tile = blank((103, 107, 112, 255))
    for y in range(TILE):
        for x in range(TILE):
            n = noise(x, y, 71)
            color = shade((101, 106, 111, 255), (n % 35) - 17)
            if (x + y + noise(x // 4, y // 4, 73) % 7) % 11 == 0:
                color = mix(color, (70, 76, 83, 255), 0.4)
            if noise(x // 3, y // 3, 79) % 17 == 0:
                color = mix(color, (137, 143, 147, 255), 0.3)
            tile[y][x] = color
    return tile


def sand():
    tile = blank((205, 183, 122, 255))
    for y in range(TILE):
        for x in range(TILE):
            n = noise(x, y, 83)
            color = shade((207, 187, 128, 255), (n % 25) - 10)
            if noise(x, y, 89) % 23 == 0:
                color = mix(color, (236, 219, 160, 255), 0.35)
            if noise(x // 2, y // 2, 97) % 13 == 0:
                color = mix(color, (171, 145, 91, 255), 0.28)
            tile[y][x] = color
    return tile


def log():
    tile = blank((92, 58, 36, 255))
    for y in range(TILE):
        for x in range(TILE):
            band = ((x // 4) % 2) * 14
            n = noise(x, y, 101)
            color = shade((87 + band, 54 + band // 2, 34, 255), (n % 21) - 9)
            if noise(x // 2, y // 6, 107) % 8 == 0:
                color = mix(color, (48, 31, 22, 255), 0.45)
            tile[y][x] = color
    for y in range(5, TILE, 11):
        for x in range(4, TILE - 4):
            if noise(x, y, 109) % 3 != 0:
                tile[y][x] = mix(tile[y][x], (46, 29, 20, 255), 0.55)
    return tile


def leaves():
    tile = blank((40, 102, 45, 255))
    for y in range(TILE):
        for x in range(TILE):
            n = noise(x, y, 113)
            color = shade((42, 111, 48, 255), (n % 47) - 20)
            if noise(x // 3, y // 3, 127) % 4 == 0:
                color = mix(color, (75, 151, 64, 255), 0.45)
            if noise(x // 2, y // 2, 131) % 7 == 0:
                color = mix(color, (20, 65, 34, 255), 0.4)
            tile[y][x] = color
    return tile


def water():
    tile = blank((67, 137, 216, 205))
    for y in range(TILE):
        for x in range(TILE):
            wave = ((x * 2 + y + noise(x // 4, y // 4, 137) % 7) % 13) < 2
            color = (61, 132, 216, 205)
            color = shade(color, (noise(x, y, 139) % 25) - 8)
            if wave:
                color = mix(color, (143, 207, 255, 220), 0.48)
            if noise(x // 3, y // 3, 149) % 9 == 0:
                color = mix(color, (32, 91, 176, 210), 0.25)
            tile[y][x] = color
    return tile


def png_bytes(pixels):
    height = len(pixels)
    width = len(pixels[0])
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for r, g, b, a in row:
            raw.extend((r, g, b, a))

    def chunk(kind, data):
        payload = kind + data
        return (
            struct.pack(">I", len(data))
            + payload
            + struct.pack(">I", zlib.crc32(payload) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def write_png(path, pixels):
    with open(path, "wb") as file:
        file.write(png_bytes(pixels))


def padded(tile):
    result = [[(0, 0, 0, 0) for _ in range(CELL)] for _ in range(CELL)]
    for y in range(CELL):
        for x in range(CELL):
            sx = min(TILE - 1, max(0, x - 1))
            sy = min(TILE - 1, max(0, y - 1))
            result[y][x] = tile[sy][sx]
    return result


def atlas(tiles):
    result = [[(0, 0, 0, 0) for _ in range(CELL * len(tiles))] for _ in range(CELL)]
    for index, tile in enumerate(tiles):
        cell = padded(tile)
        ox = index * CELL
        for y in range(CELL):
            for x in range(CELL):
                result[y][ox + x] = cell[y][x]
    return result


def main():
    os.makedirs(TEXTURE_DIR, exist_ok=True)
    tiles = [
        ("dirt.png", dirt()),
        ("grass_top.png", grass_top()),
        ("grass_side.png", grass_side()),
        ("stone.png", stone()),
        ("sand.png", sand()),
        ("log.png", log()),
        ("leaves.png", leaves()),
        ("water.png", water()),
    ]
    for name, tile in tiles:
        write_png(os.path.join(TEXTURE_DIR, name), tile)
    write_png(os.path.join(TEXTURE_DIR, "block_atlas.png"), atlas([tile for _, tile in tiles]))


if __name__ == "__main__":
    main()
