// Rendu WebGL2 d'une scène SCN1 (traits et surfaces), commun à la visionneuse et à la carte.
//
// preparerTampons est pure : elle range la scène en tableaux typés prêts pour le GPU, en
// flottants 32 bits relatifs à une origine choisie par l'appelant (le décalage entre l'origine
// et le centre de la vue est calculé en double précision côté JS, comme dans couche-lignes.js).
//
// Ordre d'affichage : chaque sommet porte le rang de sa primitive dans la scène ; le shader en
// tire la profondeur (rang plus grand = plus près), le test de profondeur LESS garde ainsi
// l'ordre d'AutoCAD entre surfaces et traits dessinés en deux appels. Une surface transparente
// dessinée après un trait ouvrirait un trou dans ce trait : elle ouvre un nouveau lot, et les
// lots sont dessinés l'un après l'autre (voir `ordre`). Sans transparence, un seul lot.
//
// Calques : chaque sommet porte l'indice de son calque ; une texture d'un octet par calque dit
// s'il est visible. Masquer un calque = changer un octet, sans reconstruire les tampons ; les
// sommets d'un calque masqué sont envoyés hors du cadre par le shader.

const PX_PAR_MM = 96 / 25.4;

// ---------------------------------------------------------------- préparation (pure)

function segmentValide(p, k) {
  const x0 = p[k], y0 = p[k + 1], x1 = p[k + 2], y1 = p[k + 3];
  return Number.isFinite(x0) && Number.isFinite(y0) && Number.isFinite(x1) && Number.isFinite(y1)
    && (x0 !== x1 || y0 !== y1);
}

function triangleValide(p, k) {
  for (let i = k; i < k + 6; i++) if (!Number.isFinite(p[i])) return false;
  return true;
}

// Indice (dans le tableau à plat) du premier point fini d'un trait, ou -1.
function premierPointFini(p) {
  for (let k = 0; k + 1 < p.length; k += 2) if (Number.isFinite(p[k]) && Number.isFinite(p[k + 1])) return k;
  return -1;
}

/**
 * Range une scène en tampons. `origine` : [x0, y0] dans le repère de la scène.
 * Rend :
 * - surfaces : { positions (x, y), couleurs (RVBA, octets), rangs, calques, nombre de sommets } ;
 *   des triangles à dessiner en TRIANGLES ;
 * - traits : { positions, autres, cotes, epaisseurs (mm), couleurs, rangs, calques, indices,
 *   nombre de sommets } ; chaque segment donne un quadrilatère de 4 sommets (cette extrémité,
 *   l'autre extrémité, côté ±1) et 6 indices ; le shader l'élargit à
 *   max(1, mm × 96 / 25.4) px ; un trait réduit à un point donne un carré de cette largeur ;
 * - rangs : nombre de primitives (borne des rangs, pour la profondeur) ;
 * - ordre : lots à dessiner dans l'ordre, { surfaces: [premier sommet, nombre de sommets],
 *   traits: [premier indice, nombre d'indices] } ;
 * - origine, emprise [xmin, ymin, xmax, ymax] dans le repère de la scène (null si vide).
 */
