import csv

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
        ["secret.ZIP", "Sous/Plan2.dwg", "lecteurA", "x", 30, "LINE", 10, ""],
    ])
    ecrire(tmp_path / "b.csv", [
        ["Secret.zip", "plan1.dwg", "lecteurB", "x", 5, "LINE", 5, ""],
    ])
    md = tmp_path / "sortie.md"
    comparer(tmp_path, md)
    texte = md.read_text(encoding="utf-8")
    assert "Secret" not in texte and "plan1" not in texte.lower() and "Plan2" not in texte
    assert "P01-1" in texte and "P01-2" in texte
    def cellules(nom, section):
        lignes = texte.split("## Détail")[1 if section == "detail" else 0].splitlines()
        for l in lignes:
            c = [x.strip() for x in l.split("|")[1:-1]]
            if c and c[0] == nom:
                return c
        raise AssertionError(nom)

    # synthèse : lecteur | global | minimum | erreurs/absents | médiane
    assert cellules("lecteurA", "synthese")[1:] == ["96,0 %", "93,3 %", "0", "20"]
    assert cellules("lecteurB", "synthese")[1:] == ["20,0 %", "0,0 %", "1", "5"]
    # détail : colonnes lecteurA, lecteurB
    assert cellules("P01-1", "detail")[1:] == ["93,3 %", "33,3 %"]
    assert cellules("P01-2", "detail")[1:] == ["100,0 %", "0,0 %"]
    corr = list(csv.DictReader((tmp_path / "correspondance.csv").open(encoding="utf-8", newline="")))
    assert {c["identifiant"] for c in corr} >= {"P01", "P01-1", "P01-2"}
    assert any(c["nom"] == "Secret.zip" for c in corr)
