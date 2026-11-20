// Essai carte : fond Esri, voile et contour de la Métropole, contraintes de navigation,
// couche WebGL de segments, et projet ZIP eTransmit importé par le worker wasm puis dessiné par
// une couche WebGL de scène. SDK ArcGIS 5.x chargé par CDN, modules obtenus par $arcgis.import.

import { echelleMini, lireLignes, genererSegments, contourMercator, anneauxContrainte, anneauxVoile } from "./outils.js";
import { creerCoucheLignes } from "./couche-lignes.js";
import { creerCoucheScene } from "./couche-scene.js";
import { lireScene } from "../commun/scene.js";
import { empriseACadrer } from "../commun/rendu-scene.js";

// Version épinglée du SDK : seul endroit où elle figure.
const VERSION_SDK = "5.2";
const SDK = `https://js.arcgis.com/${VERSION_SDK}/`;

const CHEMIN_CONTOUR = "../../data/limite_metropole.geojson";
const CLE_STOCKEE = "cle-esri";
// Clé écrite par le workflow à la publication (secret CLE_ESRI) ; config.js est absent en local.
const CLE_PUBLIEE = await import("./config.js").then(m => String(m.CLE_ESRI || "").trim(), () => "");
const SR = { wkid: 3857 };
const NICE = [809111, 5418722];
const DEMI_COTE_ESSAI = 10000;          // segments générés dans ±10 km autour de Nice
const N_MAX = 5000000;
const ECHELLE_SANS_CONTOUR = 250000;
const ECHELLE_NIVEAU_20 = 564.248588;
const TRAIT_MIN_PT = 1.125;             // 1,5 px à l'échelle minimale
const TRAIT_MAX_PT = 4.5;               // 6 px au niveau 20
const COULEUR_CONTOUR = "#1c1c1a";
const MONDE = 20037508.342789244;
const CADRE_VOILE = { xmin: -MONDE, ymin: -MONDE, xmax: MONDE, ymax: MONDE };

const MSG_CONTOUR_ABSENT = "Contour absent : déposer data/limite_metropole.geojson";
const MSG_FOND_NEUTRE = "Clé Esri absente ou refusée : fond neutre";

