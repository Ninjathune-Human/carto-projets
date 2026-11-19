import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  echelleMini, lireLignes, genererSegments, versMercator, contourMercator,
  anneauxContrainte, anneauxVoile, indexerSegments,
} from "../outils.js";

const FIXTURE = new URL("./deux_polylignes.lignes", import.meta.url);

// Générateur pseudo-aléatoire déterministe (congruentiel linéaire), valeurs dans [0, 1).
function lcg(graine) {
  let s = graine >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

// Aire signée (repère y vers le haut) : positive si l'anneau tourne dans le sens antihoraire.
function aire(anneau) {
  let a = 0;
  for (let i = 0; i < anneau.length - 1; i++) {
    a += anneau[i][0] * anneau[i + 1][1] - anneau[i + 1][0] * anneau[i][1];
  }
  return a / 2;
}

test("échelle minimale pour voir toute la Métropole", () => {
  assert.ok(Math.abs(echelleMini(40000, 60000, 1000, 800) - 297637.795) < 0.01);
});

test("lecture du format .lignes", async () => {
  const p = lireLignes((await readFile(new URL("./deux_polylignes.lignes", import.meta.url))).buffer);
  assert.deepEqual(p.map(a => Array.from(a)), [[0, 0, 1, 1], [2, 2, 3, 3, 4, 4]]);
});

test("en-tête .lignes invalide refusé", async () => {
  const octets = new Uint8Array(await readFile(FIXTURE));
  octets.set([0x58, 0x58, 0x58, 0x58], 0);
  assert.throws(() => lireLignes(octets.buffer), /LIG0/);
});

test("fichier .lignes tronqué ou trop long refusé", async () => {
  const octets = new Uint8Array(await readFile(FIXTURE));
  assert.throws(() => lireLignes(octets.buffer.slice(0, 60)), /tronqué/);
  assert.throws(() => lireLignes(octets.buffer.slice(0, 6)), /tronqué/);
  const long = new Uint8Array(octets.length + 1);
  long.set(octets);
  assert.throws(() => lireLignes(long.buffer), /octets en trop/);
});

test("génération de segments aléatoires dans une emprise", () => {
  const p = genererSegments(1000, -10, 20, 30, 50, lcg(7));
  assert.equal(p.length, 1000);
  for (const a of p) {
    assert.ok(a instanceof Float64Array);
    assert.equal(a.length, 4);
    for (const x of [a[0], a[2]]) assert.ok(x >= -10 && x <= 30);
    for (const y of [a[1], a[3]]) assert.ok(y >= 20 && y <= 50);
  }
  const debut = genererSegments(5, -10, 20, 30, 50, lcg(7));
  assert.deepEqual(debut.map(a => Array.from(a)), p.slice(0, 5).map(a => Array.from(a)));
});

test("projection WGS84 vers Web Mercator", () => {
  assert.deepEqual(versMercator(0, 0), [0, 0]);
  assert.ok(Math.abs(versMercator(180, 0)[0] - 20037508.342789244) < 1e-6);
  assert.ok(Math.abs(versMercator(0, 45)[1] - 5621521.486192066) < 1e-6);
});

test("contour lu depuis Polygon, MultiPolygon, Feature et FeatureCollection", () => {
  const carre = [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]];
  const poly = { type: "Polygon", coordinates: carre };
  const multi = { type: "MultiPolygon", coordinates: [carre, carre] };
  const entite = { type: "Feature", properties: {}, geometry: poly };
  const collection = { type: "FeatureCollection", features: [entite, { type: "Feature", properties: {}, geometry: null }, entite] };
  const [x1, y1] = versMercator(1, 1);
  for (const [geojson, nombre] of [[poly, 1], [multi, 2], [entite, 1], [collection, 2]]) {
    const c = contourMercator(geojson);
    assert.equal(c.polygones.length, nombre);
    assert.deepEqual(c.polygones[0][0][2], [x1, y1]);
    assert.deepEqual(c.etendue, { xmin: 0, ymin: 0, xmax: x1, ymax: y1 });
  }
  assert.throws(() => contourMercator({ type: "Point", coordinates: [0, 0] }), /polygone/);
  const lambert = { type: "Polygon", coordinates: [[[1040000, 6280000], [1041000, 6280000], [1041000, 6281000], [1040000, 6280000]]] };
  assert.throws(() => contourMercator(lambert), /WGS84/);
});

test("anneaux de la contrainte et du voile orientés", () => {
  // Extérieur antihoraire et trou horaire, comme le veut la RFC 7946.
  const exterieur = [[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]];
  const trou = [[4, 4], [4, 6], [6, 6], [6, 4], [4, 4]];
  const polygones = [[exterieur, trou]];

  const contrainte = anneauxContrainte(polygones);
  assert.equal(contrainte.length, 2);
  assert.ok(aire(contrainte[0]) < 0, "extérieur horaire");
  assert.ok(aire(contrainte[1]) > 0, "trou antihoraire");

  const voile = anneauxVoile(polygones, { xmin: -100, ymin: -100, xmax: 100, ymax: 100 });
  assert.equal(voile.length, 3);
  assert.ok(aire(voile[0]) < 0, "cadre horaire");
  assert.deepEqual(Math.abs(aire(voile[0])), 40000);
  assert.ok(aire(voile[1]) > 0, "contour percé : antihoraire");
  assert.ok(aire(voile[2]) < 0, "trou du contour à nouveau voilé : horaire");
  assert.deepEqual(exterieur[1], [10, 0], "entrée non modifiée");
});

test("index en grille : mêmes segments qu'une recherche exhaustive", () => {
  const p = genererSegments(5000, 0, 0, 1000, 1000, lcg(3));
  p.push(Float64Array.of(-500, -500, 1500, 1500));          // segment long, hors grille
  p.push(Float64Array.of(100, 100, 120, 100, 120, 130));     // polyligne de deux segments
  p.push(Float64Array.of(5, 5));                             // point isolé : aucun segment
  const index = indexerSegments(p);
  assert.equal(index.nbSegments, 5000 + 1 + 2);

  const segments = [];
  for (const a of p) for (let i = 0; i + 3 < a.length; i += 2) segments.push([a[i], a[i + 1], a[i + 2], a[i + 3]]);
  const exhaustif = (xmin, ymin, xmax, ymax) => segments
    .map((s, i) => [s, i])
    .filter(([s]) => Math.min(s[0], s[2]) <= xmax && Math.max(s[0], s[2]) >= xmin
      && Math.min(s[1], s[3]) <= ymax && Math.max(s[1], s[3]) >= ymin)
    .map(([, i]) => i);

  const rectangles = [
    [0, 0, 1000, 1000], [-2000, -2000, 3000, 3000], [250, 250, 300, 400],
    [110, 95, 115, 105], [999, 999, 2000, 2000], [-900, -900, -800, -800], [1400, 1400, 1450, 1450],
  ];
  for (const r of rectangles) {
    const trouves = Array.from(index.chercher(...r));
    assert.equal(new Set(trouves).size, trouves.length, "sans doublon");
    assert.deepEqual(trouves.sort((a, b) => a - b), exhaustif(...r), String(r));
  }
  assert.deepEqual(index.etendue, { xmin: -500, ymin: -500, xmax: 1500, ymax: 1500 });
  assert.equal(index.x0[5000], -500);
  assert.equal(indexerSegments([]).chercher(0, 0, 1, 1).length, 0);
});