export function preparerTampons(scene, origine) {
  const prims = scene.primitives;
  const [ox, oy] = origine;

  // Premier passage : comptes et emprise.
  let nbSommetsS = 0;
  let nbSegments = 0;
  let xmin = Infinity, ymin = Infinity, xmax = -Infinity, ymax = -Infinity;
  const etendre = (x, y) => {
    if (x < xmin) xmin = x;
    if (x > xmax) xmax = x;
    if (y < ymin) ymin = y;
    if (y > ymax) ymax = y;
  };
  for (const prim of prims) {
    const p = prim.points;
    if (prim.genre === "surface") {
      for (let k = 0; k + 6 <= p.length; k += 6) {
        if (!triangleValide(p, k)) continue;
        nbSommetsS += 3;
        etendre(p[k], p[k + 1]); etendre(p[k + 2], p[k + 3]); etendre(p[k + 4], p[k + 5]);
      }
    } else {
      let n = 0;
      for (let k = 0; k + 4 <= p.length; k += 2) {
        if (!segmentValide(p, k)) continue;
        n++;
        etendre(p[k], p[k + 1]); etendre(p[k + 2], p[k + 3]);
      }
      if (n === 0) {
        const k = premierPointFini(p);
        if (k >= 0) { n = 1; etendre(p[k], p[k + 1]); }
      }
      nbSegments += n;
    }
  }

  const TypeCalque = scene.calques.length <= 0xffff ? Uint16Array : Uint32Array;
  const nbSommetsT = nbSegments * 4;
  const surfaces = {
    positions: new Float32Array(nbSommetsS * 2),
    couleurs: new Uint8Array(nbSommetsS * 4),
    rangs: new Float32Array(nbSommetsS),
    calques: new TypeCalque(nbSommetsS),
    nombre: nbSommetsS,
  };
  const traits = {
    positions: new Float32Array(nbSommetsT * 2),
    autres: new Float32Array(nbSommetsT * 2),
    cotes: new Int8Array(nbSommetsT),
    epaisseurs: new Float32Array(nbSommetsT),
    couleurs: new Uint8Array(nbSommetsT * 4),
    rangs: new Float32Array(nbSommetsT),
    calques: new TypeCalque(nbSommetsT),
    indices: new Uint32Array(nbSegments * 6),
    nombre: nbSommetsT,
  };

  // Second passage : remplissage, lots.
  const ordre = [];
  let lot = { surfaces: [0, 0], traits: [0, 0] };
  let traitDansLot = false;
  let s = 0; // sommets de surface écrits
  let t = 0; // sommets de trait écrits

  const sommetTrait = (prim, rang, x, y, autreX, autreY, cote) => {
    const c = prim.couleur;
    traits.positions[t * 2] = x - ox;
    traits.positions[t * 2 + 1] = y - oy;
    traits.autres[t * 2] = autreX - ox;
    traits.autres[t * 2 + 1] = autreY - oy;
    traits.cotes[t] = cote;
    traits.epaisseurs[t] = prim.epaisseurMm;
    traits.couleurs[t * 4] = c[0];
    traits.couleurs[t * 4 + 1] = c[1];
    traits.couleurs[t * 4 + 2] = c[2];
    traits.couleurs[t * 4 + 3] = c[3];
    traits.rangs[t] = rang;
    traits.calques[t] = prim.calque;
    t++;
  };
  // Sommets 0 et 1 à l'extrémité a, 2 et 3 à l'extrémité b ; vu de b, le côté s'inverse.
  // `autres` sert au shader à orienter le quadrilatère : l'autre extrémité pour un segment ;
  // pour un point isolé (a = b), un point à 1 unité vers +x depuis a et vers -x depuis b.
  const quadrilatere = (prim, rang, ax, ay, autreAx, autreAy, bx, by, autreBx, autreBy) => {
    const premier = t;
    sommetTrait(prim, rang, ax, ay, autreAx, autreAy, 1);
    sommetTrait(prim, rang, ax, ay, autreAx, autreAy, -1);
    sommetTrait(prim, rang, bx, by, autreBx, autreBy, -1);
    sommetTrait(prim, rang, bx, by, autreBx, autreBy, 1);
    const i = (premier / 4) * 6;
    const ind = traits.indices;
    ind[i] = premier;
    ind[i + 1] = premier + 1;
    ind[i + 2] = premier + 2;
    ind[i + 3] = premier + 2;
    ind[i + 4] = premier + 1;
    ind[i + 5] = premier + 3;
  };

  for (let rang = 0; rang < prims.length; rang++) {
    const prim = prims[rang];
    const p = prim.points;
    if (prim.genre === "surface") {
      if (prim.couleur[3] < 255 && traitDansLot) {
        ordre.push(lot);
        lot = { surfaces: [s, 0], traits: [(t / 4) * 6, 0] };
        traitDansLot = false;
      }
      for (let k = 0; k + 6 <= p.length; k += 6) {
        if (!triangleValide(p, k)) continue;
        for (let j = 0; j < 3; j++) {
          surfaces.positions[s * 2] = p[k + j * 2] - ox;
          surfaces.positions[s * 2 + 1] = p[k + j * 2 + 1] - oy;
          surfaces.couleurs[s * 4] = prim.couleur[0];
          surfaces.couleurs[s * 4 + 1] = prim.couleur[1];
          surfaces.couleurs[s * 4 + 2] = prim.couleur[2];
          surfaces.couleurs[s * 4 + 3] = prim.couleur[3];
          surfaces.rangs[s] = rang;
          surfaces.calques[s] = prim.calque;
          s++;
        }
      }
    } else {
      const avant = t;
      for (let k = 0; k + 4 <= p.length; k += 2) {
        if (!segmentValide(p, k)) continue;
        const ax = p[k], ay = p[k + 1], bx = p[k + 2], by = p[k + 3];
        quadrilatere(prim, rang, ax, ay, bx, by, bx, by, ax, ay);
      }
      if (t === avant) {
        const k = premierPointFini(p);
        if (k >= 0) {
          const x = p[k], y = p[k + 1];
          quadrilatere(prim, rang, x, y, x + 1, y, x, y, x - 1, y);
        }
      }
      if (t > avant) traitDansLot = true;
    }
    lot.surfaces[1] = s - lot.surfaces[0];
    lot.traits[1] = (t / 4) * 6 - lot.traits[0];
  }
  if (lot.surfaces[1] > 0 || lot.traits[1] > 0) ordre.push(lot);

  return {
    surfaces,
    traits,
    rangs: prims.length,
    ordre,
    origine: [ox, oy],
    emprise: xmin <= xmax ? [xmin, ymin, xmax, ymax] : null,
  };
}