const $ = id => document.getElementById(id);
const entier = new Intl.NumberFormat("fr-FR");
const decimal = new Intl.NumberFormat("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const comparer = new Intl.Collator("fr", { numeric: true, sensitivity: "base" }).compare;
const pluriel = (n, mot) => `${entier.format(n)} ${mot}${n >= 2 ? "s" : ""}`;
const couleurFond = () => getComputedStyle(document.documentElement).getPropertyValue("--fond").trim() || "#f7f6f2";

// ---------------------------------------------------------------- messages

const messages = new Map();
function message(cle, texte, alerte = false) {
  if (texte) messages.set(cle, { texte, alerte });
  else messages.delete(cle);
  $("messages").replaceChildren(...[...messages.values()].map(m => {
    const li = document.createElement("li");
    li.textContent = m.texte;
    if (m.alerte) li.className = "alerte";
    return li;
  }));
}
addEventListener("error", e => message("erreur", "Erreur : " + (e.message || "inconnue"), true));
addEventListener("unhandledrejection", e => {
  if (e.reason?.name === "AbortError") return; // requête ou animation interrompue : normal
  message("erreur", "Erreur : " + (e.reason?.message || e.reason || "inconnue"), true);
});

// ---------------------------------------------------------------- panneau

const encoche = $("encoche");
function plier(ouvrir) {
  encoche.setAttribute("aria-expanded", String(ouvrir));
  encoche.title = ouvrir ? "Replier le panneau" : "Déplier le panneau";
  $("corps").hidden = !ouvrir;
}
encoche.addEventListener("click", () => plier(encoche.getAttribute("aria-expanded") !== "true"));

function lireCleStockee() {
  try { return (localStorage.getItem(CLE_STOCKEE) || "").trim(); } catch { return ""; }
}

// Une clé saisie dans la page l'emporte sur la clé publiée.
function lireCle() {
  return lireCleStockee() || CLE_PUBLIEE;
}

function enregistrerCle() {
  const cle = $("cle").value.trim();
  try {
    if (cle) localStorage.setItem(CLE_STOCKEE, cle);
    else localStorage.removeItem(CLE_STOCKEE);
  } catch {
    message("fond", "Clé non enregistrée : stockage local indisponible", true);
    return;
  }
  // La clé doit être connue du SDK avant sa première requête vers un service sécurisé.
  location.reload();
}
$("cle").value = lireCleStockee();
if (CLE_PUBLIEE) $("cle").placeholder = "Clé publiée utilisée";
$("enregistrer").addEventListener("click", enregistrerCle);
$("cle").addEventListener("keydown", e => { if (e.key === "Enter") enregistrerCle(); });

// ---------------------------------------------------------------- compteurs

const mesures = { rendus: 0, dureeRendus: 0, tuiles: 0 };
let images = 0;
let debutSeconde = 0;
function boucle(t) {
  images++;
  const dt = t - debutSeconde;
  if (dt >= 1000) {
    $("ips").textContent = entier.format(Math.round(images * 1000 / dt));
    $("rps").textContent = entier.format(Math.round(mesures.rendus * 1000 / dt));
    if (mesures.rendus > 0) $("duree-rendu").textContent = decimal.format(mesures.dureeRendus / mesures.rendus) + " ms";
    $("nb-tuiles").textContent = entier.format(mesures.tuiles);
    images = 0;
    mesures.rendus = 0;
    mesures.dureeRendus = 0;
    debutSeconde = t;
  }
  requestAnimationFrame(boucle);
}
requestAnimationFrame(t => { debutSeconde = t; requestAnimationFrame(boucle); });

// ---------------------------------------------------------------- dépôt d'un fichier sur la carte

const estZip = f => /\.zip$/i.test(f.name) || f.type === "application/zip" || f.type === "application/x-zip-compressed";
const avecFichiers = e => [...(e.dataTransfer?.types || [])].includes("Files");
let ouvrirFichier = null; // fournie quand la carte est prête
let survols = 0;

// Écoute en phase de capture : la vue ArcGIS ne doit pas pouvoir intercepter le dépôt.
addEventListener("dragenter", e => {
  if (!avecFichiers(e)) return;
  e.preventDefault();
  survols++;
  $("carte").classList.add("depot");
}, true);
addEventListener("dragover", e => {
  if (!avecFichiers(e)) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = "copy";
}, true);
addEventListener("dragleave", e => {
  if (!avecFichiers(e)) return;
  survols = Math.max(0, survols - 1);
  if (survols === 0) $("carte").classList.remove("depot");
}, true);
addEventListener("drop", e => {
  if (!avecFichiers(e)) return;
  e.preventDefault();
  survols = 0;
  $("carte").classList.remove("depot");
  const fichiers = [...e.dataTransfer.files];
  if (fichiers.length === 0) return;
  if (!ouvrirFichier) {
    message("plan", "Carte non prête : fichier ignoré", true);
    return;
  }
  ouvrirFichier(fichiers.find(estZip) || fichiers[0]);
}, true);

// ---------------------------------------------------------------- chargements

function attendre(ms) { return new Promise(r => setTimeout(r, ms)); }

function chargerSdk() {
  const style = document.createElement("link");
  style.rel = "stylesheet";
  style.href = SDK + "esri/themes/light/main.css";
  document.head.append(style);
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.type = "module";
    script.src = SDK;
    script.onload = async () => {
      for (let i = 0; i < 100 && typeof globalThis.$arcgis?.import !== "function"; i++) await attendre(50);
      if (typeof globalThis.$arcgis?.import === "function") resolve(globalThis.$arcgis);
      else reject(new Error("$arcgis.import absent"));
    };
    script.onerror = () => reject(new Error("js.arcgis.com injoignable"));
    document.head.append(script);
  });
}

async function chargerContour() {
  let reponse = null;
  try { reponse = await fetch(CHEMIN_CONTOUR, { cache: "no-cache" }); } catch { /* hors ligne */ }
  if (!reponse || !reponse.ok) {
    message("contour", MSG_CONTOUR_ABSENT, true);
    return null;
  }
  try {
    return contourMercator(await reponse.json());
  } catch (e) {
    message("contour", "Contour illisible : " + e.message, true);
    return null;
  }
}

