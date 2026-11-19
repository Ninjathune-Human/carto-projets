import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { classerReponse, classerCouverture } from "../sources.js";

test("réponses", () => {
  assert.equal(classerReponse({ ok: true, status: 200, json: { layers: [] } }), "OK");
  assert.equal(classerReponse({ ok: true, status: 200, json: { error: { code: 499 } } }), "jeton requis");
  assert.equal(classerReponse({ erreurReseau: true }), "CORS ou réseau");
  assert.equal(classerReponse({ ok: false, status: 404 }), "HTTP 404");
});

test("réponses : jeton 498 et erreur réseau prioritaire", () => {
  assert.equal(classerReponse({ ok: true, status: 200, json: { error: { code: 498 } } }), "jeton requis");
  assert.equal(classerReponse({ ok: true, status: 200, json: { error: { code: 400 } } }), "OK");
  assert.equal(classerReponse({ erreurReseau: true, ok: true, status: 200, json: {} }), "CORS ou réseau");
});

test("couverture", () => {
  const nice = { xmin: 0, ymin: 0, xmax: 10, ymax: 10 }, nca = { xmin: -50, ymin: -50, xmax: 50, ymax: 50 };
  assert.equal(classerCouverture({ xmin: 1, ymin: 1, xmax: 9, ymax: 9 }, nice, nca), "Nice");
  assert.equal(classerCouverture({ xmin: -30, ymin: 1, xmax: 9, ymax: 9 }, nice, nca), "Métropole");
  assert.equal(classerCouverture(null, nice, nca), "inconnue");
  assert.equal(classerCouverture({ xmin: -30, ymin: 1, xmax: 9, ymax: 9 }, nice, null), "Métropole");
  assert.equal(classerCouverture({ xmin: 0, ymin: 0, xmax: 10 + 5e-7, ymax: 10 }, nice, nca), "Nice");
});

test("catalogue.json", async () => {
  const cat = JSON.parse(await readFile(new URL("../../../catalogue.json", import.meta.url), "utf8"));
  assert.ok(Array.isArray(cat));
  const ids = (url) => cat.find((e) => e.url.includes(url)).couches.map((c) => c.id);
  const annexes = ids("PLU_Annexes/PLU_Annexes");
  for (const n of [32, 36, 48, 49, 52, 53, 54, 200]) assert.ok(annexes.includes(n), "annexes " + n);
  const serv = ids("PLU_Servitudes");
  for (const n of [26, 28]) assert.ok(serv.includes(n), "servitudes " + n);
});
