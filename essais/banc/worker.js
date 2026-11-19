import init, { mesurer_zip } from "./pkg/carto_wasm.js";

let pret = null;

self.onmessage = async (e) => {
  const { nom, octets } = e.data;
  try {
    if (!pret) pret = init();
    await pret;
    const debut = performance.now();
    const json = mesurer_zip(new Uint8Array(octets));
    const duree_zip_ms = performance.now() - debut;
    self.postMessage({ nom, resultats: JSON.parse(json), duree_zip_ms });
  } catch (err) {
    self.postMessage({ nom, erreur: String((err && err.message) || err) });
  }
};
