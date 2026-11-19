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


def test_comparer_ignore_csv_sans_colonne_lecteur(tmp_path):
    def preparer(dossier):
        dossier.mkdir()
        ecrire(dossier / "reference.csv", [["S.zip", "p.dwg", "reference", "AC1032", "", "LINE", 10, ""]])
        ecrire(dossier / "a.csv", [["S.zip", "p.dwg", "lecteurA", "x", 10, "LINE", 8, ""]])
    preparer(tmp_path / "sans")
    preparer(tmp_path / "avec")
    (tmp_path / "avec" / "emprises.csv").write_text(
        "fichier,chemin,insunits,xmin,ymin,xmax,ymax,geodata\nS.zip,p.dwg,6,1,2,3,4,false\n",
        encoding="utf-8")
    comparer(tmp_path / "sans", tmp_path / "sans.md")
    comparer(tmp_path / "avec", tmp_path / "avec.md")
    assert (tmp_path / "avec.md").read_text(encoding="utf-8") == (tmp_path / "sans.md").read_text(encoding="utf-8")
    assert "80,0 %" in (tmp_path / "avec.md").read_text(encoding="utf-8")


def test_normaliser_type_table_unique():
    from comparer import normaliser_type
    # acadrust
    for brut in ("DIMENSION_LINEAR", "DIMENSION_ALIGNED", "DIMENSION_RADIUS", "DIMENSION_DIAMETER",
                 "DIMENSION_ANGULAR_2LINE", "DIMENSION_ANGULAR_3POINT", "DIMENSION_ORDINATE", "DIMENSION"):
        assert normaliser_type(brut) == "DIMENSION"
    # libredwg-web
    assert normaliser_type("POLYLINE2D") == "POLYLINE"
    assert normaliser_type("POLYLINE3D") == "POLYLINE"
    assert normaliser_type("POLYLINE") == "POLYLINE"
    assert normaliser_type("MINSERT") == "INSERT"
    # objets personnalisés, proxys et noms hors types standard
    for brut in ("EXTENDED_ENTITY", "ACAD_PROXY_ENTITY", "?AEC_WALL", "UNKNOWN", "ARCALIGNEDTEXT", "AEC_WALL"):
        assert normaliser_type(brut) == "PERSONNALISEE"
    # types standard conservés
    for brut in ("LINE", "CIRCLE", "LWPOLYLINE", "HATCH", "INSERT", "ACAD_TABLE", "MULTILEADER"):
        assert normaliser_type(brut) == brut


def test_comparer_ramene_les_noms_des_lecteurs_aux_types_ezdxf(tmp_path):
    ecrire(tmp_path / "reference.csv", [
        ["S.zip", "p.dwg", "reference", "AC1032", "", "DIMENSION", 4, ""],
        ["S.zip", "p.dwg", "reference", "AC1032", "", "POLYLINE", 3, ""],
        ["S.zip", "p.dwg", "reference", "AC1032", "", "ARCALIGNEDTEXT", 2, ""],
        ["S.zip", "p.dwg", "reference", "AC1032", "", "LINE", 1, ""],
    ])
    ecrire(tmp_path / "acadrust.csv", [   # noms acadrust bruts
        ["S.zip", "p.dwg", "acadrust", "x", 1, "DIMENSION_LINEAR", 3, ""],
        ["S.zip", "p.dwg", "acadrust", "x", 1, "DIMENSION_RADIUS", 1, ""],
        ["S.zip", "p.dwg", "acadrust", "x", 1, "POLYLINE", 3, ""],
        ["S.zip", "p.dwg", "acadrust", "x", 1, "EXTENDED_ENTITY", 2, ""],
        ["S.zip", "p.dwg", "acadrust", "x", 1, "LINE", 1, ""],
    ])
    ecrire(tmp_path / "libredwg.csv", [   # noms libredwg-web bruts
        ["S.zip", "p.dwg", "libredwg", "x", 1, "DIMENSION", 4, ""],
        ["S.zip", "p.dwg", "libredwg", "x", 1, "POLYLINE2D", 2, ""],
        ["S.zip", "p.dwg", "libredwg", "x", 1, "POLYLINE3D", 1, ""],
        ["S.zip", "p.dwg", "libredwg", "x", 1, "ACAD_PROXY_ENTITY", 2, ""],
        ["S.zip", "p.dwg", "libredwg", "x", 1, "LINE", 1, ""],
    ])
    comparer(tmp_path, tmp_path / "s.md")
    texte = (tmp_path / "s.md").read_text(encoding="utf-8")
    ligne = {l.split("|")[1].strip(): [c.strip() for c in l.split("|")[2:-1]]
             for l in texte.splitlines() if l.startswith("| acadrust") or l.startswith("| libredwg")}
    assert ligne["acadrust"][0] == "100,0 %"
    assert ligne["libredwg"][0] == "100,0 %"
    # les CSV bruts gardent les noms propres à chaque lecteur
    assert "DIMENSION_LINEAR" in (tmp_path / "acadrust.csv").read_text(encoding="utf-8")


