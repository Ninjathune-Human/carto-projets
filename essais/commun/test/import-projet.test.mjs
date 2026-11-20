import test from "node:test";
import assert from "node:assert/strict";
import { messageErreur, estZip } from "../import-projet.js";

test("message_par_code_d_erreur", () => {
  assert.equal(messageErreur({ code: "aucun_dwg" }, "a.zip"), "a.zip : aucun dessin DWG dans le ZIP");
  assert.equal(
    messageErreur({ code: "plusieurs_maitres", candidats: ["B.dwg", "A.dwg"] }, "a.zip"),
    "a.zip : plusieurs dessins maîtres possibles, choisir dans la liste",
  );
  // Le message de refus de carto-wasm est affiché tel quel, sans nom de fichier.
  const refus = "Plan non géoréférencé : aucun système reconnu (Lambert 93, CC43, CC44, Lambert II étendu, Lambert III Sud ou carto).";
  assert.equal(messageErreur({ code: "non_georeference", message: refus }, "a.zip"), refus);
  assert.equal(messageErreur({ code: "non_georeference" }, "a.zip"), "Plan non géoréférencé");
  assert.equal(messageErreur({ code: "lecture", message: "DWG illisible" }, "a.zip"), "a.zip : lecture impossible (DWG illisible)");
  assert.equal(messageErreur({ code: "autre", x: 1 }, "a.zip"), 'a.zip : import impossible ({"code":"autre","x":1})');
});

test("zip_reconnu_a_l_extension_ou_au_type", () => {
  assert.ok(estZip({ name: "Projet.ZIP", type: "" }));
  assert.ok(estZip({ name: "projet", type: "application/zip" }));
  assert.ok(estZip({ name: "projet", type: "application/x-zip-compressed" }));
  assert.ok(!estZip({ name: "plan.lignes", type: "" }));
  assert.ok(!estZip({ name: "zip.png", type: "image/png" }));
});
