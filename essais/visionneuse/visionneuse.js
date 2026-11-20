// Visionneuse de scène : import d'un ZIP eTransmit par le worker wasm (repère Lambert 93),
// rendu WebGL2 sur fond gris clair, calques, rapport, et comparaison avec une capture d'AutoCAD.
// Rien ne quitte le navigateur : le ZIP et la capture sont lus localement.

import { preparerTampons, origineScene, empriseACadrer, creerRendu } from "../commun/rendu-scene.js";
import { creerImporteur, ecouterDepot, estZip } from "../commun/import-projet.js";

const FOND = [0xd9 / 255, 0xd9 / 255, 0xd9 / 255];
const RESOLUTION_MIN = 1e-4;   // m par px CSS : 0,1 mm
const RESOLUTION_MAX = 1e5;
const MARGE = 24;              // px CSS autour de l'emprise, en vue d'ensemble

const $ = id => document.getElementById(id);
const entier = new Intl.NumberFormat("fr-FR");
const comparer = new Intl.Collator("fr", { numeric: true, sensitivity: "base" }).compare;
const pluriel = (n, mot) => `${entier.format(n)} ${mot}${n >= 2 ? "s" : ""}`;

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
addEventListener("unhandledrejection", e => message("erreur", "Erreur : " + (e.reason?.message || e.reason || "inconnue"), true));

// ---------------------------------------------------------------- panneau

const encoche = $("encoche");
const panneauOuvert = () => encoche.getAttribute("aria-expanded") === "true";
function plier(ouvrir) {
  encoche.setAttribute("aria-expanded", String(ouvrir));
  encoche.title = ouvrir ? "Replier le panneau" : "Déplier le panneau";
  $("corps").hidden = !ouvrir;
}
encoche.addEventListener("click", () => plier(!panneauOuvert()));

// ---------------------------------------------------------------- rendu

const vueEl = $("vue");
const canvas = $("plan");
// alpha laissé à true : avec alpha false, Chrome laisse des bandes noires après un redimensionnement.
// Chaque image est effacée en gris opaque et le mélange garde l'alpha à 1 : le canvas reste opaque.
const gl = canvas.getContext("webgl2", { antialias: true, depth: true });
let rendu = null;
let projet = null; // { scene, tampons, visibles, nbTraits, nbSurfaces, cadrage }

const vue = { centre: [0, 0], resolution: 1 }; // repère Lambert 93, m par px CSS
let taille = [1, 1];                            // px CSS

function initialiserRendu() {
  rendu = null;
  try {
    rendu = creerRendu(gl);
  } catch (e) {
    message("webgl", "Affichage du plan impossible : " + e.message, true);
  }
}
if (gl) initialiserRendu();
else message("webgl", "WebGL 2 indisponible dans ce navigateur : le plan ne peut pas être affiché", true);

// Rendu à la demande : une image au plus par rafraîchissement, aucune boucle continue.
let prevu = false;
function demander() {
  if (prevu) return;
  prevu = true;
  requestAnimationFrame(peindre);
}
function peindre() {
  prevu = false;
  if (!gl || gl.isContextLost()) return;
  gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
  gl.clearColor(FOND[0], FOND[1], FOND[2], 1);
  gl.clear(gl.COLOR_BUFFER_BIT);
  if (rendu && projet) rendu.dessiner({ centre: vue.centre, resolution: vue.resolution, rotation: 0, taille });
}

canvas.addEventListener("webglcontextlost", e => {
  e.preventDefault(); // permet la restauration
  rendu = null;
});
canvas.addEventListener("webglcontextrestored", () => {
  initialiserRendu();
  if (rendu && projet) {
    rendu.charger(projet.tampons);
    rendu.calques(projet.visibles);
  }
  demander();
});

