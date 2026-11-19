"""Compare les lecteurs DWG à la référence ODA/ezdxf sur le schéma CSV commun."""
import csv
import statistics
import sys
import unicodedata
from collections import defaultdict
from pathlib import Path

SOUS_ENTITES = {"ATTRIB", "SEQEND", "VERTEX"}
REFERENCE = "reference"

# R12 : une seule table de noms. Les types de référence sont ceux d'ezdxf (noms DXF) ; les CSV bruts
# gardent les noms propres à chaque lecteur (acadrust : DIMENSION_LINEAR, EXTENDED_ENTITY, ?NOM ;
# libredwg-web : POLYLINE2D, POLYLINE3D, ACAD_PROXY_ENTITY, ARCALIGNEDTEXT).
PERSONNALISEE = "PERSONNALISEE"
ALIAS_TYPES = {
    "POLYLINE2D": "POLYLINE",
    "POLYLINE3D": "POLYLINE",
    "MINSERT": "INSERT",
    "MLEADER": "MULTILEADER",
}
TYPES_STANDARD = {
    "3DFACE", "3DSOLID", "ACAD_TABLE", "ARC", "ARC_DIMENSION", "ATTDEF", "BODY", "CIRCLE", "DIMENSION",
    "ELLIPSE", "EXTRUDEDSURFACE", "HATCH", "HELIX", "IMAGE", "INSERT", "LARGE_RADIAL_DIMENSION",
    "LEADER", "LIGHT", "LINE", "LOFTEDSURFACE", "LWPOLYLINE", "MESH", "MLINE", "MPOLYGON", "MTEXT",
    "MULTILEADER", "OLE2FRAME", "PLANESURFACE", "POINT", "POLYLINE", "RAY", "REGION", "REVOLVEDSURFACE",
    "SHAPE", "SOLID", "SPLINE", "SURFACE", "SWEPTSURFACE", "TEXT", "TOLERANCE", "TRACE", "VIEWPORT",
    "WIPEOUT", "XLINE",
}


def normaliser_type(nom: str) -> str:
    """Ramène un nom de type de lecteur au type DXF d'ezdxf ; hors types standard : PERSONNALISEE."""
    if nom.startswith("DIMENSION_"):
        return "DIMENSION"
    nom = ALIAS_TYPES.get(nom, nom)
    return nom if nom in TYPES_STANDARD else PERSONNALISEE


def taux(ref: dict[str, int], lu: dict[str, int]) -> float | None:
    """Σ min(lu, ref) / Σ ref, hors sous-entités ; None si la référence est vide."""
    ref = {t: n for t, n in ref.items() if t not in SOUS_ENTITES}
    total = sum(ref.values())
    if total == 0:
        return None
    return sum(min(lu.get(t, 0), n) for t, n in ref.items()) / total


def _cle(fichier: str, chemin: str) -> tuple[str, str]:
    c = unicodedata.normalize("NFC", chemin.replace("\\", "/"))
    if c.lower().endswith(".dwg"):
        c = c[:-4]
    return unicodedata.normalize("NFC", fichier).lower(), c.lower()


def _lire_csv(dossier: Path):
    lignes = []
    for f in sorted(dossier.glob("*.csv")):
        if f.name == "correspondance.csv":
            continue
        with open(f, encoding="utf-8", newline="") as h:
            lecteur = csv.DictReader(h)
            if "lecteur" not in (lecteur.fieldnames or []):
                continue  # autre CSV du dossier (ex. emprises.csv)
            lignes.extend(lecteur)
    return lignes


def _entier(v):
    try:
        return int(v)
    except (TypeError, ValueError):
        return 0


