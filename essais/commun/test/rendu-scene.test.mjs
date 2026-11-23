import { test } from "node:test";
import assert from "node:assert/strict";
import { preparerTampons, origineScene, empriseACadrer } from "../rendu-scene.js";

const trait = (points, options = {}) => ({
  genre: "trait",
  couleur: options.couleur ?? [40, 50, 60, 255],
  calque: options.calque ?? 0,
  epaisseurMm: options.epaisseurMm ?? 0,
  points: Float64Array.from(points),
});
const surface = (points, options = {}) => ({
  genre: "surface",
  couleur: options.couleur ?? [10, 20, 30, 255],
  calque: options.calque ?? 0,
  epaisseurMm: 0,
  points: Float64Array.from(points),
});
const scene = (...primitives) => ({
  calques: [{ nom: "A", couleur: [255, 0, 0] }, { nom: "B", couleur: [0, 0, 255] }],
  primitives,
  rapport: {},
});

test("surface_et_trait", () => {
  const s = scene(
    surface([1001, 2001, 1003, 2001, 1001, 2004], { calque: 1, couleur: [10, 20, 30, 255] }),
    trait([1000, 2000, 1010, 2000], { calque: 0, couleur: [40, 50, 60, 128], epaisseurMm: 0.5 }),
  );
  const t = preparerTampons(s, [1000, 2000]);

  assert.equal(t.surfaces.nombre, 3);
  assert.ok(t.surfaces.positions instanceof Float32Array);
  assert.deepEqual([...t.surfaces.positions], [1, 1, 3, 1, 1, 4]);
  assert.deepEqual([...t.surfaces.couleurs], [10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255]);
  assert.deepEqual([...t.surfaces.calques], [1, 1, 1]);

  // Un segment : un quadrilatère de 4 sommets, deux par extrémité, de part et d'autre du trait.
  assert.equal(t.traits.nombre, 4);
  assert.ok(t.traits.positions instanceof Float32Array);
  assert.deepEqual([...t.traits.positions], [0, 0, 0, 0, 10, 0, 10, 0]);
  assert.deepEqual([...t.traits.autres], [10, 0, 10, 0, 0, 0, 0, 0]);
  assert.deepEqual([...t.traits.cotes], [1, -1, -1, 1]);
  for (const e of t.traits.epaisseurs) assert.ok(Math.abs(e - 0.5) < 1e-6);
  assert.deepEqual([...t.traits.couleurs], [40, 50, 60, 128, 40, 50, 60, 128, 40, 50, 60, 128, 40, 50, 60, 128]);
  assert.deepEqual([...t.traits.calques], [0, 0, 0, 0]);
  assert.deepEqual([...t.traits.indices], [0, 1, 2, 2, 1, 3]);

  // Profondeur : le rang de la primitive, le trait dessiné après la surface passe au-dessus.
  assert.ok(Math.min(...t.traits.rangs) > Math.max(...t.surfaces.rangs));
  assert.equal(t.rangs, 2);
  assert.deepEqual(t.origine, [1000, 2000]);
  assert.deepEqual(t.emprise, [1000, 2000, 1010, 2004]);
  assert.deepEqual(t.ordre, [{ surfaces: [0, 3], traits: [0, 6] }]);
});

test("polyligne_de_trois_points", () => {
  const t = preparerTampons(scene(trait([0, 0, 10, 0, 10, 10])), [0, 0]);
  assert.equal(t.traits.nombre, 8);
  assert.deepEqual([...t.traits.indices], [0, 1, 2, 2, 1, 3, 4, 5, 6, 6, 5, 7]);
  assert.deepEqual([...t.traits.positions.subarray(8)], [10, 0, 10, 0, 10, 10, 10, 10]);
});

test("segments_nuls_et_point_isole", () => {
  // Un point répété ne produit pas de segment ; un trait réduit à un point devient un point.
  const t = preparerTampons(scene(trait([0, 0, 5, 0, 5, 0, 5, 5]), trait([7, 7, 7, 7])), [0, 0]);
  assert.equal(t.traits.nombre, 12);
  assert.deepEqual([...t.traits.positions.subarray(16)], [7, 7, 7, 7, 7, 7, 7, 7]);
  // Le point est orienté par `autres` (vers +x depuis a, vers -x depuis b) pour former un carré.
  assert.deepEqual([...t.traits.autres.subarray(16)], [8, 7, 8, 7, 6, 7, 6, 7]);
  assert.deepEqual([...t.traits.rangs.subarray(8)], [1, 1, 1, 1]);
  assert.deepEqual(t.emprise, [0, 0, 7, 7]);
});

