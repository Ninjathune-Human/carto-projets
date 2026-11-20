"""Référence des conversions vers Lambert 93 (pyproj).

Usage : reference_conversions.py <grille.tif> <sortie.csv>
Tire 1 000 points par système non Lambert 93 dans l'emprise de la Métropole
(graine 20261120) et écrit `systeme,x,y,x93,y93`.
"""
import csv
import random
import sys
from pathlib import Path

import pyproj
from pyproj import Transformer

SYSTEMES = [("CC43", 3943), ("CC44", 3944), ("Lambert II étendu", 27572),
            ("Lambert III Sud", 27563), ("Lambert III carto", 27573)]
EMPRISE_WGS84 = (6.80, 43.60, 7.50, 44.40)
PAR_SYSTEME = 1000
GRAINE = 20261120


def lignes(dossier_grille: str) -> list[list]:
    pyproj.datadir.append_data_dir(dossier_grille)
    rng = random.Random(GRAINE)
    sortie = []
    for nom, epsg in SYSTEMES:
        depuis_geo = Transformer.from_crs(4326, epsg, always_xy=True)
        vers_l93 = Transformer.from_crs(epsg, 2154, always_xy=True)
        for _ in range(PAR_SYSTEME):
            lon = rng.uniform(EMPRISE_WGS84[0], EMPRISE_WGS84[2])
            lat = rng.uniform(EMPRISE_WGS84[1], EMPRISE_WGS84[3])
            x, y = depuis_geo.transform(lon, lat)
            x93, y93 = vers_l93.transform(x, y)
            sortie.append([nom, repr(x), repr(y), repr(x93), repr(y93)])
    return sortie


def main(argv: list[str]) -> None:
    grille, sortie = Path(argv[1]), Path(argv[2])
    with open(sortie, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["systeme", "x", "y", "x93", "y93"])
        w.writerows(lignes(str(grille.parent)))


if __name__ == "__main__":
    main(sys.argv)
