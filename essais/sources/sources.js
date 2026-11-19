// Sources open data : fonctions pures (importables par node) et page (lancée seulement dans un navigateur).

export function classerReponse({ ok, status, json, erreurReseau } = {}) {
  if (erreurReseau) return "CORS ou réseau";
  const code = json && json.error && json.error.code;
  if (code === 498 || code === 499) return "jeton requis";
  if (ok) return "OK";
  return "HTTP " + status;
}

const TOLERANCE = 1e-6;

// CRS-agnostique : les trois étendues doivent partager le même système de coordonnées.
export function classerCouverture(etendue, bboxNice, bboxMetropole) {
  if (!etendue || !bboxNice) return "inconnue";
  const t = TOLERANCE;
  const dedans = etendue.xmin >= bboxNice.xmin - t && etendue.ymin >= bboxNice.ymin - t
    && etendue.xmax <= bboxNice.xmax + t && etendue.ymax <= bboxNice.ymax + t;
  return dedans ? "Nice" : "Métropole";
}

// Boîte englobante d'un GeoJSON (Feature, FeatureCollection ou géométrie), en WGS84.
export function bboxGeoJSON(geo) {
  let xmin = Infinity, ymin = Infinity, xmax = -Infinity, ymax = -Infinity;
  const parcourir = (c) => {
    if (typeof c[0] === "number") {
      if (c[0] < xmin) xmin = c[0]; if (c[0] > xmax) xmax = c[0];
      if (c[1] < ymin) ymin = c[1]; if (c[1] > ymax) ymax = c[1];
    } else for (const x of c) parcourir(x);
  };
  const visiter = (g) => {
    if (!g) return;
    if (g.type === "FeatureCollection") g.features.forEach(visiter);
    else if (g.type === "Feature") visiter(g.geometry);
    else if (g.type === "GeometryCollection") g.geometries.forEach(visiter);
    else if (g.coordinates) parcourir(g.coordinates);
  };
  visiter(geo);
  return xmin === Infinity ? null : { xmin, ymin, xmax, ymax };
}

export function tableauMarkdown(entetes, lignes) {
  const c = (s) => String(s).replace(/\|/g, "\\|");
  return [
    "| " + entetes.map(c).join(" | ") + " |",
    "| " + entetes.map(() => "---").join(" | ") + " |",
    ...lignes.map((l) => "| " + l.map(c).join(" | ") + " |"),
  ].join("\n");
}

const TYPES = {
  "arcgis-mapserver": "ArcGIS MapServer", "datagouv-api": "API data.gouv.fr",
  "opendatasoft": "Opendatasoft", "wfs": "WFS", "api-geo": "API Découpage administratif",
};

async function interroger(url) {
  const ctrl = new AbortController();
  const minuteur = setTimeout(() => ctrl.abort(), 15000);
  try {
    const rep = await fetch(url, { mode: "cors", signal: ctrl.signal });
    let json;
    try { json = await rep.clone().json(); } catch { json = undefined; }
    return { ok: rep.ok, status: rep.status, json };
  } catch (e) {
    return { erreurReseau: true };
  } finally {
    clearTimeout(minuteur);
  }
}

function lancerPage() {
  const $ = (id) => document.getElementById(id);
  const corps = $("corps"), etat = $("etat"), bLancer = $("lancer"), bCopier = $("copier");
  const ENTETES = ["Source", "Producteur", "Type", "Statut", "Couverture"];
  let lignes = [];

  function afficher() {
    corps.textContent = "";
    for (const l of lignes) {
      const tr = document.createElement("tr");
      if (l.couche) tr.className = "couche";
      [l.source, l.producteur, l.type, l.statut, l.couverture].forEach((v, i) => {
        const td = document.createElement("td");
        td.textContent = v;
        td.dataset.label = ENTETES[i];
        tr.appendChild(td);
      });
      corps.appendChild(tr);
    }
  }

  async function lancer() {
    bLancer.disabled = true; bCopier.disabled = true;
    etat.textContent = "Chargement du catalogue...";
    let catalogue;
    try {
      catalogue = await (await fetch("../../catalogue.json")).json();
    } catch (e) {
      etat.textContent = "Catalogue illisible : " + e.message;
      bLancer.disabled = false;
      return;
    }
    lignes = [];
    const notes = [];
    const sep = (u) => (u.includes("?") ? "&" : "?");

    // Boîtes englobantes de référence, en WGS84.
    let bboxNice = null, bboxMetropole = null;
    const entreeNice = catalogue.find((e) => e.type === "api-geo");
    const reponseNice = entreeNice ? await interroger(entreeNice.url) : null;
    if (reponseNice && reponseNice.ok && reponseNice.json) bboxNice = bboxGeoJSON(reponseNice.json);
    if (!bboxNice) notes.push("contour de Nice inaccessible : couverture inconnue");
    try {
      const r = await fetch("../../data/limite_metropole.geojson");
      if (r.ok) bboxMetropole = bboxGeoJSON(await r.json());
    } catch (e) { /* absent : bboxMetropole reste null */ }
    if (!bboxMetropole) notes.push("limite de la Métropole absente (data/limite_metropole.geojson)");

    for (const e of catalogue) {
      etat.textContent = "Vérification : " + e.titre + "...";
      const type = TYPES[e.type] || e.type;
      let rep;
      if (e.type === "api-geo" && reponseNice) rep = reponseNice;
      else rep = await interroger(e.type === "arcgis-mapserver" ? e.url + "?f=json" : e.url);
      lignes.push({ source: e.titre, producteur: e.producteur, type, statut: classerReponse(rep), couverture: "", couche: false });
      afficher();
      for (const c of e.couches || []) {
        const q = e.url + "/" + c.id + "/query?where=1%3D1&returnExtentOnly=true&outSR=4326&f=json";
        const r = await interroger(q);
        const statut = classerReponse(r);
        const ext = statut === "OK" && r.json && r.json.extent ? r.json.extent : null;
        lignes.push({
          source: c.id + " " + c.nom, producteur: e.producteur, type: "Couche ArcGIS", statut,
          couverture: classerCouverture(ext, bboxNice, bboxMetropole), couche: true,
        });
        afficher();
      }
    }
    etat.textContent = "Terminé : " + lignes.length + " lignes." + (notes.length ? " Note : " + notes.join(" ; ") + "." : "");
    bLancer.disabled = false; bCopier.disabled = false;
  }

  async function copier() {
    const md = tableauMarkdown(ENTETES, lignes.map((l) => [l.source, l.producteur, l.type, l.statut, l.couverture]));
    try {
      await navigator.clipboard.writeText(md);
      etat.textContent = "Tableau copié en Markdown.";
    } catch (e) {
      etat.textContent = "Copie impossible : " + e.message;
    }
  }

  bLancer.addEventListener("click", lancer);
  bCopier.addEventListener("click", copier);
}

if (typeof document !== "undefined") lancerPage();