def _flottant(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def _pct(x) -> str:
    return "—" if x is None else f"{x * 100:.1f}".replace(".", ",") + " %"


def attribuer_identifiants(refs: dict[tuple, dict]):
    """Identifiants anonymes : P01… par ZIP trié, P01-1… par DWG trié dans le ZIP.

    refs : cle (voir _cle) -> {"fichier", "chemin", ...}.
    Retourne (zips, id_zip, id_dwg, correspondance).
    """
    zips = sorted({e["fichier"] for e in refs.values()})
    id_zip = {z: f"P{i:02d}" for i, z in enumerate(zips, 1)}
    id_dwg, correspondance = {}, []
    for z in zips:
        correspondance.append((id_zip[z], z))
        cles = sorted((k for k, e in refs.items() if e["fichier"] == z), key=lambda k: k[1])
        for i, k in enumerate(cles, 1):
            id_dwg[k] = f"{id_zip[z]}-{i}"
            correspondance.append((id_dwg[k], f"{z} / {refs[k]['chemin']}"))
    return zips, id_zip, id_dwg, correspondance


def comparer(dossier_resultats: Path, sortie_md: Path) -> None:
    dossier = Path(dossier_resultats)
    refs: dict[tuple, dict] = {}   # cle -> {"fichier","chemin","comptes"}
    lecteurs: dict[str, dict[tuple, dict]] = defaultdict(dict)
    for l in _lire_csv(dossier):
        if not l.get("chemin"):
            continue  # ZIP illisible : pas de DWG identifiable
        cle = _cle(l["fichier"], l["chemin"])
        cible = refs if l["lecteur"] == REFERENCE else lecteurs[l["lecteur"]]
        e = cible.setdefault(cle, {"fichier": l["fichier"], "chemin": l["chemin"], "comptes": {},
                                   "erreur": False, "duree": None})
        if l.get("erreur"):
            e["erreur"] = True
        elif l.get("duree_ms") not in (None, ""):
            e["duree"] = _flottant(l["duree_ms"])
        if l.get("type") and l["type"] not in SOUS_ENTITES:  # exclues avant le regroupement
            t = normaliser_type(l["type"])
            e["comptes"][t] = e["comptes"].get(t, 0) + _entier(l["nombre"])

    zips, id_zip, id_dwg, correspondance = attribuer_identifiants(refs)
    ordre = sorted(refs, key=lambda k: (id_zip[refs[k]["fichier"]], int(id_dwg[k].split("-")[1])))

    noms = sorted(lecteurs)
    par_lecteur = {n: {} for n in noms}   # nom -> cle -> taux ou None
    resume = []
    for n in noms:
        somme_min = somme_ref = 0
        taux_dwg, erreurs, durees = [], 0, []
        for k in ordre:
            r = refs[k]["comptes"]
            r_util = {t: v for t, v in r.items() if t not in SOUS_ENTITES}
            lu = lecteurs[n].get(k)
            if refs[k]["erreur"] or sum(r_util.values()) == 0:
                par_lecteur[n][k] = None
                continue
            lus = {}
            if lu is None or lu["erreur"]:
                erreurs += 1
            else:
                lus = lu["comptes"]
                if lu["duree"] is not None:
                    durees.append(lu["duree"])
            t = taux(r, lus)
            somme_ref += sum(r_util.values())
            somme_min += sum(min(lus.get(x, 0), v) for x, v in r_util.items())
            taux_dwg.append(t)
            par_lecteur[n][k] = t
        glob = somme_min / somme_ref if somme_ref else None
        mini = min(taux_dwg) if taux_dwg else None
        med = statistics.median(durees) if durees else None
        resume.append((n, glob, mini, erreurs, med))

    md = ["# Comparaison des lecteurs DWG", "",
          f"Référence : {len(refs)} DWG (DXF produits par ODA File Converter, comptés avec ezdxf).", "",
          "## Synthèse", "",
          "| Lecteur | Taux global | Taux minimum par DWG | DWG en erreur ou absents | Durée médiane (ms) |",
          "|---|---|---|---|---|"]
    for n, g, m, e, d in resume:
        dur = "—" if d is None else f"{d:.0f}"
        md.append(f"| {n} | {_pct(g)} | {_pct(m)} | {e} | {dur} |")
    md += ["", "## Détail par DWG", "", "| DWG | " + " | ".join(noms) + " |",
           "|---|" + "---|" * len(noms)]
    for k in ordre:
        md.append(f"| {id_dwg[k]} | " + " | ".join(_pct(par_lecteur[n][k]) for n in noms) + " |")
    md.append("")

    sortie_md = Path(sortie_md)
    sortie_md.parent.mkdir(parents=True, exist_ok=True)
    sortie_md.write_text("\n".join(md), encoding="utf-8")
    with open(dossier / "correspondance.csv", "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["identifiant", "nom"])
        w.writerows(correspondance)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage : python tools/comparer.py <dossier_resultats> <sortie.md>")
    comparer(Path(sys.argv[1]), Path(sys.argv[2]))
