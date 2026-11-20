// Import d'un projet ZIP eTransmit, commun à la visionneuse et à la carte : worker wasm
// (worker-scene.js), import le plus récent prioritaire, messages d'erreur, choix du dessin maître,
// et dépôt de fichiers sur la page.

import { lireScene } from "./scene.js";

const comparer = new Intl.Collator("fr", { numeric: true, sensitivity: "base" }).compare;

/** Vrai pour un fichier ZIP, reconnu à son extension ou à son type MIME. */
export const estZip = f => /\.zip$/i.test(f.name) || f.type === "application/zip" || f.type === "application/x-zip-compressed";

/** Message affiché pour une erreur du worker ({ code, … }) sur le fichier `nom`. */
export function messageErreur(erreur, nom) {
  switch (erreur?.code) {
    case "aucun_dwg":
      return `${nom} : aucun dessin DWG dans le ZIP`;
    case "plusieurs_maitres":
      return `${nom} : plusieurs dessins maîtres possibles, choisir dans la liste`;
    case "non_georeference":
      return erreur.message || "Plan non géoréférencé";
    case "lecture":
      return `${nom} : lecture impossible (${erreur.message})`;
    default:
      return `${nom} : import impossible (${JSON.stringify(erreur)})`;
  }
}

/**
 * Importeur de projets ZIP. Options :
 * - repere : repère de sortie du worker, "l93" ou "mercator" ;
 * - message(texte, alerte) : affiche l'état de l'import ;
 * - ouvrirScene(scene, nom, ms) : affiche la scène lue ; nom du fichier, durée de l'import en ms ;
 * - maitres : bloc du choix du dessin maître, masqué hors choix ; listeMaitres : sa liste (ul) ;
 * - choixPropose() : facultatif, appelé quand la liste s'affiche (pour déplier le panneau).
 * Rend importer(fichier, maitre?) ; un import plus récent remplace celui en cours.
 */
export function creerImporteur({ repere, message, ouvrirScene, maitres, listeMaitres, choixPropose }) {
  let worker = null;
  let occupe = false;
  let jeton = 0;

  function obtenirWorker() {
    if (!worker) worker = new Worker(new URL("./worker-scene.js", import.meta.url), { type: "module" });
    return worker;
  }

  // Un bouton par candidat : un appui réimporte le même fichier avec ce dessin maître.
  function proposerMaitres(candidats, fichier) {
    listeMaitres.replaceChildren(...[...candidats].sort(comparer).map(nom => {
      const li = document.createElement("li");
      const bouton = document.createElement("button");
      bouton.type = "button";
      bouton.textContent = nom;
      bouton.addEventListener("click", () => importer(fichier, nom));
      li.append(bouton);
      return li;
    }));
    maitres.hidden = candidats.length === 0;
  }

  async function importer(fichier, maitre) {
    const demande = ++jeton;
    const nom = fichier.name;
    maitres.hidden = true;
    if (occupe && worker) {
      // Un import plus récent remplace celui en cours.
      worker.terminate();
      worker = null;
    }
    occupe = true;
    message(maitre ? `Import de ${nom} (${maitre})…` : `Import de ${nom}…`);
    let zip;
    try {
      zip = await fichier.arrayBuffer();
    } catch (e) {
      if (demande === jeton) {
        occupe = false;
        message(`${nom} : lecture du fichier impossible (${e.message})`, true);
      }
      return;
    }
    if (demande !== jeton) return;
    let w;
    try {
      w = obtenirWorker();
    } catch (e) {
      occupe = false;
      message(`Module d'import indisponible (${e.message})`, true);
      return;
    }
    const debut = performance.now();
    w.onmessage = ({ data }) => {
      if (demande !== jeton) return;
      occupe = false;
      if (data.erreur) {
        const plusieurs = data.erreur.code === "plusieurs_maitres";
        if (plusieurs) proposerMaitres(data.erreur.candidats || [], fichier);
        message(messageErreur(data.erreur, nom), true);
        if (plusieurs) choixPropose?.();
        return;
      }
      let scene;
      try {
        scene = lireScene(data.scn);
      } catch (e) {
        message(`${nom} : scène illisible (${e.message})`, true);
        return;
      }
      try {
        ouvrirScene(scene, nom, performance.now() - debut);
      } catch (e) {
        message(`${nom} : affichage impossible (${e.message})`, true);
      }
    };
    w.onerror = e => {
      if (demande !== jeton) return;
      e.preventDefault();
      occupe = false;
      worker = null;
      w.terminate();
      message("Module d'import indisponible" + (e.message ? ` (${e.message})` : ""), true);
    };
    w.postMessage({ zip, repere, maitre }, [zip]);
  }

  return importer;
}

/**
 * Dépôt de fichiers sur la page : la classe `depot` marque `cible` pendant le survol, puis
 * deposer(fichiers) reçoit les fichiers lâchés (tableau, éventuellement vide). Écoute la fenêtre
 * en phase de capture : la vue (ArcGIS sur la carte) ne peut pas intercepter le dépôt, et le
 * navigateur n'ouvre jamais le fichier à la place de la page.
 */
export function ecouterDepot(cible, deposer) {
  const avecFichiers = e => [...(e.dataTransfer?.types || [])].includes("Files");
  let survols = 0;
  addEventListener("dragenter", e => {
    if (!avecFichiers(e)) return;
    e.preventDefault();
    survols++;
    cible.classList.add("depot");
  }, true);
  addEventListener("dragover", e => {
    if (!avecFichiers(e)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  }, true);
  addEventListener("dragleave", e => {
    if (!avecFichiers(e)) return;
    survols = Math.max(0, survols - 1);
    if (survols === 0) cible.classList.remove("depot");
  }, true);
  addEventListener("drop", e => {
    if (!avecFichiers(e)) return;
    e.preventDefault();
    survols = 0;
    cible.classList.remove("depot");
    deposer([...e.dataTransfer.files]);
  }, true);
}
