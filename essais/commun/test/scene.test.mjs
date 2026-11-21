import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { lireScene } from "../scene.js";

const fixture = new URL("./deux_primitives.scn", import.meta.url);

test("lit_deux_primitives", () => {
  const b = readFileSync(fixture);
  const tampon = b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
  const s = lireScene(tampon);
  assert.equal(s.calques.length, 1);
  assert.equal(s.calques[0].nom, "A");
  assert.deepEqual(s.calques[0].couleur, [255, 0, 0]);
  assert.equal(s.primitives.length, 2);
  const t = s.primitives[0];
  assert.equal(t.genre, "trait");
  assert.equal(t.points.length, 4);
  assert.ok(Math.abs(t.epaisseurMm - 0.25) < 1e-6);
  assert.deepEqual(t.couleur, [255, 0, 0, 255]);
  const f = s.primitives[1];
  assert.equal(f.genre, "surface");
  assert.equal(f.points.length, 6);
  assert.ok(s.rapport.maitre);
});

test("rejette_signature", () => {
  const t = new Uint8Array(32);
  t.set([0x58, 0x58, 0x58, 0x58]);
  assert.throws(() => lireScene(t.buffer), /SCN1/);
});
