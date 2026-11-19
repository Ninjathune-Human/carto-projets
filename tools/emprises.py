"""Classe l'emprise de chaque DWG (EXTMIN/EXTMAX) par rapport au contour de la Métropole (L93)."""
import csv
import json
import sys
from pathlib import Path

from pyproj import Transformer
from shapely.geometry import box, shape
from shapely.ops import transform, unary_union

from comparer import _cle, identifiants_du_dossier

FACTEURS = (1, 0.01, 0.001)
CLASSES = ("dedans", "a_cheval", "dehors", "inconnue")


def classer(emprise, contour):
    """Retourne (classe, facteur) ; le facteur convertit les unités du dessin en mètres."""
    if emprise is None:
        return "inconnue", None
    for f in FACTEURS:
        b = box(*(v * f for v in emprise))
        if b.within(contour):
            return "dedans", f
        if b.intersects(contour) and not b.touches(contour):  # contact seul = pas de recouvrement
            return "a_cheval", f
    return "dehors", None


def charger_contour(chemin: Path):
    """Union de tous les polygones du GeoJSON (WGS84), reprojetée en EPSG:2154."""
    data = json.loads(Path(chemin).read_text(encoding="utf-8"))
    if data.get("type") == "FeatureCollection":
        geoms = [shape(f["geometry"]) for f in data["features"] if f.get("geometry")]
    elif data.get("type") == "Feature":
        geoms = [shape(data["geometry"])]
    else:
        geoms = [shape(data)]
    union = unary_union(geoms)
    t = Transformer.from_crs(4326, 2154, always_xy=True)
    return transform(t.transform, union)


def _flottant(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def _emprise(l):
    v = [_flottant(l.get(k)) for k in ("xmin", "ymin", "xmax", "ymax")]
    return None if None in v else tuple(v)


def emprises(csv_emprises: Path, geojson: Path, sortie_md: Path) -> None:
    csv_emprises = Path(csv_emprises)
    contour = charger_contour(geojson)
    with open(csv_emprises, encoding="utf-8", newline="") as h:
        lignes = [l for l in csv.DictReader(h) if l.get("chemin")]
    refs = {_cle(l["fichier"], l["chemin"]): {"fichier": l["fichier"], "chemin": l["chemin"]}
            for l in lignes}
    ids = identifiants_du_dossier(csv_emprises.parent, refs)[2]  # complète correspondance.csv
    lignes.sort(key=lambda l: (int(ids[_cle(l["fichier"], l["chemin"])][1:].split("-")[0]),
                               int(ids[_cle(l["fichier"], l["chemin"])].split("-")[1])))
    res = [(ids[_cle(l["fichier"], l["chemin"])], *classer(_emprise(l), contour),
            l.get("insunits") or "—", "oui" if (l.get("geodata") or "").strip().lower() == "true" else "non")
           for l in lignes]

    md = ["# Emprises des DWG par rapport au contour de la Métropole", "",
          f"{len(res)} DWG ; emprise lue dans l'en-tête (EXTMIN/EXTMAX), supposée en Lambert 93 "
          "après application du facteur d'unité (1, 0,01 puis 0,001).", "",
          "## Synthèse", ""]
    for c in CLASSES:
        md.append(f"- {c} : {sum(1 for r in res if r[1] == c)}")
    md += ["", "## Détail", "", "| Id | Classe | Facteur | INSUNITS | GeoData |", "|---|---|---|---|---|"]
    for i, c, f, u, g in res:
        fs = "—" if f is None else str(f).replace(".", ",")
        md.append(f"| {i} | {c} | {fs} | {u} | {g} |")
    md.append("")
    sortie_md = Path(sortie_md)
    sortie_md.parent.mkdir(parents=True, exist_ok=True)
    sortie_md.write_text("\n".join(md), encoding="utf-8")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit("usage : python3 tools/emprises.py <emprises.csv> <contour.geojson> <sortie.md>")
    if not Path(sys.argv[2]).is_file():
        print(f"erreur : contour introuvable : {sys.argv[2]}", file=sys.stderr)
        sys.exit(1)
    emprises(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))
    print(sys.argv[3])
