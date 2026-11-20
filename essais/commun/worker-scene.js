// Import d'un ZIP eTransmit : { zip: ArrayBuffer, repere: "l93" | "mercator", maitre? }
// -> { scn: ArrayBuffer } (transféré) ou { erreur: { code, … } }.
// Le module wasm est chargé dynamiquement : un paquet absent ou invalide arrive au catch
// et devient { erreur: { code: "lecture", message } } au lieu d'un échec silencieux du worker.
// Une exception du moteur qui n'est pas une erreur structurée de carto-wasm est un piège wasm
// (« unreachable », RuntimeError) : { erreur: { code: "piege", message } ; la page termine alors
// ce worker, dont l'instance est dans un état indéfini.
import { erreurMoteur } from "./import-projet.js";

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

const texteDe = (e) => String((e && e.message) || e);

self.onmessage = async ({ data: { zip, repere, maitre } }) => {
  let module;
  try {
    module = await charger();
  } catch (e) {
    self.postMessage({ erreur: { code: "lecture", message: texteDe(e) } });
    return;
  }
  try {
    const scn = module.importer_zip(new Uint8Array(zip), repere, maitre ?? undefined);
    const buffer = scn.buffer;
    self.postMessage({ scn: buffer }, [buffer]);
  } catch (e) {
    self.postMessage({ erreur: erreurMoteur(texteDe(e)) });
  }
};
