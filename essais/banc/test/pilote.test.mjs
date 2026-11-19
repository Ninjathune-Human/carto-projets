import test from "node:test";
import assert from "node:assert/strict";
import { creerPilote } from "../pilote.js";

// Faux worker : `comportement(message, worker)` répond via worker.onmessage ou provoque une panne.
function fabrique(comportement) {
  const etat = { crees: [], octetsRecus: 0 };
  const creerWorker = () => {
    const w = {
      id: etat.crees.length,
      termine: false,
      postMessage(message) {
        if (message.type === "ouvrir") etat.octetsRecus += message.octets.byteLength;
        setImmediate(() => comportement(message, w, etat));
      },
      terminate() { w.termine = true; },
      repondre(donnees) { if (!w.termine) w.onmessage?.({ data: donnees }); },
    };
    etat.crees.push(w);
    return w;
  };
  return { creerWorker, etat };
}

const octets = () => Promise.resolve(new ArrayBuffer(8));

function normal(message, w) {
  if (message.type === "ouvrir") w.repondre({ chemins: ["a.dwg", "b.dwg", "c.dwg"] });
  else w.repondre({ element: { chemin: message.chemin, duree_ms: 1, mesure: { total: 1 } } });
}

test("liste puis mesure chaque DWG avec un seul worker et un seul envoi du ZIP", async () => {
  const { creerWorker, etat } = fabrique(normal);
  const p = creerPilote({ creerWorker, delaiMs: 1000 });
  const chemins = await p.lister("z", octets);
  assert.deepEqual(chemins, ["a.dwg", "b.dwg", "c.dwg"]);
  for (const c of chemins) assert.equal((await p.mesurer("z", octets, c)).chemin, c);
  assert.equal(etat.crees.length, 1);
  assert.equal(etat.octetsRecus, 8);
});

test("une erreur du worker ne perd que ce DWG : worker recréé, ZIP renvoyé, suite mesurée", async () => {
  const { creerWorker, etat } = fabrique((m, w) => (m.type === "mesurer" && m.chemin === "b.dwg" ? w.repondre({ erreur: "unreachable" }) : normal(m, w)));
  const p = creerPilote({ creerWorker, delaiMs: 1000 });
  const chemins = await p.lister("z", octets);
  assert.equal((await p.mesurer("z", octets, "a.dwg")).chemin, "a.dwg");
  await assert.rejects(p.mesurer("z", octets, "b.dwg"), /unreachable/);
  assert.equal(etat.crees[0].termine, true);
  assert.equal((await p.mesurer("z", octets, "c.dwg")).chemin, "c.dwg");
  assert.equal(etat.crees.length, 2);
  assert.equal(etat.octetsRecus, 16); // le ZIP est renvoyé au nouveau worker
  assert.equal(chemins.length, 3);
});

test("délai dépassé : erreur 'délai dépassé', worker recréé, DWG suivant mesuré", async () => {
  const { creerWorker, etat } = fabrique((m, w) => { if (!(m.type === "mesurer" && m.chemin === "a.dwg")) normal(m, w); });
  const p = creerPilote({ creerWorker, delaiMs: 30 });
  await p.lister("z", octets);
  await assert.rejects(p.mesurer("z", octets, "a.dwg"), /délai dépassé/);
  assert.equal(etat.crees[0].termine, true);
  assert.equal((await p.mesurer("z", octets, "b.dwg")).chemin, "b.dwg");
  assert.equal(etat.crees.length, 2);
});

test("onerror : le lecteur s'est arrêté, worker recréé", async () => {
  const { creerWorker, etat } = fabrique((m, w) => (m.type === "mesurer" && m.chemin === "a.dwg" ? w.onerror({ message: "RuntimeError" }) : normal(m, w)));
  const p = creerPilote({ creerWorker, delaiMs: 1000 });
  await p.lister("z", octets);
  await assert.rejects(p.mesurer("z", octets, "a.dwg"), /le lecteur s'est arrêté \(RuntimeError\)/);
  assert.equal((await p.mesurer("z", octets, "b.dwg")).chemin, "b.dwg");
  assert.equal(etat.crees.length, 2);
});

test("onmessageerror : réponse illisible, worker recréé", async () => {
  const { creerWorker, etat } = fabrique((m, w) => (m.type === "mesurer" && m.chemin === "a.dwg" ? w.onmessageerror() : normal(m, w)));
  const p = creerPilote({ creerWorker, delaiMs: 1000 });
  await p.lister("z", octets);
  await assert.rejects(p.mesurer("z", octets, "a.dwg"), /réponse illisible/);
  assert.equal((await p.mesurer("z", octets, "b.dwg")).chemin, "b.dwg");
  assert.equal(etat.crees.length, 2);
});

test("ZIP illisible : lister lève l'erreur du worker", async () => {
  const { creerWorker } = fabrique((m, w) => w.repondre({ erreur: "ZIP illisible" }));
  const p = creerPilote({ creerWorker, delaiMs: 1000 });
  await assert.rejects(p.lister("z", octets), /ZIP illisible/);
});

test("une réponse tardive d'un worker abandonné est ignorée", async () => {
  const { creerWorker, etat } = fabrique((m, w, e) => {
    if (m.type === "mesurer" && m.chemin === "a.dwg") setTimeout(() => normal(m, w), 80);
    else normal(m, w);
  });
  const p = creerPilote({ creerWorker, delaiMs: 20 });
  await p.lister("z", octets);
  await assert.rejects(p.mesurer("z", octets, "a.dwg"), /délai dépassé/);
  const r = await p.mesurer("z", octets, "b.dwg");
  assert.equal(r.chemin, "b.dwg");
  await new Promise((ok) => setTimeout(ok, 120));
  assert.equal(etat.crees.length, 2);
});
