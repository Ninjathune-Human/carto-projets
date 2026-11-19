import csv
import subprocess
import sys
from pathlib import Path

from pyproj import Transformer
from shapely.geometry import Polygon

from emprises import classer

CARRE = Polygon([(0, 0), (100, 0), (100, 100), (0, 100)])


def test_emprise_inconnue():
    assert classer(None, CARRE) == ("inconnue", None)


def test_dedans_en_metres():
    assert classer((10, 10, 20, 20), CARRE) == ("dedans", 1)


def test_plan_en_centimetres():
    assert classer((1000, 1000, 2000, 2000), CARRE) == ("dedans", 0.01)


def test_plan_en_millimetres():
    assert classer((10000, 10000, 20000, 20000), CARRE) == ("dedans", 0.001)


def test_a_cheval():
    assert classer((90, 90, 110, 110), CARRE) == ("a_cheval", 1)


def test_dehors():
    assert classer((2e6, 2e6, 2e6 + 10, 2e6 + 10), CARRE)[0] == "dehors"


def test_cli(tmp_path):
    contour = tmp_path / "contour.geojson"
    contour.write_text(
        '{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":'
        '[[[7.20,43.65],[7.30,43.65],[7.30,43.75],[7.20,43.75],[7.20,43.65]]]}}',
        encoding="utf-8")
    x, y = Transformer.from_crs(4326, 2154, always_xy=True).transform(7.25, 43.70)
    colonnes = ["fichier", "chemin", "insunits", "xmin", "ymin", "xmax", "ymax", "geodata"]
    lignes = [
        ["SecretClient.zip", "plans/Alpha.dwg", "6", x, y, x + 50, y + 50, "false"],
        ["SecretClient.zip", "plans/Beta.dwg", "5", x * 100, y * 100, (x + 50) * 100, (y + 50) * 100, "true"],
        ["Autre.zip", "Gamma.dwg", "", "", "", "", "", "false"],
    ]
    emp = tmp_path / "emprises.csv"
    with open(emp, "w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(colonnes)
        w.writerows(lignes)
    sortie = tmp_path / "out" / "emprises.md"
    r = subprocess.run([sys.executable, str(Path(__file__).parents[1] / "emprises.py"),
                        str(emp), str(contour), str(sortie)], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == str(sortie)
    md = sortie.read_text(encoding="utf-8")
    assert "| P02-1 | dedans | 1 | 6 | non |" in md
    assert "| P02-2 | dedans | 0,01 | 5 | oui |" in md
    assert "| P01-1 | inconnue |" in md
    for nom in ("SecretClient", "Alpha", "Beta", "Gamma", "Autre"):
        assert nom not in md


def test_cli_contour_absent(tmp_path):
    emp = tmp_path / "emprises.csv"
    emp.write_text("fichier,chemin,insunits,xmin,ymin,xmax,ymax,geodata\n", encoding="utf-8")
    r = subprocess.run([sys.executable, str(Path(__file__).parents[1] / "emprises.py"),
                        str(emp), str(tmp_path / "absent.geojson"), str(tmp_path / "o.md")],
                       capture_output=True, text=True)
    assert r.returncode == 1
    assert "absent.geojson" in r.stderr


def _cli(tmp_path, geojson):
    contour = tmp_path / "contour.geojson"
    contour.write_text(geojson, encoding="utf-8")
    emp = tmp_path / "emprises.csv"
    emp.write_text("fichier,chemin,insunits,xmin,ymin,xmax,ymax,geodata\nA.zip,a.dwg,6,1,2,3,4,false\n", encoding="utf-8")
    return subprocess.run([sys.executable, str(Path(__file__).parents[1] / "emprises.py"),
                           str(emp), str(contour), str(tmp_path / "o.md")], capture_output=True, text=True)


def test_cli_contour_en_lambert_93_refuse(tmp_path):
    r = _cli(tmp_path, '{"type":"Polygon","coordinates":[[[1000000,6300000],[1010000,6300000],'
                       '[1010000,6310000],[1000000,6310000],[1000000,6300000]]]}')
    assert r.returncode == 1
    assert "WGS84" in r.stderr
    assert not (tmp_path / "o.md").exists()


def test_cli_contour_avec_crs_non_wgs84_refuse(tmp_path):
    r = _cli(tmp_path, '{"type":"FeatureCollection","crs":{"type":"name","properties":{"name":"urn:ogc:def:crs:EPSG::2154"}},'
                       '"features":[{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":'
                       '[[[7.2,43.6],[7.3,43.6],[7.3,43.7],[7.2,43.7],[7.2,43.6]]]}}]}')
    assert r.returncode == 1
    assert "WGS84" in r.stderr


def test_cli_contour_avec_crs_wgs84_accepte(tmp_path):
    r = _cli(tmp_path, '{"type":"FeatureCollection","crs":{"type":"name","properties":{"name":"urn:ogc:def:crs:OGC:1.3:CRS84"}},'
                       '"features":[{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":'
                       '[[[7.2,43.6],[7.3,43.6],[7.3,43.7],[7.2,43.7],[7.2,43.6]]]}}]}')
    assert r.returncode == 0, r.stderr
