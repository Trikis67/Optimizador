"""Genera las imágenes del instalador NSIS con la marca Trikis.

Uso (desde la raíz del proyecto):  python branding/instalador_imagenes.py
Crea src-tauri/installer/sidebar.bmp (164x314) y header.bmp (150x57).
"""
import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "installer"
LOGO = ROOT / "docs" / "logo.png"
FONTS = ROOT / "src" / "fonts"

BG = (26, 25, 22)
BG2 = (36, 33, 27)
AMBER = (209, 154, 60)
TEXT = (239, 232, 214)
MUTED = (169, 158, 134)
LINE = (59, 53, 44)

K = 3  # dibujamos a 3x y reducimos para suavizar


def font(name, size):
    return ImageFont.truetype(str(FONTS / name), size * K)


def gradient(w, h, top, bottom):
    img = Image.new("RGB", (w, h), top)
    d = ImageDraw.Draw(img)
    for y in range(h):
        t = y / max(1, h - 1)
        d.line([(0, y), (w, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)))
    return img


def glow(img, center, radius, color, alpha):
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    cx, cy = center
    d.ellipse([cx - radius, cy - radius, cx + radius, cy + radius], fill=color + (alpha,))
    layer = layer.filter(ImageFilter.GaussianBlur(radius / 2))
    return Image.alpha_composite(img.convert("RGBA"), layer)


def centered(d, y, text, f, fill, w, spacing=0):
    if spacing:
        widths = [d.textlength(c, font=f) for c in text]
        total = sum(widths) + spacing * (len(text) - 1)
        x = (w - total) / 2
        for c, cw in zip(text, widths):
            d.text((x, y), c, font=f, fill=fill)
            x += cw + spacing
    else:
        tw = d.textlength(text, font=f)
        d.text(((w - tw) / 2, y), text, font=f, fill=fill)


def sidebar():
    w, h = 164 * K, 314 * K
    img = gradient(w, h, BG2, BG)
    img = glow(img, (w // 2, 92 * K), 70 * K, AMBER, 60)
    d = ImageDraw.Draw(img)

    # marcas de velocímetro alrededor del logo
    cx, cy = w // 2, 92 * K
    for i in range(29):
        a = math.radians(210 - i * 240 / 28)
        r1, r2 = (68 if i % 4 == 0 else 72) * K, 77 * K
        col = (190, 70, 45) if i >= 24 else (AMBER if i % 4 == 0 else LINE)
        d.line([(cx + r1 * math.cos(a), cy - r1 * math.sin(a)), (cx + r2 * math.cos(a), cy - r2 * math.sin(a))],
               fill=col, width=(2 if i % 4 == 0 else 1) * K)

    logo = Image.open(LOGO).convert("RGBA").resize((118 * K, 118 * K), Image.LANCZOS)
    img.alpha_composite(logo, (cx - 59 * K, cy - 59 * K))
    d = ImageDraw.Draw(img)

    centered(d, 176 * K, "OPTIMIZADOR", font("AlfaSlabOne-400.woff2", 17), TEXT, w, spacing=1 * K)
    centered(d, 200 * K, "por Trikis", font("SpecialElite-400.woff2", 12), AMBER, w)
    d.line([(46 * K, 224 * K), (118 * K, 224 * K)], fill=LINE, width=1 * K)
    f = font("AlegreyaSans-400.woff2", 11)
    for i, line in enumerate(["Rendimiento · Limpieza", "Drivers · Salud del sistema"]):
        centered(d, (234 + i * 15) * K, line, f, MUTED, w)
    centered(d, 290 * K, "trikis.nl", font("SpecialElite-400.woff2", 10), MUTED, w)
    # borde derecho ámbar fino
    d.rectangle([w - 2 * K, 0, w, h], fill=AMBER)
    return img.convert("RGB").resize((164, 314), Image.LANCZOS)


def header():
    w, h = 150 * K, 57 * K
    img = Image.new("RGBA", (w, h), BG + (255,))
    img = glow(img, (w - 30 * K, h // 2), 26 * K, AMBER, 50)
    logo = Image.open(LOGO).convert("RGBA").resize((46 * K, 46 * K), Image.LANCZOS)
    img.alpha_composite(logo, (w - 52 * K, (h - 46 * K) // 2))
    d = ImageDraw.Draw(img)
    d.text((8 * K, 12 * K), "Optimizador", font=font("AlfaSlabOne-400.woff2", 13), fill=TEXT)
    d.text((8 * K, 31 * K), "por Trikis", font=font("SpecialElite-400.woff2", 10), fill=AMBER)
    return img.convert("RGB").resize((150, 57), Image.LANCZOS)


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    sidebar().save(OUT / "sidebar.bmp")
    header().save(OUT / "header.bmp")
    print("Listo:", OUT)