/**
 * Origine conseillée pour preparerTampons : la médiane des premiers points des primitives.
 * Elle reste dans le plan même si un objet égaré est loin (cartouche à l'origine…), ce qui
 * garde au plan la précision des flottants 32 bits.
 */
export function origineScene(scene) {
  const xs = [];
  const ys = [];
  for (const prim of scene.primitives) {
    const k = premierPointFini(prim.points);
    if (k < 0) continue;
    xs.push(prim.points[k]);
    ys.push(prim.points[k + 1]);
  }
  if (xs.length === 0) return [0, 0];
  const mediane = v => Float64Array.from(v).sort()[v.length >> 1];
  return [mediane(xs), mediane(ys)];
}

/**
 * Emprise à cadrer en vue d'ensemble, à partir de `complete` (emprise de toutes les primitives)
 * et de l'emprise essentielle du rapport (percentiles 2 à 98 des points dans la Métropole, voir
 * carto-georef). L'emprise essentielle seule rognerait le plan (jusqu'à 2 % des points de chaque
 * côté) ; l'emprise complète réduirait le plan à un point s'il y a un objet égaré loin (cartouche
 * à l'origine). D'où : l'emprise complète bornée à l'emprise essentielle élargie, de chaque côté,
 * de la moitié de son plus grand côté. Sans emprise essentielle valide (4 nombres finis
 * ordonnés) : l'emprise complète.
 */
export function empriseACadrer(rapport, complete) {
  const e = rapport?.emprise;
  const valide = Array.isArray(e) && e.length === 4 && e.every(Number.isFinite) && e[0] <= e[2] && e[1] <= e[3];
  if (!valide) return complete;
  if (!complete) return e;
  const d = Math.max(e[2] - e[0], e[3] - e[1]) / 2;
  const b = [
    Math.max(complete[0], e[0] - d), Math.max(complete[1], e[1] - d),
    Math.min(complete[2], e[2] + d), Math.min(complete[3], e[3] + d),
  ];
  return b[0] <= b[2] && b[1] <= b[3] ? b : e;
}

