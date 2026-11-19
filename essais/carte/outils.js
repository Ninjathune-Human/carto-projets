// Outils purs de l'essai carte : aucune dépendance ArcGIS, testés sous Node.

const RAYON_MERCATOR = 6378137;
const LATITUDE_MAX = 85.0511287798066;

/**
 * Échelle minimale (dénominateur) pour voir une étendue de largeurM × hauteurM mètres
 * dans une vue de largeurPx × hauteurPx pixels CSS (96 ppp), avec 5 % de marge.
 */
export function echelleMini(largeurM, hauteurM, largeurPx, hauteurPx) {
  return Math.max(largeurM / largeurPx, hauteurM / hauteurPx) * 96 / 0.0254 * 1.05;
}

/**
 * Lit le format binaire .lignes : b"LIG0", u32 LE nombre de polylignes, puis pour chacune
 * u32 LE n et n × (f64 LE x, f64 LE y), en EPSG:3857.
 * Renvoie un tableau de Float64Array (x, y entrelacés), vues d'un même tampon.
 */
export function lireLignes(donnees) {
  let vue;
  if (donnees instanceof ArrayBuffer) vue = new DataView(donnees);
  else if (ArrayBuffer.isView(donnees)) vue = new DataView(donnees.buffer, donnees.byteOffset, donnees.byteLength);
  else throw new TypeError("lireLignes attend un ArrayBuffer");

  const taille = vue.byteLength;
  if (taille < 8) throw new Error("fichier .lignes tronqué : en-tête incomplet");
  if (vue.getUint8(0) !== 0x4c || vue.getUint8(1) !== 0x49 || vue.getUint8(2) !== 0x47 || vue.getUint8(3) !== 0x30) {
    throw new Error("fichier .lignes invalide : en-tête LIG0 attendu");
  }
  const nombre = vue.getUint32(4, true);

  // Premier passage : valider la structure et compter les points.
  const longueurs = [];
  let pos = 8;
  let points = 0;
  for (let i = 0; i < nombre; i++) {
    if (pos + 4 > taille) throw new Error(`fichier .lignes tronqué : polyligne ${i + 1} sur ${nombre} absente`);
    const n = vue.getUint32(pos, true);
    pos += 4;
    if (pos + n * 16 > taille) throw new Error(`fichier .lignes tronqué : polyligne ${i + 1} sur ${nombre} incomplète`);
    pos += n * 16;
    longueurs.push(n);
    points += n;
  }
  if (pos !== taille) throw new Error(`fichier .lignes invalide : ${taille - pos} octets en trop`);

  // Second passage : copier les coordonnées dans un seul tampon.
  const tout = new Float64Array(points * 2);
  const polylignes = new Array(longueurs.length);
  pos = 8;
  let k = 0;
  for (let i = 0; i < longueurs.length; i++) {
    const n = longueurs[i];
    pos += 4;
    const debut = k;
    for (let j = 0; j < n * 2; j++, pos += 8) tout[k++] = vue.getFloat64(pos, true);
    polylignes[i] = tout.subarray(debut, k);
  }
  return polylignes;
}

/**
 * n polylignes de deux points tirées dans l'emprise [xmin, xmax] × [ymin, ymax] :
 * origine uniforme, direction uniforme, longueur jusqu'à 0,5 % du plus grand côté.
 */
export function genererSegments(n, xmin, ymin, xmax, ymax, alea = Math.random) {
  if (!Number.isInteger(n) || n < 0) throw new RangeError("nombre de segments invalide");
  const largeur = xmax - xmin;
  const hauteur = ymax - ymin;
  const longueurMax = 0.005 * Math.max(largeur, hauteur);
  const borne = (v, a, b) => (v < a ? a : v > b ? b : v);
  const tout = new Float64Array(n * 4);
  const segments = new Array(n);
  for (let i = 0; i < n; i++) {
    const x0 = xmin + alea() * largeur;
    const y0 = ymin + alea() * hauteur;
    const angle = alea() * 2 * Math.PI;
    const longueur = alea() * longueurMax;
    const o = i * 4;
    tout[o] = x0;
    tout[o + 1] = y0;
    tout[o + 2] = borne(x0 + Math.cos(angle) * longueur, xmin, xmax);
    tout[o + 3] = borne(y0 + Math.sin(angle) * longueur, ymin, ymax);
    segments[i] = tout.subarray(o, o + 4);
  }
  return segments;
}

