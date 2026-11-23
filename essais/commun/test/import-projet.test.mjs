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

// --- Pièges, délai et renouvellement du worker (R12) ---

import { mock } from "node:test";
import { readFileSync } from "node:fs";
import { creerImporteur, erreurMoteur, DELAI_IMPORT_MS } from "../import-projet.js";

const PIEGE = "Lecture impossible : un dessin du ZIP a interrompu le moteur.";
const DELAI = "Import trop long : abandon après 120 s.";

test("piege_et_delai_ont_leur_message", () => {
  assert.equal(messageErreur({ code: "piege", message: "unreachable" }, "a.zip"), PIEGE);
  assert.equal(messageErreur({ code: "delai" }, "a.zip"), DELAI);
  assert.equal(DELAI_IMPORT_MS, 120_000);
});

test("exception_du_moteur_json_ou_piege", () => {
  // Le JSON structuré de carto-wasm passe tel quel.
  assert.deepEqual(erreurMoteur('{"code":"aucun_dwg"}'), { code: "aucun_dwg" });
  assert.deepEqual(
    erreurMoteur('{"code":"lecture","message":"DWG illisible"}'),
    { code: "lecture", message: "DWG illisible" },
  );
  // Tout autre texte est un piège wasm (unreachable, RuntimeError…).
  for (const texte of ["unreachable", "unreachable executed", "RuntimeError: memory access out of bounds", "", "42", "null", '{"x":1}']) {
    assert.deepEqual(erreurMoteur(texte), { code: "piege", message: texte }, texte);
  }
});

/** Worker factice : garde les messages reçus et note sa terminaison. */
class FauxWorker {
  static crees = [];
  constructor() {
    this.termine = false;
    this.recus = [];
    FauxWorker.crees.push(this);
  }
  postMessage(m) {
    this.recus.push(m);
  }
  terminate() {
    this.termine = true;
  }
}

// Minuteries factices pour chaque import : le délai de 120 s ne retient jamais le processus.
test.afterEach(() => mock.timers.reset());

function banc() {
  mock.timers.enable({ apis: ["setTimeout"] });
  FauxWorker.crees = [];
  globalThis.Worker = FauxWorker;
  const messages = [];
  const scenes = [];
  const importer = creerImporteur({
    repere: "l93",
    message: (texte, alerte) => messages.push({ texte, alerte: !!alerte }),
    ouvrirScene: (scene, nom) => scenes.push({ scene, nom }),
    maitres: { hidden: true },
    listeMaitres: { replaceChildren() {} },
  });
  const fichier = { name: "a.zip", arrayBuffer: async () => new ArrayBuffer(8) };
  return { importer, fichier, messages, scenes, dernier: () => messages.at(-1) };
}

function scnDeTest() {
  const b = readFileSync(new URL("./deux_primitives.scn", import.meta.url));
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
}

test("piege_termine_le_worker_et_le_suivant_est_neuf", async () => {
  const { importer, fichier, dernier } = banc();
  await importer(fichier);
  const w = FauxWorker.crees[0];
  w.onmessage({ data: { erreur: { code: "piege", message: "unreachable" } } });
  assert.ok(w.termine);
  assert.deepEqual(dernier(), { texte: PIEGE, alerte: true });
  await importer(fichier);
  assert.equal(FauxWorker.crees.length, 2);
  assert.ok(!FauxWorker.crees[1].termine);
});

test("erreur_structuree_termine_aussi_le_worker", async () => {
  const { importer, fichier, dernier } = banc();
  await importer(fichier);
  const w = FauxWorker.crees[0];
  w.onmessage({ data: { erreur: { code: "aucun_dwg" } } });
  assert.ok(w.termine);
  assert.deepEqual(dernier(), { texte: "a.zip : aucun dessin DWG dans le ZIP", alerte: true });
  await importer(fichier);
  assert.equal(FauxWorker.crees.length, 2);
});

test("onerror_et_onmessageerror_terminent_le_worker", async () => {
  const { importer, fichier, dernier } = banc();
  await importer(fichier);
  const w1 = FauxWorker.crees[0];
  w1.onerror({ message: "boum", preventDefault() {} });
  assert.ok(w1.termine);
  assert.deepEqual(dernier(), { texte: "Module d'import indisponible (boum)", alerte: true });
  await importer(fichier);
  const w2 = FauxWorker.crees[1];
  assert.ok(w2, "un worker neuf après onerror");
  w2.onmessageerror({});
  assert.ok(w2.termine);
  assert.deepEqual(dernier(), { texte: "a.zip : réponse du module d'import illisible", alerte: true });
  await importer(fichier);
  assert.equal(FauxWorker.crees.length, 3);
});

test("delai_de_120_s_termine_le_worker", async () => {
  const { importer, fichier, messages, dernier } = banc();
  await importer(fichier);
  const w = FauxWorker.crees[0];
  mock.timers.tick(DELAI_IMPORT_MS - 1);
  assert.ok(!w.termine);
  const avant = messages.length;
  mock.timers.tick(1);
  assert.ok(w.termine);
  assert.equal(messages.length, avant + 1);
  assert.deepEqual(dernier(), { texte: DELAI, alerte: true });
  // Une réponse tardive du worker abandonné est ignorée.
  w.onmessage({ data: { erreur: { code: "aucun_dwg" } } });
  assert.equal(messages.length, avant + 1);
  await importer(fichier);
  assert.equal(FauxWorker.crees.length, 2);
});

test("succes_garde_le_worker_et_annule_le_delai", async () => {
  const { importer, fichier, messages, scenes } = banc();
  await importer(fichier);
  const w = FauxWorker.crees[0];
  w.onmessage({ data: { scn: scnDeTest() } });
  assert.equal(scenes.length, 1);
  const avant = messages.length;
  mock.timers.tick(DELAI_IMPORT_MS);
  assert.ok(!w.termine);
  assert.equal(messages.length, avant);
  await importer(fichier);
  assert.equal(FauxWorker.crees.length, 1);
});