// ---------------------------------------------------------------- WebGL2

const ENTETE = `#version 300 es
precision highp float;
precision highp int;
layout(location = 4) in vec4 a_couleur;
layout(location = 5) in float a_rang;
layout(location = 6) in uint a_calque;
uniform vec2 u_decalage;      // origine des tampons - centre de la vue (m)
uniform float u_resolution;   // m par px CSS
uniform mat2 u_rotation;
uniform vec2 u_versNdc;       // 2 / taille en px CSS
uniform float u_rangs;
uniform highp sampler2D u_masque;
uniform int u_largeurMasque;
out vec4 v_couleur;

bool calqueVisible() {
  int c = int(a_calque);
  return texelFetch(u_masque, ivec2(c % u_largeurMasque, c / u_largeurMasque), 0).r > 0.5;
}
// Rang plus grand = plus près : avec LESS, la primitive dessinée après passe au-dessus.
float profondeur() {
  return 1.0 - 2.0 * (a_rang + 1.0) / (u_rangs + 1.0);
}
vec2 versPixels(vec2 p) {
  return u_rotation * ((p + u_decalage) / u_resolution);
}
const vec4 HORS_CADRE = vec4(2.0, 2.0, 2.0, 1.0);
`;

const SOURCE_SURFACES = ENTETE + `
layout(location = 0) in vec2 a_position;
void main() {
  if (!calqueVisible()) { gl_Position = HORS_CADRE; v_couleur = vec4(0.0); return; }
  gl_Position = vec4(versPixels(a_position) * u_versNdc, profondeur(), 1.0);
  v_couleur = a_couleur;
}`;

// Quadrilatère en espace écran : demi-largeur de part et d'autre du segment, et prolongement
// d'une demi-largeur au-delà de chaque extrémité pour fermer les angles entre segments.
const SOURCE_TRAITS = ENTETE + `
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_autre;
layout(location = 2) in float a_cote;
layout(location = 3) in float a_epaisseur;
const float PX_PAR_MM = ${PX_PAR_MM.toFixed(10)};
void main() {
  if (!calqueVisible()) { gl_Position = HORS_CADRE; v_couleur = vec4(0.0); return; }
  vec2 p = versPixels(a_position);
  vec2 d = versPixels(a_autre) - p;
  float l = length(d);
  vec2 dir = l > 0.0 ? d / l : vec2(1.0, 0.0);
  float demi = 0.5 * max(1.0, a_epaisseur * PX_PAR_MM);
  vec2 decal = vec2(-dir.y, dir.x) * (a_cote * demi) - dir * demi;
  gl_Position = vec4((p + decal) * u_versNdc, profondeur(), 1.0);
  v_couleur = a_couleur;
}`;

const SOURCE_FRAGMENTS = `#version 300 es
precision mediump float;
in vec4 v_couleur;
out vec4 couleur;
void main() {
  couleur = vec4(v_couleur.rgb * v_couleur.a, v_couleur.a); // prémultipliée
}`;

const UNIFORMES = ["u_decalage", "u_resolution", "u_rotation", "u_versNdc", "u_rangs", "u_masque", "u_largeurMasque"];

function compiler(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const journal = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error("shader refusé : " + journal);
  }
  return shader;
}

