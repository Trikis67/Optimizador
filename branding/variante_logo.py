import math, sys
from PIL import Image, ImageDraw
src, out = sys.argv[1], sys.argv[2]
base = Image.open(src).convert("RGBA")          # 500x500
K = 4; W = 500*K
lay = Image.new("RGBA", (W, W), (0,0,0,0)); d = ImageDraw.Draw(lay)
cx = cy = 250*K
ink = (42, 28, 15, 255); amber = (209, 154, 60, 255); red = (190, 70, 45, 255)
# marcas de velocímetro: de 210° a -30° pasando por arriba
n = 25
for i in range(n):
    a = math.radians(210 - i*240/(n-1))
    major = i % 4 == 0
    r1, r2 = (166 if major else 174)*K, 190*K
    col = red if i >= n-5 else ink
    w = (7 if major else 4)*K
    p1 = (cx + r1*math.cos(a), cy - r1*math.sin(a)); p2 = (cx + r2*math.cos(a), cy - r2*math.sin(a))
    d.line([p1, p2], fill=col, width=w)
# arco fino de la escala
r = 194*K
d.arc([cx-r, cy-r, cx+r, cy+r], start=-210, end=30, fill=ink, width=3*K)
# insignia con rayo
bx, by, br = 392*K, 392*K, 64*K
d.ellipse([bx-br-10*K, by-br-10*K, bx+br+10*K, by+br+10*K], fill=(232, 214, 176, 255))
d.ellipse([bx-br, by-br, bx+br, by+br], fill=amber, outline=ink, width=8*K)
bolt = [(8,-44),(-26,6),(-2,6),(-10,44),(26,-8),(2,-8),(12,-44)]
d.polygon([(bx+x*K*1.05, by+y*K*1.05) for x,y in bolt], fill=ink)
lay = lay.resize((500, 500), Image.LANCZOS)
img = Image.alpha_composite(base, lay)
# ojo con brillo ámbar (energía)
e = Image.new("RGBA", (500*K,500*K)); de = ImageDraw.Draw(e)
ex, ey, er = 289*K, 230*K, 11*K
de.ellipse([ex-er, ey-er, ex+er, ey+er], fill=(240, 170, 60, 255))
img = Image.alpha_composite(img, e.resize((500,500), Image.LANCZOS))
img.save(out)
