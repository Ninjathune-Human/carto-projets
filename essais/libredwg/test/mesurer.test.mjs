import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { zipSync } from "fflate";
import { mesurerZip } from "../mesurer.mjs";

const fixture = readFileSync(new URL("../../../crates/cad/tests/fixtures/deux_lignes_un_cercle.dwg", import.meta.url));

function zipAvec(nom, octets) {
  return zipSync({ [nom]: new Uint8Array(octets) });
}

test("libredwg compte la fixture", async () => {
  const lignes = await mesurerZip(zipAvec("a.dwg", fixture), "t.zip");
  assert.deepEqual(
    lignes.filter((l) => l.type).map((l) => [l.type, l.nombre]).sort(),
    [["CIRCLE", 1], ["LINE", 2]],
  );
  assert.equal(lignes[0].lecteur, "libredwg");
  assert.equal(lignes[0].fichier, "t.zip");
  assert.equal(lignes[0].chemin, "a.dwg");
});

test("un DWG corrompu donne une ligne d'erreur sans lever", async () => {
  const lignes = await mesurerZip(zipAvec("sub\\Mauvais.DWG", Buffer.from("n'importe quoi")), "t.zip");
  assert.equal(lignes.length, 1);
  assert.equal(lignes[0].chemin, "sub/Mauvais.DWG");
  assert.equal(lignes[0].type, "");
  assert.equal(lignes[0].nombre, 0);
  assert.ok(lignes[0].erreur);
});

test("un ZIP illisible donne une ligne sans chemin", async () => {
  const lignes = await mesurerZip(new Uint8Array([1, 2, 3]), "x.zip");
  assert.equal(lignes.length, 1);
  assert.equal(lignes[0].chemin, "");
  assert.ok(lignes[0].erreur);
});