/** Longitude, latitude WGS84 (degrés) vers Web Mercator EPSG:3857 (mètres). */
export function versMercator(lon, lat) {
  const phi = Math.max(-LATITUDE_MAX, Math.min(LATITUDE_MAX, lat)) * Math.PI / 180;
  // atanh(sin φ) = ln(tan(π/4 + φ/2)), exact à l'équateur.
  return [RAYON_MERCATOR * lon * Math.PI / 180, RAYON_MERCATOR * Math.atanh(Math.sin(phi))];
}

function polygonesGeoJSON(g) {
  if (!g || typeof g !== "object") return [];
  switch (g.type) {
    case "FeatureCollection": return (g.features || []).flatMap(polygonesGeoJSON);
    case "Feature": return polygonesGeoJSON(g.geometry);
    case "GeometryCollection": return (g.geometries || []).flatMap(polygonesGeoJSON);
    case "Polygon": return [g.coordinates];
    case "MultiPolygon": return g.coordinates;
    default: return [];
  }
}

/**
 * Contour GeoJSON (RFC 7946, WGS84 ; Feature, FeatureCollection, Polygon ou MultiPolygon)
 * reprojeté en Web Mercator : { polygones: [[anneau, ...trous], ...], etendue }.
 */
export function contourMercator(geojson) {
  const polygones = polygonesGeoJSON(geojson)
    .map(poly => (poly || [])
      .filter(anneau => Array.isArray(anneau) && anneau.length >= 4)
      .map(anneau => anneau.map(([lon, lat]) => {
        if (!(Math.abs(lon) <= 180 && Math.abs(lat) <= 90)) {
          throw new Error("coordonnées hors WGS84 (GeoJSON RFC 7946 attendu)");
        }
        return versMercator(lon, lat);
      })))
    .filter(poly => poly.length > 0);
  if (polygones.length === 0) throw new Error("aucun polygone dans le GeoJSON");
  const etendue = { xmin: Infinity, ymin: Infinity, xmax: -Infinity, ymax: -Infinity };
  for (const poly of polygones) for (const [x, y] of poly[0]) {
    if (x < etendue.xmin) etendue.xmin = x;
    if (y < etendue.ymin) etendue.ymin = y;
    if (x > etendue.xmax) etendue.xmax = x;
    if (y > etendue.ymax) etendue.ymax = y;
  }
  return { polygones, etendue };
}

function aireSignee(anneau) {
  let a = 0;
  for (let i = 0; i < anneau.length - 1; i++) {
    a += anneau[i][0] * anneau[i + 1][1] - anneau[i + 1][0] * anneau[i][1];
  }
  return a / 2;
}

function orienter(anneau, horaire) {
  const antihoraire = aireSignee(anneau) > 0;
  return antihoraire === horaire ? anneau.slice().reverse() : anneau;
}

/** Anneaux du contour selon la convention ArcGIS : extérieurs horaires, trous antihoraires. */
export function anneauxContrainte(polygones) {
  return polygones.flatMap(poly => poly.map((anneau, i) => orienter(anneau, i === 0)));
}

/**
 * Anneaux du voile : le cadre (horaire), percé des extérieurs du contour (antihoraires) ;
 * les trous éventuels du contour redeviennent des parties voilées (horaires).
 */
export function anneauxVoile(polygones, cadre) {
  const { xmin, ymin, xmax, ymax } = cadre;
  const bord = [[xmin, ymin], [xmin, ymax], [xmax, ymax], [xmax, ymin], [xmin, ymin]];
  return [bord, ...polygones.flatMap(poly => poly.map((anneau, i) => orienter(anneau, i !== 0)))];
}

const CELLULES_MAX_PAR_SEGMENT = 64;

/**
 * Index en grille uniforme des segments de polylignes (Float64Array x, y entrelacés).
 * chercher(xmin, ymin, xmax, ymax) renvoie, sans doublon, les numéros des segments dont
 * la boîte englobante touche le rectangle (vue réutilisée : valable jusqu'à l'appel suivant).
 */
