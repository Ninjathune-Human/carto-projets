"""Référence : compte les entités des DXF produits par ODA File Converter (ezdxf).

Disposition attendue : <dossier_ref>/<nom du ZIP sans .zip>/<chemin du DWG dans le ZIP>.dxf
Sortie : schéma CSV commun (fichier,chemin,lecteur,version,duree_ms,type,nombre,erreur).
"""
import csv
import sys
from collections import Counter
from pathlib import Path

import ezdxf

ENTETE = ["fichier", "chemin", "lecteur", "version", "duree_ms", "type", "nombre", "erreur"]
SOUS_ENTITES = {"ATTRIB", "SEQEND", "VERTEX"}


def _lire(chemin: Path):
    doc = ezdxf.readfile(chemin)
    comptes = Counter(e.dxftype() for e in doc.modelspace() if e.dxftype() not in SOUS_ENTITES)
    return doc.dxfversion, dict(comptes)


def compter_dxf(chemin: Path) -> dict[str, int]:
    """Entités de l'espace objet, hors sous-entités exclues."""
    return _lire(Path(chemin))[1]


def lignes_reference(dossier_ref: Path) -> list[dict]:
    dossier_ref = Path(dossier_ref)
    lignes = []
    for dxf in sorted(p for p in dossier_ref.rglob("*") if p.is_file() and p.suffix.lower() == ".dxf"):
        rel = dxf.relative_to(dossier_ref).as_posix().split("/")
        if len(rel) < 2:
            continue
        base = {"fichier": rel[0] + ".zip", "chemin": "/".join(rel[1:])[: -len(dxf.suffix)] + ".dwg",
                "lecteur": "reference", "version": "", "duree_ms": "", "type": "", "nombre": 0, "erreur": ""}
        try:
            version, comptes = _lire(dxf)
        except Exception as e:  # DXF illisible : ligne d'erreur
            lignes.append({**base, "erreur": " ".join(f"{type(e).__name__}: {e}".split())})
            continue
        base["version"] = version
        if not comptes:
            lignes.append(base)
        for t in sorted(comptes):
            lignes.append({**base, "type": t, "nombre": comptes[t]})
    return lignes


def ecrire_reference(dossier_ref: Path, sortie: Path) -> None:
    lignes = lignes_reference(dossier_ref)
    Path(sortie).parent.mkdir(parents=True, exist_ok=True)
    with open(sortie, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=ENTETE, lineterminator="\n")
        w.writeheader()
        w.writerows(lignes)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage : python tools/reference.py <dossier_ref> <sortie.csv>")
    ecrire_reference(Path(sys.argv[1]), Path(sys.argv[2]))
