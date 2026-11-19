import init, { lister_dwg, mesurer_dwg } from "./pkg/carto_wasm.js";

// Un message = une opération sur un seul DWG (R15) : "ouvrir" garde le ZIP et liste ses DWG,
// "mesurer" mesure un DWG du ZIP gardé. Un piège wasm tue le worker : la page le recrée.
let pret = null;
let zip = null;

self.onmessage = async (e) => {
  try {
    if (!pret) pret = init();
    await pret;
    if (e.data.type === "ouvrir") {
      zip = new Uint8Array(e.data.octets);
      self.postMessage({ chemins: JSON.parse(lister_dwg(zip)) });
    } else if (e.data.type === "mesurer") {
      if (!zip) throw new Error("aucun ZIP ouvert");
      self.postMessage({ element: JSON.parse(mesurer_dwg(zip, e.data.chemin)) });
    } else {
      throw new Error("message inconnu");
    }
  } catch (err) {
    self.postMessage({ erreur: String((err && err.message) || err) });
  }
};