export function indexerSegments(polylignes) {
  let nb = 0;
  for (const p of polylignes) if (p.length >= 4) nb += p.length / 2 - 1;
  const x0 = new Float64Array(nb), y0 = new Float64Array(nb);
  const x1 = new Float64Array(nb), y1 = new Float64Array(nb);
  const etendue = { xmin: Infinity, ymin: Infinity, xmax: -Infinity, ymax: -Infinity };
  let s = 0;
  for (const p of polylignes) {
    if (p.length < 4) continue;
    for (let i = 0; i + 3 < p.length; i += 2, s++) {
      x0[s] = p[i]; y0[s] = p[i + 1]; x1[s] = p[i + 2]; y1[s] = p[i + 3];
    }
    for (let i = 0; i + 1 < p.length; i += 2) {
      const x = p[i], y = p[i + 1];
      if (x < etendue.xmin) etendue.xmin = x;
      if (y < etendue.ymin) etendue.ymin = y;
      if (x > etendue.xmax) etendue.xmax = x;
      if (y > etendue.ymax) etendue.ymax = y;
    }
  }

  const cote = Math.max(1, Math.min(1024, Math.ceil(Math.sqrt(nb / 4))));
  const largeurCellule = nb ? (etendue.xmax - etendue.xmin) / cote || 1 : 1;
  const hauteurCellule = nb ? (etendue.ymax - etendue.ymin) / cote || 1 : 1;
  const colonne = x => Math.max(0, Math.min(cote - 1, Math.floor((x - etendue.xmin) / largeurCellule)));
  const rangee = y => Math.max(0, Math.min(cote - 1, Math.floor((y - etendue.ymin) / hauteurCellule)));

  // Comptage par cellule ; les segments couvrant trop de cellules vont dans une liste à part.
  const debuts = new Uint32Array(cote * cote + 1);
  const longs = [];
  const plage = new Int32Array(4);
  const cellules = (k) => {
    plage[0] = colonne(Math.min(x0[k], x1[k])); plage[1] = colonne(Math.max(x0[k], x1[k]));
    plage[2] = rangee(Math.min(y0[k], y1[k])); plage[3] = rangee(Math.max(y0[k], y1[k]));
    return (plage[1] - plage[0] + 1) * (plage[3] - plage[2] + 1);
  };
  const estLong = new Uint8Array(nb);
  for (let k = 0; k < nb; k++) {
    if (cellules(k) > CELLULES_MAX_PAR_SEGMENT) { estLong[k] = 1; longs.push(k); continue; }
    for (let j = plage[2]; j <= plage[3]; j++) for (let i = plage[0]; i <= plage[1]; i++) debuts[j * cote + i + 1]++;
  }
  for (let c = 1; c < debuts.length; c++) debuts[c] += debuts[c - 1];
  const contenu = new Uint32Array(debuts[debuts.length - 1]);
  const curseur = debuts.slice(0, cote * cote);
  for (let k = 0; k < nb; k++) {
    if (estLong[k]) continue;
    cellules(k);
    for (let j = plage[2]; j <= plage[3]; j++) for (let i = plage[0]; i <= plage[1]; i++) contenu[curseur[j * cote + i]++] = k;
  }

  const marque = new Uint32Array(nb);
  const resultat = new Uint32Array(nb);
  let requete = 0;
  const touche = (k, xmin, ymin, xmax, ymax) =>
    Math.min(x0[k], x1[k]) <= xmax && Math.max(x0[k], x1[k]) >= xmin
    && Math.min(y0[k], y1[k]) <= ymax && Math.max(y0[k], y1[k]) >= ymin;

  function chercher(xmin, ymin, xmax, ymax) {
    let n = 0;
    if (nb === 0 || xmax < etendue.xmin || xmin > etendue.xmax || ymax < etendue.ymin || ymin > etendue.ymax) {
      return resultat.subarray(0, 0);
    }
    requete = (requete + 1) >>> 0;
    if (requete === 0) { marque.fill(0); requete = 1; }
    const i0 = colonne(xmin), i1 = colonne(xmax), j0 = rangee(ymin), j1 = rangee(ymax);
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) {
        const c = j * cote + i;
        for (let e = debuts[c]; e < debuts[c + 1]; e++) {
          const k = contenu[e];
          if (marque[k] === requete) continue;
          marque[k] = requete;
          if (touche(k, xmin, ymin, xmax, ymax)) resultat[n++] = k;
        }
      }
    }
    for (const k of longs) if (touche(k, xmin, ymin, xmax, ymax)) resultat[n++] = k;
    return resultat.subarray(0, n);
  }

  return { nbSegments: nb, etendue, x0, y0, x1, y1, chercher };
}