// Taille du tampon de dessin = taille CSS × densité de pixels de l'écran.
function redimensionner() {
  const { width: w, height: h } = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  taille = [Math.max(1, w), Math.max(1, h)];
  const pw = Math.max(1, Math.round(w * dpr));
  const ph = Math.max(1, Math.round(h * dpr));
  if (canvas.width !== pw) canvas.width = pw;
  if (canvas.height !== ph) canvas.height = ph;
  demander();
}
new ResizeObserver(redimensionner).observe(canvas);
// Changement de densité (zoom du navigateur, autre écran) sans changement de taille CSS.
(function suivreDensite() {
  matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`)
    .addEventListener("change", () => { redimensionner(); suivreDensite(); }, { once: true });
})();
redimensionner();

// ---------------------------------------------------------------- navigation

const borner = r => Math.min(RESOLUTION_MAX, Math.max(RESOLUTION_MIN, r));

// (px, py) : position en px CSS depuis le coin haut gauche du canvas.
function versMonde(px, py) {
  return [vue.centre[0] + (px - taille[0] / 2) * vue.resolution, vue.centre[1] - (py - taille[1] / 2) * vue.resolution];
}

// Place le point du monde `m` sous la position écran (px, py), à la résolution r.
function caler(m, px, py, r) {
  vue.resolution = r;
  vue.centre = [m[0] - (px - taille[0] / 2) * r, m[1] + (py - taille[1] / 2) * r];
  demander();
}

function deplacer(dx, dy) {
  vue.centre = [vue.centre[0] - dx * vue.resolution, vue.centre[1] + dy * vue.resolution];
  demander();
}

function zoomer(px, py, facteur) {
  caler(versMonde(px, py), px, py, borner(vue.resolution / facteur));
}

function vueEnsemble() {
  const e = projet?.cadrage;
  if (!e) return;
  // Zone utile : à droite du panneau déplié, s'il laisse au moins la moitié du plan.
  let gauche = 0;
  if (panneauOuvert()) {
    const rc = canvas.getBoundingClientRect();
    const g = $("panneau").getBoundingClientRect().right - rc.left;
    if (g > 0 && rc.width - g >= rc.width / 2) gauche = g;
  }
  const largeur = Math.max(1, taille[0] - gauche - 2 * MARGE);
  const hauteur = Math.max(1, taille[1] - 2 * MARGE);
  let r = Math.max((e[2] - e[0]) / largeur, (e[3] - e[1]) / hauteur);
  if (!(r > 0)) r = 0.01; // emprise réduite à un point
  caler([(e[0] + e[2]) / 2, (e[1] + e[3]) / 2], gauche + (taille[0] - gauche) / 2, taille[1] / 2, borner(r));
}
$("ensemble").addEventListener("click", vueEnsemble);

// Souris, doigt et stylet : glisser pour déplacer, pincer pour zoomer.
const pointeurs = new Map();
const position = e => {
  const r = canvas.getBoundingClientRect();
  return { x: e.clientX - r.left, y: e.clientY - r.top };
};

canvas.addEventListener("pointerdown", e => {
  if (e.pointerType === "mouse" && e.button !== 0 && e.button !== 1) return; // gauche, ou molette comme dans AutoCAD
  canvas.setPointerCapture(e.pointerId);
  pointeurs.set(e.pointerId, position(e));
  canvas.classList.add("saisi");
});

canvas.addEventListener("pointermove", e => {
  const avant = pointeurs.get(e.pointerId);
  if (!avant) return;
  const apres = position(e);
  if (pointeurs.size === 1) {
    deplacer(apres.x - avant.x, apres.y - avant.y);
  } else {
    // Deux pointeurs : le point du monde sous leur milieu suit le milieu, l'écart règle le zoom.
    let autre = null;
    for (const [id, p] of pointeurs) if (id !== e.pointerId) { autre = p; break; }
    const milieu0 = { x: (avant.x + autre.x) / 2, y: (avant.y + autre.y) / 2 };
    const milieu1 = { x: (apres.x + autre.x) / 2, y: (apres.y + autre.y) / 2 };
    const d0 = Math.hypot(avant.x - autre.x, avant.y - autre.y);
    const d1 = Math.hypot(apres.x - autre.x, apres.y - autre.y);
    const r = d0 >= 1 && d1 >= 1 ? borner(vue.resolution * d0 / d1) : vue.resolution;
    caler(versMonde(milieu0.x, milieu0.y), milieu1.x, milieu1.y, r);
  }
  pointeurs.set(e.pointerId, apres);
});

function relacher(e) {
  pointeurs.delete(e.pointerId);
  if (pointeurs.size === 0) canvas.classList.remove("saisi");
}
canvas.addEventListener("pointerup", relacher);
canvas.addEventListener("pointercancel", relacher);
canvas.addEventListener("lostpointercapture", relacher);

// Molette : zoom autour du curseur ; pincement sur pavé tactile (ctrlKey) plus fin.
canvas.addEventListener("wheel", e => {
  e.preventDefault();
  const unite = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? taille[1] : 1;
  const k = e.ctrlKey ? 0.01 : 0.002;
  const facteur = Math.min(2, Math.max(0.5, Math.exp(-e.deltaY * unite * k)));
  const p = position(e);
  zoomer(p.x, p.y, facteur);
}, { passive: false });

// Double clic de la molette : vue d'ensemble, comme le zoom étendu d'AutoCAD.
canvas.addEventListener("mousedown", e => {
  e.preventDefault(); // ni sélection de texte ni défilement automatique
  if (e.button === 1 && e.detail === 2) vueEnsemble();
});
canvas.addEventListener("contextmenu", e => e.preventDefault());
canvas.addEventListener("gesturestart", e => e.preventDefault()); // Safari : pas de zoom de la page

// ---------------------------------------------------------------- import du projet

const importer = creerImporteur({
  repere: "l93",
  message: (texte, alerte) => message("projet", texte, alerte),
  ouvrirScene,
  maitres: $("maitres"),
  listeMaitres: $("liste-maitres"),
  choixPropose: () => plier(true),
});

function ouvrirScene(scene, nomFichier, ms) {
  const tampons = preparerTampons(scene, origineScene(scene));
  let nbTraits = 0;
  for (const p of scene.primitives) if (p.genre === "trait") nbTraits++;
  projet = {
    scene,
    tampons,
    visibles: scene.calques.map(() => true),
    nbTraits,
    nbSurfaces: scene.primitives.length - nbTraits,
    // Un objet égaré loin du plan ne réduit pas le plan à un point (voir empriseACadrer).
    cadrage: empriseACadrer(scene.rapport, tampons.emprise),
  };
  rendu?.charger(tampons);
  remplirCalques();
  remplirRapport();
  $("ensemble").disabled = !projet.cadrage;
  vueEnsemble();
  demander();
  const nom = scene.rapport?.maitre || nomFichier;
  if (!tampons.emprise) message("projet", `${nom} : aucune primitive à afficher`, true);
  else message("projet", `${nom} : ${pluriel(projet.nbTraits, "trait")} et ${pluriel(projet.nbSurfaces, "surface")}, importé en ${entier.format(Math.round(ms))} ms`);
}

function remplirCalques() {
  const { scene, visibles } = projet;
  const liste = scene.calques.map((c, i) => ({ ...c, i })).sort((a, b) => comparer(a.nom, b.nom));
  if (liste.length === 0) {
    const li = document.createElement("li");
    li.className = "vide";
    li.textContent = "Aucun calque";
    $("calques").replaceChildren(li);
    return;
  }
  $("calques").replaceChildren(...liste.map(c => {
    const li = document.createElement("li");
    const label = document.createElement("label");
    const case_ = document.createElement("input");
    case_.type = "checkbox";
    case_.checked = visibles[c.i];
    case_.addEventListener("change", () => {
      visibles[c.i] = case_.checked;
      rendu?.calques(visibles);
      demander();
    });
    const pastille = document.createElement("span");
    pastille.className = "pastille";
    pastille.style.background = `rgb(${c.couleur[0]}, ${c.couleur[1]}, ${c.couleur[2]})`;
    const nom = document.createElement("span");
    nom.className = "nom";
    nom.textContent = c.nom || "(sans nom)";
    label.append(case_, pastille, nom);
    li.append(label);
    return li;
  }));
}

function element(balise, texte, classe) {
  const e = document.createElement(balise);
  if (texte !== undefined) e.textContent = texte;
  if (classe) e.className = classe;
  return e;
}

function remplirRapport() {
  const r = projet.scene.rapport || {};
  const fiche = element("dl", undefined, "fiche");
  const ligne = (terme, valeur, nombre = false) => fiche.append(element("dt", terme), element("dd", valeur, nombre ? "nombre" : undefined));
  ligne("Système reconnu", r.systeme || "aucun");
  ligne("Dessin maître", r.maitre || "inconnu");
  ligne("Traits", entier.format(projet.nbTraits), true);
  ligne("Surfaces", entier.format(projet.nbSurfaces), true);
  ligne("Calques", entier.format(projet.scene.calques.length), true);

  const blocs = [fiche];
  const dessins = r.dessins || [];
  blocs.push(element("h2", `Dessins (${entier.format(dessins.length)})`));
  const listeDessins = element("ul", undefined, "liste");
  for (const d of dessins) listeDessins.append(element("li", d));
  blocs.push(listeDessins);

  const ignorees = Object.entries(r.ignorees || {}).sort((a, b) => comparer(a[0], b[0]));
  blocs.push(element("h2", "Types ignorés"));
  if (ignorees.length === 0) blocs.push(element("p", "Aucun", "vide"));
  else {
    const ul = element("ul", undefined, "liste");
    for (const [type, n] of ignorees) {
      const li = element("li");
      li.append(element("span", type), element("span", entier.format(n), "nombre"));
      ul.append(li);
    }
    blocs.push(ul);
  }

  const avertissements = r.avertissements || [];
  blocs.push(element("h2", "Avertissements"));
  if (avertissements.length === 0) blocs.push(element("p", "Aucun", "vide"));
  else {
    const ul = element("ul", undefined, "liste avertissements");
    for (const a of avertissements) ul.append(element("li", a));
    blocs.push(ul);
  }
  $("rapport").replaceChildren(...blocs);
}

$("fichier").addEventListener("change", () => {
  const f = $("fichier").files?.[0];
  $("fichier").value = "";
  if (f) importer(f);
});

// ---------------------------------------------------------------- comparaison avec une capture

const capture = $("capture");
let urlCapture = null;

function appliquerDisposition() {
  const mode = !urlCapture ? "seul" : $("superpose").checked ? "superpose" : "cote";
  vueEl.dataset.mode = mode;
  capture.hidden = mode === "seul";
  const superpose = mode === "superpose";
  $("opacite").disabled = !superpose;
  $("ligne-opacite").classList.toggle("inactive", !superpose);
  capture.style.opacity = superpose ? String($("opacite").value / 100) : "";
  for (const r of document.querySelectorAll('input[name="disposition"]')) r.disabled = !urlCapture;
}

function ouvrirCapture(fichier) {
  const ancienne = urlCapture;
  const url = URL.createObjectURL(fichier); // lecture locale : la capture n'est envoyée nulle part
  capture.onload = () => {
    if (capture.src !== url) return;
    message("capture", `Capture : ${fichier.name} (${entier.format(capture.naturalWidth)} × ${entier.format(capture.naturalHeight)} px)`);
  };
  capture.onerror = () => {
    if (capture.src !== url) return;
    URL.revokeObjectURL(url);
    urlCapture = null;
    capture.removeAttribute("src");
    appliquerDisposition();
    message("capture", `${fichier.name} : image illisible`, true);
  };
  urlCapture = url;
  capture.src = url;
  if (ancienne) URL.revokeObjectURL(ancienne);
  appliquerDisposition();
}

$("fichier-capture").addEventListener("change", () => {
  const f = $("fichier-capture").files?.[0];
  $("fichier-capture").value = "";
  if (f) ouvrirCapture(f);
});
for (const r of document.querySelectorAll('input[name="disposition"]')) r.addEventListener("change", appliquerDisposition);
$("opacite").addEventListener("input", () => {
  $("valeur-opacite").textContent = `${$("opacite").value} %`;
  appliquerDisposition();
});

// ---------------------------------------------------------------- dépôt de fichiers sur la page

const estImage = f => f.type.startsWith("image/") || /\.(png|jpe?g|webp|gif|bmp)$/i.test(f.name);

ecouterDepot(vueEl, fichiers => {
  const zip = fichiers.find(estZip);
  const image = fichiers.find(estImage);
  if (zip) importer(zip);
  if (image) ouvrirCapture(image);
  if (!zip && !image) message("projet", "Déposer un projet .zip ou une capture d'écran", true);
});