function creerProgramme(gl, sourceSommets) {
  const vs = compiler(gl, gl.VERTEX_SHADER, sourceSommets);
  const fs = compiler(gl, gl.FRAGMENT_SHADER, SOURCE_FRAGMENTS);
  const programme = gl.createProgram();
  gl.attachShader(programme, vs);
  gl.attachShader(programme, fs);
  gl.linkProgram(programme);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!gl.getProgramParameter(programme, gl.LINK_STATUS)) {
    const journal = gl.getProgramInfoLog(programme);
    gl.deleteProgram(programme);
    throw new Error("programme refusé : " + journal);
  }
  const u = {};
  for (const nom of UNIFORMES) u[nom] = gl.getUniformLocation(programme, nom);
  return { programme, u };
}

/**
 * Rendu d'une scène préparée dans un contexte WebGL2 (avec tampon de profondeur).
 * - charger(tampons) : envoie au GPU le résultat de preparerTampons (remplace le précédent) ;
 * - calques(visibles) : visibles[i] === false masque le calque i, toute autre valeur l'affiche ;
 * - dessiner({ centre, resolution, rotation, taille }) : centre [x, y] dans le repère de la
 *   scène, resolution en unités par px CSS, rotation en degrés, taille [largeur, hauteur] en
 *   px CSS ; occupe tout le tampon de dessin, efface la profondeur mais pas la couleur ;
 * - liberer() : rend les ressources GPU.
 */
