// Pilote du worker de mesure : un DWG par message, worker recréé après toute erreur, tout
// message illisible, tout arrêt fatal ou tout délai dépassé (R15). Sans accès au DOM : testable sous Node.
export const DELAI_MS = 120000;

// creerWorker() rend un objet de type Worker (postMessage, terminate, onmessage, onmessageerror, onerror).
export function creerPilote({ creerWorker, delaiMs = DELAI_MS }) {
  let worker = null;
  let ouvert = null; // { cle, chemins } : ZIP gardé par le worker courant

  function recreer() {
    if (worker) worker.terminate();
    worker = null;
    ouvert = null;
  }

  function appeler(message, transferts = []) {
    return new Promise((resolve, reject) => {
      if (!worker) worker = creerWorker();
      let fini = false;
      let minuterie = null;
      const echec = (texte) => {
        if (fini) return;
        fini = true;
        clearTimeout(minuterie);
        recreer();
        reject(new Error(texte));
      };
      worker.onmessage = (e) => {
        if (fini) return;
        if (e.data && e.data.erreur) return echec(e.data.erreur);
        fini = true;
        clearTimeout(minuterie);
        resolve(e.data);
      };
      worker.onmessageerror = () => echec("réponse illisible du lecteur");
      worker.onerror = (e) => echec("le lecteur s'est arrêté (" + ((e && e.message) || "erreur fatale") + ")");
      minuterie = setTimeout(() => echec("délai dépassé"), delaiMs);
      worker.postMessage(message, transferts);
    });
  }

  // Envoie le ZIP au worker s'il ne le détient pas déjà (premier appel, ou worker recréé).
  async function assurerOuvert(cle, lireOctets) {
    if (worker && ouvert && ouvert.cle === cle) return ouvert.chemins;
    const octets = await lireOctets();
    const r = await appeler({ type: "ouvrir", octets }, [octets]);
    ouvert = { cle, chemins: r.chemins };
    return r.chemins;
  }

  return {
    // Chemins des DWG du ZIP ; lève une erreur si le ZIP est illisible.
    lister: (cle, lireOctets) => assurerOuvert(cle, lireOctets),
    // Élément JSON {chemin, duree_ms, mesure|erreur} ; lève une erreur si le worker a échoué
    // (le worker est alors déjà recréé : l'appel suivant rouvre le ZIP).
    async mesurer(cle, lireOctets, chemin) {
      await assurerOuvert(cle, lireOctets);
      return (await appeler({ type: "mesurer", chemin })).element;
    },
  };
}
