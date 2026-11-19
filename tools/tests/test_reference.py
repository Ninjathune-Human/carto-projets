import csv

import ezdxf
import pytest

from comparer import taux
from reference import compter_dxf, ecrire_reference


def test_compter_dxf_ne_compte_que_l_espace_objet(tmp_path):
    doc = ezdxf.new("R2018")
    msp = doc.modelspace()
    msp.add_line((0, 0), (1, 0))
    msp.add_line((0, 0), (0, 1))
    msp.add_circle((0, 0), 1)
    doc.blocks.new("B").add_line((0, 0), (1, 1))
    doc.saveas(tmp_path / "t.dxf")
    assert compter_dxf(tmp_path / "t.dxf") == {"LINE": 2, "CIRCLE": 1}


def test_taux_ignore_les_types_en_plus():
    assert taux({"LINE": 10, "CIRCLE": 5}, {"LINE": 10, "CIRCLE": 4, "HATCH": 3}) == pytest.approx(14 / 15)


def test_taux_exclut_les_sous_entites():
    assert taux({"LINE": 1, "ATTRIB": 5}, {"LINE": 1}) == 1.0


def test_taux_reference_vide():
    assert taux({}, {}) is None


def test_ecrire_reference_schema_commun(tmp_path):
    ref = tmp_path / "ref"
    (ref / "Lot A" / "sous").mkdir(parents=True)
    doc = ezdxf.new("R2018")
    doc.modelspace().add_line((0, 0), (1, 0))
    doc.saveas(ref / "Lot A" / "sous" / "plan.dxf")
    (ref / "Lot A" / "casse.dxf").write_text("pas un dxf")
    sortie = tmp_path / "out" / "reference.csv"
    ecrire_reference(ref, sortie)
    lignes = list(csv.DictReader(open(sortie, encoding="utf-8", newline="")))
    assert open(sortie, encoding="utf-8").readline().strip() == "fichier,chemin,lecteur,version,duree_ms,type,nombre,erreur"
    ok = [l for l in lignes if l["chemin"] == "sous/plan.dwg"]
    assert ok == [{"fichier": "Lot A.zip", "chemin": "sous/plan.dwg", "lecteur": "reference",
                   "version": "AC1032", "duree_ms": "", "type": "LINE", "nombre": "1", "erreur": ""}]
    ko = [l for l in lignes if l["chemin"] == "casse.dwg"]
    assert len(ko) == 1 and ko[0]["type"] == "" and ko[0]["nombre"] == "0" and ko[0]["erreur"]