export function creerRendu(gl) {
  const surfaces = creerProgramme(gl, SOURCE_SURFACES);
  const traits = creerProgramme(gl, SOURCE_TRAITS);
  const masque = gl.createTexture();
  const largeurMax = Math.min(2048, gl.getParameter(gl.MAX_TEXTURE_SIZE));
  // Avec un tampon de profondeur trop court pour distinguer tous les rangs, LESS ferait
  // disparaître des primitives ; LEQUAL les garde (ordre du peintre dans chaque appel).
  const bitsProfondeur = gl.getParameter(gl.DEPTH_BITS) || 16;
  let charge = null;

  function attribut(tampons, emplacement, donnees, taille, type, normalise = false) {
    const b = gl.createBuffer();
    tampons.push(b);
    gl.bindBuffer(gl.ARRAY_BUFFER, b);
    gl.bufferData(gl.ARRAY_BUFFER, donnees, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(emplacement);
    if (type === gl.UNSIGNED_SHORT || type === gl.UNSIGNED_INT) gl.vertexAttribIPointer(emplacement, taille, type, 0, 0);
    else gl.vertexAttribPointer(emplacement, taille, type, normalise, 0, 0);
  }

  function attributsCommuns(tampons, d) {
    attribut(tampons, 4, d.couleurs, 4, gl.UNSIGNED_BYTE, true);
    attribut(tampons, 5, d.rangs, 1, gl.FLOAT);
    attribut(tampons, 6, d.calques, 1, d.calques instanceof Uint16Array ? gl.UNSIGNED_SHORT : gl.UNSIGNED_INT);
  }

  function liberer() {
    if (!charge) return;
    for (const b of charge.tampons) gl.deleteBuffer(b);
    if (charge.vaoS) gl.deleteVertexArray(charge.vaoS);
    if (charge.vaoT) gl.deleteVertexArray(charge.vaoT);
    charge = null;
  }

  function calques(visibles) {
    if (!charge) return;
    const { largeur, hauteur, nbCalques } = charge.masque;
    const octets = new Uint8Array(largeur * hauteur);
    for (let i = 0; i < nbCalques; i++) octets[i] = visibles?.[i] === false ? 0 : 255;
    gl.bindTexture(gl.TEXTURE_2D, masque);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, largeur, hauteur, 0, gl.RED, gl.UNSIGNED_BYTE, octets);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  function charger(t) {
    liberer();
    const tampons = [];
    let vaoS = null;
    let vaoT = null;
    if (t.surfaces.nombre > 0) {
      vaoS = gl.createVertexArray();
      gl.bindVertexArray(vaoS);
      attribut(tampons, 0, t.surfaces.positions, 2, gl.FLOAT);
      attributsCommuns(tampons, t.surfaces);
    }
    if (t.traits.nombre > 0) {
      vaoT = gl.createVertexArray();
      gl.bindVertexArray(vaoT);
      attribut(tampons, 0, t.traits.positions, 2, gl.FLOAT);
      attribut(tampons, 1, t.traits.autres, 2, gl.FLOAT);
      attribut(tampons, 2, t.traits.cotes, 1, gl.BYTE);
      attribut(tampons, 3, t.traits.epaisseurs, 1, gl.FLOAT);
      attributsCommuns(tampons, t.traits);
      const indices = gl.createBuffer();
      tampons.push(indices);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indices);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, t.traits.indices, gl.STATIC_DRAW);
    }
    gl.bindVertexArray(null);
    gl.bindBuffer(gl.ARRAY_BUFFER, null);

    // Plus grand indice de calque utilisé (la scène peut en déclarer moins que les sommets n'en citent).
    let nbCalques = 1;
    for (const c of t.surfaces.calques) if (c >= nbCalques) nbCalques = c + 1;
    for (let i = 0; i < t.traits.calques.length; i += 4) if (t.traits.calques[i] >= nbCalques) nbCalques = t.traits.calques[i] + 1;
    const largeur = Math.min(nbCalques, largeurMax);
    const hauteur = Math.ceil(nbCalques / largeur);
    charge = {
      tampons, vaoS, vaoT,
      ordre: t.ordre, origine: t.origine, rangs: t.rangs,
      masque: { largeur, hauteur, nbCalques },
    };
    calques(null);
  }

  function uniformes({ programme, u }, etat, decalage, matrice) {
    gl.useProgram(programme);
    gl.uniform2f(u.u_decalage, decalage[0], decalage[1]);
    gl.uniform1f(u.u_resolution, etat.resolution);
    gl.uniformMatrix2fv(u.u_rotation, false, matrice);
    gl.uniform2f(u.u_versNdc, 2 / etat.taille[0], 2 / etat.taille[1]);
    gl.uniform1f(u.u_rangs, charge.rangs);
    gl.uniform1i(u.u_masque, 0);
    gl.uniform1i(u.u_largeurMasque, charge.masque.largeur);
  }

  function dessiner(etat) {
    if (!charge) return;
    const rot = ((etat.rotation || 0) * Math.PI) / 180;
    const c = Math.cos(rot);
    const s = Math.sin(rot);
    const matrice = [c, -s, s, c];
    // Différence en double précision, puis passage en flottants 32 bits.
    const decalage = [charge.origine[0] - etat.centre[0], charge.origine[1] - etat.centre[1]];

    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(charge.rangs + 2 < 2 ** bitsProfondeur ? gl.LESS : gl.LEQUAL);
    gl.depthMask(true);
    gl.clearDepth(1);
    gl.clear(gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, masque);

    if (charge.vaoS) uniformes(surfaces, etat, decalage, matrice);
    if (charge.vaoT) uniformes(traits, etat, decalage, matrice);
    for (const lot of charge.ordre) {
      if (lot.surfaces[1] > 0) {
        gl.useProgram(surfaces.programme);
        gl.bindVertexArray(charge.vaoS);
        gl.drawArrays(gl.TRIANGLES, lot.surfaces[0], lot.surfaces[1]);
      }
      if (lot.traits[1] > 0) {
        gl.useProgram(traits.programme);
        gl.bindVertexArray(charge.vaoT);
        gl.drawElements(gl.TRIANGLES, lot.traits[1], gl.UNSIGNED_INT, lot.traits[0] * 4);
      }
    }

    gl.bindVertexArray(null);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.disable(gl.BLEND);
    gl.disable(gl.DEPTH_TEST);
    gl.useProgram(null);
  }

  return {
    charger,
    calques,
    dessiner,
    liberer() {
      liberer();
      gl.deleteTexture(masque);
      gl.deleteProgram(surfaces.programme);
      gl.deleteProgram(traits.programme);
    },
  };
}
