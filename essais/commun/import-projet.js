// Import d'un projet ZIP eTransmit, commun à la visionneuse et à la carte : worker wasm
// (worker-scene.js), import le plus récent prioritaire, messages d'erreur, choix du dessin maître,
// et dépôt de fichiers sur la page. Après une erreur, un piège wasm ou un délai dépassé, le worker
// est terminé : le suivant repart d'une instance wasm neuve (un piège laisse l'instance dans un
// état indéfini, et sa mémoire resterait à son pic).

import { lireScene } from "./scene.js";

const comparer = new Intl.Collator("fr", { numeric: true, sensitivity: "base" }).compare;

/** Vrai pour un fichier ZIP, reconnu à son extension ou à son type MIME. */
export const estZip = f => /\.zip$/i.test(f.name) || f.type === "application/zip" || f.type === "application/x-zip-compressed";

/** Durée maximale d'un import, en ms. */
export const DELAI_IMPORT_MS = 120_000;

/**
 * Erreur structurée tirée du texte d'une exception du moteur : le JSON `{ code, … }` de carto-wasm
 * tel quel ; tout autre texte (« unreachable », RuntimeError…) est un piège wasm, `{ code: "piege" }`.
 */
export function erreurMoteur(texte) {
  try {
    const e = JSON.parse(texte);
    if (e && typeof e === "object" && typeof e.code === "string") return e;
  } catch {
    // pas du JSON : piège
  }
  return { code: "piege", message: texte };
}

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
    case "piege":
      return "Lecture impossible : un dessin du ZIP a interrompu le moteur.";
    case "delai":
      return "Import trop long : abandon après 120 s.";
    case "reponse":
      return `${nom} : réponse du module d'import illisible`;
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
  let minuterie = null;

  function obtenirWorker() {
    if (!worker) worker = new Worker(new URL("./worker-scene.js", import.meta.url), { type: "module" });
    return worker;
  }

  // Termine le worker et son délai : le prochain import en crée un neuf.
  function jeterWorker() {
    clearTimeout(minuterie);
    minuterie = null;
    worker?.terminate();
    worker = null;
    occupe = false;
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
    // Un import plus récent remplace celui en cours.
    if (occupe) jeterWorker();
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
      if (demande !== jeton || w !== worker) return;
      clearTimeout(minuterie);
      minuterie = null;
      occupe = false;
      if (data.erreur) {
        jeterWorker();
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
      if (demande !== jeton || w !== worker) return;
      e.preventDefault();
      jeterWorker();
      message("Module d'import indisponible" + (e.message ? ` (${e.message})` : ""), true);
    };
    w.onmessageerror = () => {
      if (demande !== jeton || w !== worker) return;
      jeterWorker();
      message(messageErreur({ code: "reponse" }, nom), true);
    };
    minuterie = setTimeout(() => {
      if (demande !== jeton || w !== worker) return;
      jeterWorker();
      message(messageErreur({ code: "delai" }, nom), true);
    }, DELAI_IMPORT_MS);
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