test("surface_tronquee_ignoree", () => {
  // n non multiple de 3 : le triangle incomplet est écarté, le reste est gardé.
  const t = preparerTampons(scene(surface([0, 0, 1, 0, 0, 1, 5, 5])), [0, 0]);
  assert.equal(t.surfaces.nombre, 3);
});

test("lots_et_transparence", () => {
  // Une surface transparente dessinée après un trait ouvre un nouveau lot, pour que le trait
  // reste visible à travers elle ; une surface opaque n'en ouvre pas.
  const opaque = preparerTampons(scene(trait([0, 0, 1, 1]), surface([0, 0, 1, 0, 0, 1])), [0, 0]);
  assert.deepEqual(opaque.ordre, [{ surfaces: [0, 3], traits: [0, 6] }]);

  const s = scene(
    surface([0, 0, 1, 0, 0, 1]),
    trait([0, 0, 1, 1]),
    surface([0, 0, 1, 0, 0, 1], { couleur: [0, 0, 0, 100] }),
    surface([0, 0, 1, 0, 0, 1], { couleur: [0, 0, 0, 100] }),
    trait([0, 0, 1, 1]),
  );
  const t = preparerTampons(s, [0, 0]);
  assert.deepEqual(t.ordre, [
    { surfaces: [0, 3], traits: [0, 6] },
    { surfaces: [3, 6], traits: [6, 6] },
  ]);
});

test("scene_vide", () => {
  const t = preparerTampons(scene(), [0, 0]);
  assert.equal(t.surfaces.nombre, 0);
  assert.equal(t.traits.nombre, 0);
  assert.equal(t.emprise, null);
  assert.deepEqual(t.ordre, []);
});

test("origine_dans_le_plan_malgre_un_objet_egare", () => {
  // Quatre primitives près de (1 040 000, 6 300 000) et une à l'origine : l'origine des tampons
  // reste dans le plan, pour garder la précision des flottants 32 bits.
  const s = scene(
    trait([1040000, 6300000, 1040010, 6300000]),
    trait([1040002, 6300004, 1040010, 6300009]),
    surface([1040001, 6300001, 1040003, 6300001, 1040001, 6300003]),
    trait([1040005, 6300002, 1040006, 6300002]),
    trait([0, 0, 1, 1]),
  );
  const [x, y] = origineScene(s);
  assert.ok(x >= 1040000 && x <= 1040010);
  assert.ok(y >= 6300000 && y <= 6300010);
});

test("emprise_a_cadrer", () => {
  // Objet égaré à l'origine : il sort du cadrage, le plan reste entier.
  const complete = [0, 0, 1040010, 6300010];
  const essentielle = [1040000, 6300000, 1040010, 6300010];
  assert.deepEqual(empriseACadrer({ emprise: essentielle }, complete), [1039995, 6299995, 1040010, 6300010]);
  // Sans objet égaré : les percentiles rognent le plan, l'emprise complète est gardée.
  assert.deepEqual(empriseACadrer({ emprise: [10, 10, 90, 90] }, [0, 0, 100, 100]), [0, 0, 100, 100]);
  // Absente, nulle ou invalide : repli sur l'emprise complète.
  assert.deepEqual(empriseACadrer({}, complete), complete);
  assert.deepEqual(empriseACadrer({ emprise: null }, complete), complete);
  assert.deepEqual(empriseACadrer(undefined, complete), complete);
  assert.deepEqual(empriseACadrer({ emprise: [1, 2, 3] }, complete), complete);
  assert.deepEqual(empriseACadrer({ emprise: [1, 2, NaN, 4] }, complete), complete);
  assert.deepEqual(empriseACadrer({ emprise: [5, 2, 3, 4] }, complete), complete);
  assert.equal(empriseACadrer({}, null), null);
  assert.deepEqual(empriseACadrer({ emprise: essentielle }, null), essentielle);
});
