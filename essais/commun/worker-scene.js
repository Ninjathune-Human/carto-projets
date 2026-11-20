// Import d'un ZIP eTransmit : { zip: ArrayBuffer, repere: "l93" | "mercator", maitre? }
// -> { scn: ArrayBuffer } (transféré) ou { erreur: { code, … } }.
// Le module wasm est chargé dynamiquement : un paquet absent ou invalide arrive au catch
// et devient { erreur: { code: "lecture", message } } au lieu d'un échec silencieux du worker.
let pret = null;

function charger() {
  if (!pret) {
    pret = import("../banc/pkg/carto_wasm.js")
      .then(async (m) => {
        await m.default();
        return m;
      })
      .catch((e) => {
        pret = null; // permet une nouvelle tentative au message suivant
        throw e;
      });
  }
  return pret;
}

self.onmessage = async ({ data: { zip, repere, maitre } }) => {
  try {
    const { importer_zip } = await charger();
    const scn = importer_zip(new Uint8Array(zip), repere, maitre ?? undefined);
    const buffer = scn.buffer;
    self.postMessage({ scn: buffer }, [buffer]);
  } catch (e) {
    const texte = String((e && e.message) || e);
    let erreur;
    try {
      erreur = JSON.parse(texte);
    } catch {
      erreur = { code: "lecture", message: texte };
    }
    self.postMessage({ erreur });
  }
};
