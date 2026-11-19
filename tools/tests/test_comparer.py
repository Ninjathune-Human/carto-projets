import csv

import pytest

from comparer import comparer

ENTETE = ["fichier", "chemin", "lecteur", "version", "duree_ms", "type", "nombre", "erreur"]


def ecrire(chemin, lignes):
    with open(chemin, "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f)
        w.writerow(ENTETE)
        w.writerows(lignes)


def test_comparer_bout_en_bout(tmp_path):
    ecrire(tmp_path / "reference.csv", [
        ["Secret.zip", "plan1.dwg", "reference", "AC1032", "", "LINE", 10, ""],
        ["Secret.zip", "plan1.dwg", "reference", "AC1032", "", "CIRCLE", 5, ""],
        ["Secret.zip", "Sous/Plan2.dwg", "reference", "AC1032", "", "LINE", 10, ""],
    ])
    ecrire(tmp_path / "a.csv", [
        ["Secret.zip", "plan1.DWG", "lecteurA", "x", 10, "LINE", 10, ""],
        ["Secret.zip", "plan1.DWG", "lecteurA", "x", 10, "CIRCLE", 4, ""],
        ["Secret.zip", "Sous/Plan2.dwg", "lecteurA", "x", 30, "LINE", 10, ""],
    ])
    ecrire(tmp_path / "b.csv", [
        ["Secret.zip", "plan1.dwg", "lecteurB", "x", 5, "LINE", 5, ""],
    ])
    md = tmp_path / "sortie.md"
    comparer(tmp_path, md)
    texte = md.read_text(encoding="utf-8")
    assert "Secret" not in texte and "plan1" not in texte.lower() and "Plan2" not in texte
    assert "P01-1" in texte and "P01-2" in texte
    lignes = {l.split("|")[1].strip(): [c.strip() for c in l.split("|")[2:-1]] for l in texte.splitlines()
              if l.startswith("|")}
    # global A = (14+10)/25 ; B = 5/25
    assert "96,0" in " ".join(lignes["lecteurA"])
    assert "20,0" in " ".join(lignes["lecteurB"])
    corr = list(csv.DictReader(open(tmp_path / "correspondance.csv", encoding="utf-8", newline="")))
    assert {c["identifiant"] for c in corr} >= {"P01", "P01-1", "P01-2"}
    assert any(c["nom"] == "Secret.zip" for c in corr)