def test_comparer_exclut_les_sous_entites_avant_normalisation(tmp_path):
    ecrire(tmp_path / "reference.csv", [
        ["S.zip", "p.dwg", "reference", "AC1032", "", "LINE", 2, ""],
        ["S.zip", "p.dwg", "reference", "AC1032", "", "ATTRIB", 5, ""],
    ])
    ecrire(tmp_path / "a.csv", [["S.zip", "p.dwg", "lecteurA", "x", 1, "LINE", 2, ""]])
    comparer(tmp_path, tmp_path / "s.md")
    assert "100,0 %" in (tmp_path / "s.md").read_text(encoding="utf-8")


def test_cle_normalise_unicode_nfc_avant_la_casse():
    import unicodedata
    from comparer import _cle
    nfd = unicodedata.normalize("NFD", "Plans/Façade.DWG")
    nfc = unicodedata.normalize("NFC", "Plans/Façade.dwg")
    assert nfd != nfc
    assert _cle("Dossier.zip", nfd) == _cle("DOSSIER.zip", nfc)
    # variante : majuscule accentuée, repliée après composition
    assert _cle("a.zip", unicodedata.normalize("NFD", "É.dwg")) == _cle("a.zip", "é.dwg")


def test_attribuer_identifiants_garde_l_existant_et_complete_apres_le_plus_grand():
    from comparer import _cle, attribuer_identifiants
    refs = {}
    for z, c in [("A.zip", "x.dwg"), ("A.zip", "nouveau.dwg"), ("B.zip", "b.dwg"), ("C.zip", "c.dwg")]:
        refs[_cle(z, c)] = {"fichier": z, "chemin": c}
    existants = {
        "zips": {"a.zip": "P01", "c.zip": "P07"},
        "dwg": {_cle("A.zip", "x.dwg"): "P01-3", _cle("C.zip", "c.dwg"): "P07-1"},
        "lignes": {"P01": "A.zip", "P01-3": "A.zip / x.dwg", "P07": "C.zip", "P07-1": "C.zip / c.dwg",
                   "P05-1": "Disparu.zip / d.dwg"},
    }
    zips, id_zip, id_dwg, corr = attribuer_identifiants(refs, existants)
    assert id_zip == {"A.zip": "P01", "B.zip": "P08", "C.zip": "P07"}   # B après le plus grand (P07)
    assert id_dwg[_cle("A.zip", "x.dwg")] == "P01-3"                     # existant conservé
    assert id_dwg[_cle("A.zip", "nouveau.dwg")] == "P01-4"               # après le plus grand du ZIP
    assert id_dwg[_cle("B.zip", "b.dwg")] == "P08-1"
    assert id_dwg[_cle("C.zip", "c.dwg")] == "P07-1"
    assert ("P05-1", "Disparu.zip / d.dwg") in corr                      # identifiant réservé, jamais réattribué


def test_comparer_et_emprises_donnent_le_meme_identifiant_au_meme_zip(tmp_path):
    from emprises import emprises
    contour = tmp_path / "c.geojson"
    contour.write_text('{"type":"Polygon","coordinates":[[[7,43],[8,43],[8,44],[7,44],[7,43]]]}', encoding="utf-8")
    # 1) le comparateur ne connaît que Zeta.zip : P01
    ecrire(tmp_path / "reference.csv", [["Zeta.zip", "z.dwg", "reference", "AC1032", "", "LINE", 1, ""]])
    ecrire(tmp_path / "a.csv", [["Zeta.zip", "z.dwg", "lecteurA", "x", 1, "LINE", 1, ""]])
    comparer(tmp_path, tmp_path / "lecteurs.md")
    # 2) emprises.csv contient aussi Alpha.zip (DWG non converti par ODA), qui sort avant Zeta par ordre alphabétique
    (tmp_path / "emprises.csv").write_text(
        "fichier,chemin,insunits,xmin,ymin,xmax,ymax,geodata\n"
        "Zeta.zip,z.dwg,6,,,,,false\nAlpha.zip,a.dwg,6,,,,,false\n", encoding="utf-8")
    emprises(tmp_path / "emprises.csv", contour, tmp_path / "emprises.md")
    md = (tmp_path / "emprises.md").read_text(encoding="utf-8")
    assert "| P01-1 | inconnue |" in md and "| P02-1 | inconnue |" in md
    corr = {c["nom"]: c["identifiant"] for c in csv.DictReader((tmp_path / "correspondance.csv").open(encoding="utf-8", newline=""))}
    assert corr["Zeta.zip"] == "P01"          # Zeta garde P01 : identifiant du comparateur conservé
    assert corr["Alpha.zip"] == "P02"
    # 3) le comparateur relancé donne les mêmes identifiants (le fichier est complété, pas écrasé)
    comparer(tmp_path, tmp_path / "lecteurs2.md")
    corr2 = {c["nom"]: c["identifiant"] for c in csv.DictReader((tmp_path / "correspondance.csv").open(encoding="utf-8", newline=""))}
    assert corr2["Zeta.zip"] == "P01" and corr2["Alpha.zip"] == "P02"
    assert "P01-1" in (tmp_path / "lecteurs2.md").read_text(encoding="utf-8")