function renduContour(echelleMin) {
  return {
    type: "simple",
    symbol: { type: "simple-line", color: COULEUR_CONTOUR, width: TRAIT_MIN_PT, cap: "round", join: "round" },
    visualVariables: [{
      type: "size",
      valueExpression: "$view.scale",
      stops: [
        { value: ECHELLE_NIVEAU_20, size: TRAIT_MAX_PT },
        { value: echelleMin, size: TRAIT_MIN_PT },
      ],
    }],
  };
}

// ---------------------------------------------------------------- démarrage

async function demarrer() {
  const cle = lireCle();
  if (cle) globalThis.esriConfig = Object.assign(globalThis.esriConfig || {}, { apiKey: cle });
  else message("fond", MSG_FOND_NEUTRE, true);

  const promesseContour = chargerContour();
  message("sdk", `Chargement du SDK ArcGIS ${VERSION_SDK}…`);
  let arcgis;
  try {
    arcgis = await chargerSdk();
  } catch (e) {
    message("sdk", `SDK ArcGIS injoignable (${e.message}) : carte indisponible`, true);
    return;
  }

  const [
    EsriMap, MapView, Basemap, Graphic, GraphicsLayer, FeatureLayer,
    Polygon, Polyline, Point, Extent, Layer, BaseLayerViewGL2D, TileInfo, config,
  ] = await arcgis.import([
    "@arcgis/core/Map.js",
    "@arcgis/core/views/MapView.js",
    "@arcgis/core/Basemap.js",
    "@arcgis/core/Graphic.js",
    "@arcgis/core/layers/GraphicsLayer.js",
    "@arcgis/core/layers/FeatureLayer.js",
    "@arcgis/core/geometry/Polygon.js",
    "@arcgis/core/geometry/Polyline.js",
    "@arcgis/core/geometry/Point.js",
    "@arcgis/core/geometry/Extent.js",
    "@arcgis/core/layers/Layer.js",
    "@arcgis/core/views/2d/layers/BaseLayerViewGL2D.js",
    "@arcgis/core/layers/support/TileInfo.js",
    "@arcgis/core/config.js",
  ]);
  if (cle) config.apiKey = cle;

  // Couches, de bas en haut : plan (WebGL), projet (WebGL), voile, contour. Le fond est en dessous.
  // Ce qui sort de la Métropole reste ainsi sous le voile.
  const construction = (nb, ms) => { $("duree-construction").textContent = `${entier.format(nb)} en ${decimal.format(ms)} ms`; };
  const plan = creerCoucheLignes({ Layer, BaseLayerViewGL2D, TileInfo }, {
    rendu(ms, nbTuiles) { mesures.rendus++; mesures.dureeRendus += ms; mesures.tuiles = nbTuiles; },
    construction,
    erreur(e) { message("plan", "Couche WebGL indisponible : " + e.message, true); },
  });
  const projet = creerCoucheScene({ Layer, BaseLayerViewGL2D, TileInfo }, {
    construction,
    erreur(e) { message("projet", "Couche WebGL du projet indisponible : " + e.message, true); },
  });
  const carte = new EsriMap();
  carte.add(plan.couche);
  carte.add(projet.couche);

  const contour = await promesseContour;
  const conteneur = $("carte");
  const tailleVue = () => [Math.max(1, conteneur.clientWidth), Math.max(1, conteneur.clientHeight)];
  let calculerEchelle = null;
  let echelle = null;
  let trait = null;
  let contrainte = null;
  if (contour) {
    const { etendue, polygones } = contour;
    calculerEchelle = () => echelleMini(etendue.xmax - etendue.xmin, etendue.ymax - etendue.ymin, ...tailleVue());
    echelle = calculerEchelle();
    contrainte = new Polygon({ rings: anneauxContrainte(polygones), spatialReference: SR });
    carte.add(new GraphicsLayer({
      title: "Voile hors Métropole",
      graphics: [new Graphic({
        geometry: new Polygon({ rings: anneauxVoile(polygones, CADRE_VOILE), spatialReference: SR }),
        symbol: { type: "simple-fill", color: [255, 255, 255, 0.5], outline: { color: [0, 0, 0, 0], width: 0 } },
      })],
    }));
    trait = new FeatureLayer({
      title: "Contour de la Métropole",
      source: [new Graphic({
        geometry: new Polyline({ paths: polygones.flat(), spatialReference: SR }),
        attributes: { ObjectID: 1 },
      })],
      fields: [{ name: "ObjectID", type: "oid" }],
      objectIdField: "ObjectID",
      geometryType: "polyline",
      spatialReference: SR,
      renderer: renduContour(echelle),
      popupEnabled: false,
      legendEnabled: false,
    });
    carte.add(trait);
  }

  const centre = contour
    ? [(contour.etendue.xmin + contour.etendue.xmax) / 2, (contour.etendue.ymin + contour.etendue.ymax) / 2]
    : NICE;
  const vue = new MapView({
    container: conteneur,
    map: carte,
    spatialReference: SR,
    center: new Point({ x: centre[0], y: centre[1], spatialReference: SR }),
    scale: echelle ?? ECHELLE_SANS_CONTOUR,
    background: { color: couleurFond() },
    constraints: contour
      ? { geometry: contrainte, minScale: echelle, rotationEnabled: false, snapToZoom: false }
      : { rotationEnabled: false, snapToZoom: false },
  });
  try { vue.ui.move("zoom", "bottom-right"); } catch { /* interface par défaut absente */ }

  // ------------------------------------------------------------ fond Esri
  let jeton = 0;
  const estDuFond = couche => !!carte.basemap
    && (carte.basemap.baseLayers.includes(couche) || carte.basemap.referenceLayers.includes(couche));

  function fondRefuse(erreur) {
    carte.basemap = null;
    message("fond", MSG_FOND_NEUTRE, true);
    message("fond-detail", erreur?.message ? "Détail : " + erreur.message : null);
  }

  async function appliquerFond(id) {
    const essai = ++jeton;
    message("fond", "Chargement du fond…");
    message("fond-detail", null);
    const fond = new Basemap({ style: { id } });
    try {
      await fond.loadAll();
      const echec = [fond, ...fond.baseLayers.toArray(), ...fond.referenceLayers.toArray()]
        .find(c => c.loadStatus === "failed");
      if (echec) throw echec.loadError || new Error("chargement refusé");
      if (fond.baseLayers.length === 0) throw new Error("fond sans couche");
      if (essai !== jeton) return;
      carte.basemap = fond;
      message("fond", null);
    } catch (e) {
      if (essai === jeton) fondRefuse(e);
    }
  }

  vue.on("layerview-create-error", ev => {
    if (ev.layer === plan.couche) message("plan", "Couche WebGL indisponible", true);
    else if (ev.layer === projet.couche) message("projet", "Couche WebGL du projet indisponible", true);
    else if (estDuFond(ev.layer)) fondRefuse(ev.error);
    else message("couche-" + ev.layer?.title, `Couche « ${ev.layer?.title} » indisponible`, true);
  });

  try {
    await vue.when();
  } catch (e) {
    message("sdk", "Vue ArcGIS indisponible : " + (e?.message || e), true);
    return;
  }
  message("sdk", null);

  const radios = document.querySelectorAll('input[name="fond"]');
  if (cle) {
    for (const r of radios) {
      r.disabled = false;
      r.addEventListener("change", () => { if (r.checked) appliquerFond(r.value); });
    }
    appliquerFond(document.querySelector('input[name="fond"]:checked').value);
  }

  // ------------------------------------------------------------ échelle minimale au redimensionnement
  if (contour) {
    let minuterie = 0;
    const ajuster = () => {
      const nouvelle = calculerEchelle();
      if (Math.abs(nouvelle - echelle) <= 1e-9 * echelle) return;
      echelle = nouvelle;
      vue.constraints.minScale = echelle;
      trait.renderer = renduContour(echelle);
      if (vue.scale > echelle) vue.scale = echelle;
    };
    new ResizeObserver(() => {
      clearTimeout(minuterie);
      minuterie = setTimeout(ajuster, 200);
    }).observe(conteneur);
  }

  // ------------------------------------------------------------ données du plan
  // Emprise [xmin, ymin, xmax, ymax] en Web Mercator, avec 10 % de marge.
  function cadrer([xmin, ymin, xmax, ymax]) {
    const marge = 0.1 * Math.max(xmax - xmin, ymax - ymin) || 50;
    vue.goTo(new Extent({
      xmin: xmin - marge, ymin: ymin - marge, xmax: xmax + marge, ymax: ymax + marge, spatialReference: SR,
    })).catch(() => { /* animation interrompue */ });
  }

  function definirPlan(polylignes, libelle) {
    const t = performance.now();
    const index = plan.definir(polylignes);
    const ms = performance.now() - t;
    $("nb-segments").textContent = entier.format(index.nbSegments);
    if (index.nbSegments === 0) {
      message("plan", `${libelle} : aucun segment`, true);
      return;
    }
    message("plan", `${libelle} : ${entier.format(index.nbSegments)} segments indexés en ${entier.format(Math.round(ms))} ms`);
    const e = index.etendue;
    cadrer([e.xmin, e.ymin, e.xmax, e.ymax]);
  }

  async function ouvrirLignes(f) {
    message("plan", `Lecture de ${f.name}…`);
    try {
      definirPlan(lireLignes(await f.arrayBuffer()), f.name);
    } catch (e) {
      message("plan", `${f.name} : ${e.message}`, true);
    }
  }

  // ------------------------------------------------------------ projet (ZIP eTransmit)
  let worker = null;
  let occupe = false;
  let jetonImport = 0;

  function obtenirWorker() {
    if (!worker) worker = new Worker(new URL("../commun/worker-scene.js", import.meta.url), { type: "module" });
    return worker;
  }

  async function importer(f, maitre) {
    const demande = { jeton: ++jetonImport, fichier: f, maitre, nom: f.name };
    $("maitres").hidden = true;
    if (occupe && worker) {
      // Un import plus récent remplace celui en cours.
      worker.terminate();
      worker = null;
    }
    occupe = true;
    message("projet", maitre ? `Import de ${f.name} (${maitre})…` : `Import de ${f.name}…`);
    let zip;
    try {
      zip = await f.arrayBuffer();
    } catch (e) {
      if (demande.jeton === jetonImport) {
        occupe = false;
        message("projet", `${f.name} : lecture du fichier impossible (${e.message})`, true);
      }
      return;
    }
    if (demande.jeton !== jetonImport) return;
    let w;
    try {
      w = obtenirWorker();
    } catch (e) {
      occupe = false;
      message("projet", `Module d'import indisponible (${e.message})`, true);
      return;
    }
    const debut = performance.now();
    w.onmessage = ({ data }) => {
      if (demande.jeton !== jetonImport) return;
      occupe = false;
      if (data.erreur) {
        afficherErreur(data.erreur, demande);
        return;
      }
      let scene;
      try {
        scene = lireScene(data.scn);
      } catch (e) {
        message("projet", `${demande.nom} : scène illisible (${e.message})`, true);
        return;
      }
      try {
        ouvrirScene(scene, demande, performance.now() - debut);
      } catch (e) {
        message("projet", `${demande.nom} : affichage impossible (${e.message})`, true);
      }
    };
    w.onerror = e => {
      if (demande.jeton !== jetonImport) return;
      e.preventDefault();
      occupe = false;
      worker = null;
      w.terminate();
      message("projet", "Module d'import indisponible" + (e.message ? ` (${e.message})` : ""), true);
    };
    w.postMessage({ zip, repere: "mercator", maitre }, [zip]);
  }

  function afficherErreur(erreur, demande) {
    switch (erreur.code) {
      case "aucun_dwg":
        message("projet", `${demande.nom} : aucun dessin DWG dans le ZIP`, true);
        break;
      case "plusieurs_maitres":
        proposerMaitres(erreur.candidats || [], demande);
        message("projet", `${demande.nom} : plusieurs dessins maîtres possibles, choisir dans la liste`, true);
        plier(true);
        break;
      case "non_georeference":
        message("projet", erreur.message || "Plan non géoréférencé", true);
        break;
      case "lecture":
        message("projet", `${demande.nom} : lecture impossible (${erreur.message})`, true);
        break;
      default:
        message("projet", `${demande.nom} : import impossible (${JSON.stringify(erreur)})`, true);
    }
  }

  function proposerMaitres(candidats, demande) {
    $("liste-maitres").replaceChildren(...[...candidats].sort(comparer).map(nom => {
      const li = document.createElement("li");
      const bouton = document.createElement("button");
      bouton.type = "button";
      bouton.textContent = nom;
      bouton.addEventListener("click", () => importer(demande.fichier, nom));
      li.append(bouton);
      return li;
    }));
    $("maitres").hidden = candidats.length === 0;
  }

  function ouvrirScene(scene, demande, ms) {
    const index = projet.definirScene(scene);
    let nbTraits = 0;
    for (const p of scene.primitives) if (p.genre === "trait") nbTraits++;
    const nom = scene.rapport?.maitre || demande.nom;
    const e = index.etendue;
    if (!(e.xmin <= e.xmax)) {
      message("projet", `${nom} : aucune primitive à afficher`, true);
      return;
    }
    // Un objet égaré loin du plan ne réduit pas le plan à un point (même règle que la visionneuse).
    cadrer(empriseACadrer(scene.rapport, [e.xmin, e.ymin, e.xmax, e.ymax]));
    const systeme = scene.rapport?.systeme ? ` (${scene.rapport.systeme})` : "";
    message("projet", `${nom}${systeme} : ${pluriel(nbTraits, "trait")} et ${pluriel(scene.primitives.length - nbTraits, "surface")}, importé en ${entier.format(Math.round(ms))} ms`);
  }

  // ------------------------------------------------------------ ouverture d'un fichier (bouton ou dépôt)
  // ZIP ou .lignes, reconnu à l'extension, au type, ou à défaut à la signature.
  async function nature(f) {
    if (estZip(f)) return "zip";
    if (/\.lignes$/i.test(f.name)) return "lignes";
    try {
      const o = new Uint8Array(await f.slice(0, 4).arrayBuffer());
      if (o[0] === 0x50 && o[1] === 0x4b) return "zip"; // PK
      if (o[0] === 0x4c && o[1] === 0x49 && o[2] === 0x47 && o[3] === 0x30) return "lignes"; // LIG0
    } catch { /* illisible : format inconnu */ }
    return null;
  }

  ouvrirFichier = async f => {
    const n = await nature(f);
    if (n === "zip") importer(f);
    else if (n === "lignes") ouvrirLignes(f);
    else message("plan", `${f.name} : format non reconnu, projet .zip ou fichier .lignes attendu`, true);
  };

  const fichier = $("fichier");
  const generer = $("generer");
  fichier.disabled = false;
  generer.disabled = false;

  fichier.addEventListener("change", () => {
    const f = fichier.files && fichier.files[0];
    fichier.value = "";
    if (f) ouvrirFichier(f);
  });

  generer.addEventListener("click", () => {
    const n = Number($("nombre").value);
    if (!Number.isInteger(n) || n < 1 || n > N_MAX) {
      message("plan", `N doit être un entier de 1 à ${entier.format(N_MAX)}`, true);
      return;
    }
    generer.disabled = true;
    message("plan", `Génération de ${entier.format(n)} segments…`);
    // Laisser le message s'afficher avant le calcul, qui bloque le fil principal.
    requestAnimationFrame(() => setTimeout(() => {
      try {
        const t = performance.now();
        const p = genererSegments(n, NICE[0] - DEMI_COTE_ESSAI, NICE[1] - DEMI_COTE_ESSAI, NICE[0] + DEMI_COTE_ESSAI, NICE[1] + DEMI_COTE_ESSAI);
        definirPlan(p, `Tirage en ${entier.format(Math.round(performance.now() - t))} ms`);
      } catch (e) {
        message("plan", "Génération impossible : " + e.message, true);
      } finally {
        generer.disabled = false;
      }
    }, 0));
  });
}

demarrer().catch(e => message("sdk", "Carte indisponible : " + (e?.message || e), true));
