"""Dessine une scène SCN1 en PNG : primitives dans l'ordre, surfaces pleines, traits à leur épaisseur."""
import argparse
import json
import struct
import sys

import numpy as np
from PIL import Image, ImageDraw


def lire_scene(o):
    """Lit un SCN1 : (calques [(nom, rgb)], primitives, rapport).

    Primitive : dict genre (0 trait, 1 surface), rgba, calque, mm, pts (tableau n x 2 de f64).
    """
    if o[:4] != b"SCN1":
        raise ValueError("SCN1 invalide")
    i = 4

    def u32():
        nonlocal i
        v = struct.unpack_from("<I", o, i)[0]
        i += 4
        return v

    calques = []
    for _ in range(u32()):
        n = u32()
        nom = o[i:i + n].decode("utf-8")
        i += n
        calques.append((nom, tuple(o[i:i + 3])))
        i += 4
    prims = []
    for _ in range(u32()):
        genre = o[i]
        rgba = tuple(o[i + 1:i + 5])
        i += 5
        calque = u32()
        mm = struct.unpack_from("<f", o, i)[0]
        i += 4
        n = u32()
        pts = np.frombuffer(o, dtype="<f8", count=2 * n, offset=i).reshape(n, 2)
        i += 16 * n
        prims.append({"genre": genre, "rgba": rgba, "calque": calque, "mm": mm, "pts": pts})
    n = u32()
    rapport = json.loads(o[i:i + n].decode("utf-8"))
    return calques, prims, rapport


def _couleur(c):
    c = c.lstrip("#")
    if len(c) == 3:
        c = "".join(ch * 2 for ch in c)
    return tuple(int(c[k:k + 2], 16) for k in (0, 2, 4))


def _emprise(prims, rapport):
    if prims:
        tous = np.concatenate([p["pts"] for p in prims if len(p["pts"])] or [np.zeros((1, 2))])
        return tous[:, 0].min(), tous[:, 1].min(), tous[:, 0].max(), tous[:, 1].max()
    e = rapport.get("emprise")
    return tuple(e) if e else (0.0, 0.0, 1.0, 1.0)


def dessiner(prims, rapport, largeur=2000, fond="#d9d9d9"):
    xmin, ymin, xmax, ymax = _emprise(prims, rapport)
    w, h = xmax - xmin, ymax - ymin
    echelle = (largeur - 1) / w if w > 0 else 1.0
    hauteur = max(1, int(round(h * echelle)) + 1) if w > 0 else 1
    img = Image.new("RGB", (largeur, hauteur), _couleur(fond))
    d = ImageDraw.Draw(img)
    for p in prims:
        if not len(p["pts"]):
            continue
        px = (p["pts"][:, 0] - xmin) * echelle
        py = (ymax - p["pts"][:, 1]) * echelle
        pts = list(zip(px.tolist(), py.tolist()))
        r, g, b, a = p["rgba"]
        if p["genre"] == 1:
            for k in range(0, len(pts) - 2, 3):
                tri = pts[k:k + 3]
                if a == 255:
                    d.polygon(tri, fill=(r, g, b))
                else:
                    _melanger(img, tri, (r, g, b), a)
        elif len(pts) >= 2:
            epaisseur = max(1, int(round(p["mm"] * 96 / 25.4)))
            d.line(pts, fill=(r, g, b), width=epaisseur, joint="curve")
    return img


def _melanger(img, tri, rgb, a):
    x0 = max(0, int(min(p[0] for p in tri)) - 1)
    y0 = max(0, int(min(p[1] for p in tri)) - 1)
    x1 = min(img.width, int(max(p[0] for p in tri)) + 2)
    y1 = min(img.height, int(max(p[1] for p in tri)) + 2)
    if x1 <= x0 or y1 <= y0:
        return
    masque = Image.new("L", (x1 - x0, y1 - y0), 0)
    ImageDraw.Draw(masque).polygon([(x - x0, y - y0) for x, y in tri], fill=a)
    img.paste(Image.new("RGB", masque.size, rgb), (x0, y0), masque)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("entree")
    ap.add_argument("sortie")
    ap.add_argument("--fond", default="#d9d9d9")
    ap.add_argument("--largeur", type=int, default=2000)
    a = ap.parse_args(argv)
    with open(a.entree, "rb") as f:
        _, prims, rapport = lire_scene(f.read())
    Image.MAX_IMAGE_PIXELS = None
    dessiner(prims, rapport, a.largeur, a.fond).save(a.sortie)
    return 0


if __name__ == "__main__":
    sys.exit(main())
