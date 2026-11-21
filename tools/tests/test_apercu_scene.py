from pathlib import Path

from PIL import Image

from apercu_scene import lire_scene, main

FIXTURE = Path(__file__).resolve().parents[2] / "essais/commun/test/deux_primitives.scn"


def test_lecture_de_la_fixture():
    calques, prims, rapport = lire_scene(FIXTURE.read_bytes())
    assert len(prims) == 2
    assert isinstance(rapport, dict)


def test_png_aux_dimensions_demandees(tmp_path):
    sortie = tmp_path / "a.png"
    assert main([str(FIXTURE), str(sortie), "--largeur", "300"]) == 0
    img = Image.open(sortie)
    assert img.width == 300 and img.height >= 1
    # Non vide : au moins un pixel différent du fond.
    assert img.convert("RGB").getextrema() != ((217, 217), (217, 217), (217, 217))
