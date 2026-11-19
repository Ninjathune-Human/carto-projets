import { creerPilote } from "./pilote.js";

const zone = document.getElementById("zone");
const entree = document.getElementById("fichiers");
const corps = document.querySelector("#resultats tbody");
const statut = document.getElementById("statut");
const bouton = document.getElementById("copier");

const lignesCsv = [];
let file = Promise.resolve();
const pilote = creerPilote({ creerWorker: () => new Worker("./worker.js", { type: "module" }) });

function cellule(tr, texte, classe) {
  const td = document.createElement("td");
  td.textContent = texte;
  if (classe) td.className = classe;
  tr.appendChild(td);
}

function ajouterLigne(fichier, dwg, entites, inconnues, duree, erreur) {
  const tr = document.createElement("tr");
  cellule(tr, fichier);
  cellule(tr, dwg);
  cellule(tr, entites, "num");
  cellule(tr, inconnues, "num");
  cellule(tr, duree, "num");
  cellule(tr, erreur, erreur ? "err" : "");
  corps.appendChild(tr);
}

function fmt(ms) {
  return typeof ms === "number" ? ms.toFixed(1) : "";
}

function enregistrer(fichier, el) {
  if (el.mesure) {
    const m = el.mesure;
    ajouterLigne(fichier.name, el.chemin, String(m.total), String(m.inconnues), fmt(el.duree_ms), "");
    const types = Object.entries(m.par_type);
    if (types.length === 0) {
      lignesCsv.push([fichier.name, el.chemin, "acadrust-wasm", m.version, el.duree_ms, "", 0, ""]);
    }
    for (const [type, nombre] of types) {
      lignesCsv.push([fichier.name, el.chemin, "acadrust-wasm", m.version, el.duree_ms, type, nombre, ""]);
    }
  } else {
    ajouterLigne(fichier.name, el.chemin, "", "", fmt(el.duree_ms), el.erreur);
    lignesCsv.push([fichier.name, el.chemin, "acadrust-wasm", "", el.duree_ms, "", 0, el.erreur]);
  }
}

// Un DWG par message (R15) : une panne, un message illisible ou un délai dépassé ne coûte que
// le DWG en cours ; le pilote recrée le worker et la suite du ZIP est mesurée.
async function traiter(fichier) {
  const lireOctets = () => fichier.arrayBuffer();
  let chemins;
  try {
    statut.textContent = "Lecture de " + fichier.name + "…";
    chemins = await pilote.lister(fichier, lireOctets);
  } catch (err) {
    ajouterLigne(fichier.name, "", "", "", "", err.message);
    lignesCsv.push([fichier.name, "", "acadrust-wasm", "", "", "", 0, err.message]);
    return;
  }
  if (chemins.length === 0) {
    ajouterLigne(fichier.name, "", "", "", "", "aucun DWG dans le ZIP");
    return;
  }
  for (const [i, chemin] of chemins.entries()) {
    statut.textContent = fichier.name + " : DWG " + (i + 1) + " sur " + chemins.length + "…";
    try {
      enregistrer(fichier, await pilote.mesurer(fichier, lireOctets, chemin));
    } catch (err) {
      ajouterLigne(fichier.name, chemin, "", "", "", err.message);
      lignesCsv.push([fichier.name, chemin, "acadrust-wasm", "", "", "", 0, err.message]);
    }
  }
}

function ajouter(liste) {
  const zips = Array.from(liste).filter((f) => /\.zip$/i.test(f.name));
  if (zips.length === 0) {
    statut.textContent = "Aucun fichier .zip sélectionné.";
    return;
  }
  for (const f of zips) file = file.then(() => traiter(f));
  file = file.then(() => {
    statut.textContent = "Terminé : " + lignesCsv.length + " ligne(s) CSV.";
    bouton.disabled = lignesCsv.length === 0;
  });
}

function champCsv(v) {
  const s = String(v ?? "");
  return /[",\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
}

function csv() {
  const entete = "fichier,chemin,lecteur,version,duree_ms,type,nombre,erreur";
  return [entete, ...lignesCsv.map((l) => l.map(champCsv).join(","))].join("\r\n") + "\r\n";
}

bouton.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(csv());
    statut.textContent = "CSV copié dans le presse-papiers.";
  } catch {
    statut.textContent = "Copie impossible : autorisation du presse-papiers refusée.";
  }
});

entree.addEventListener("change", () => {
  ajouter(entree.files);
  entree.value = "";
});
for (const ev of ["dragenter", "dragover"]) {
  zone.addEventListener(ev, (e) => { e.preventDefault(); zone.classList.add("actif"); });
}
for (const ev of ["dragleave", "drop"]) {
  zone.addEventListener(ev, (e) => { e.preventDefault(); zone.classList.remove("actif"); });
}
zone.addEventListener("drop", (e) => ajouter(e.dataTransfer.files));
