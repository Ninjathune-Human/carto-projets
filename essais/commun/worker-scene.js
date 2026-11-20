import init, { importer_zip } from "../banc/pkg/carto_wasm.js";

// Import d'un ZIP eTransmit : { zip: ArrayBuffer, repere: "l93" | "mercator", maitre? }
// -> { scn: ArrayBuffer } (transféré) ou { erreur: { code, … } }.
let pret = null;

self.onmessage = async ({ data: { zip, repere, maitre } }) => {
  try {
    if (!pret) pret = init();
    await pret;
    const scn = importer_zip(new Uint8Array(zip), repere, maitre ?? undefined);
    const buffer = scn.buffer;
    self.postMessage({ scn: buffer }, [buffer]);
  } catch (e) {
    let erreur;
    try {
      erreur = JSON.parse(String((e && e.message) || e));
    } catch {
      erreur = { code: "lecture", message: String((e && e.message) || e) };
    }
    self.postMessage({ erreur });
  }
};
