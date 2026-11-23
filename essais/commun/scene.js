// Lecture du format binaire SCN1 (scène de traits et de surfaces) produit par carto-cad.
// Petit-boutiste. Les points d'une primitive sont rangés à plat (x, y, x, y, ...) dans un Float64Array.

export function lireScene(donnees) {
  let vue;
  if (donnees instanceof ArrayBuffer) vue = new DataView(donnees);
  else if (ArrayBuffer.isView(donnees)) vue = new DataView(donnees.buffer, donnees.byteOffset, donnees.byteLength);
  else throw new TypeError("lireScene attend un ArrayBuffer");

  const taille = vue.byteLength;
  if (taille < 4 || vue.getUint32(0, false) !== 0x53434e31) {
    throw new Error("fichier de scène invalide : signature SCN1 attendue");
  }
  let pos = 4;
  const besoin = (n, quoi) => {
    if (pos + n > taille) throw new Error(`fichier SCN1 tronqué : ${quoi}`);
  };
  const lireU32 = (quoi) => {
    besoin(4, quoi);
    const v = vue.getUint32(pos, true);
    pos += 4;
    return v;
  };
  const decodeur = new TextDecoder("utf-8");

  const nbCalques = lireU32("nombre de calques");
  const calques = [];
  for (let i = 0; i < nbCalques; i++) {
    const n = lireU32(`calque ${i + 1}`);
    besoin(n + 4, `calque ${i + 1}`);
    const nom = decodeur.decode(new Uint8Array(vue.buffer, vue.byteOffset + pos, n));
    pos += n;
    calques.push({ nom, couleur: [vue.getUint8(pos), vue.getUint8(pos + 1), vue.getUint8(pos + 2)] });
    pos += 4;
  }

  const nbPrimitives = lireU32("nombre de primitives");
  const primitives = new Array(nbPrimitives);
  for (let i = 0; i < nbPrimitives; i++) {
    besoin(17, `primitive ${i + 1}`);
    const genre = vue.getUint8(pos);
    if (genre > 1) throw new Error(`fichier SCN1 invalide : genre ${genre} inconnu`);
    const couleur = [vue.getUint8(pos + 1), vue.getUint8(pos + 2), vue.getUint8(pos + 3), vue.getUint8(pos + 4)];
    const calque = vue.getUint32(pos + 5, true);
    const epaisseurMm = vue.getFloat32(pos + 9, true);
    const n = vue.getUint32(pos + 13, true);
    pos += 17;
    besoin(n * 16, `primitive ${i + 1}`);
    const points = new Float64Array(n * 2);
    for (let k = 0; k < n * 2; k++) points[k] = vue.getFloat64(pos + k * 8, true);
    pos += n * 16;
    primitives[i] = { genre: genre === 0 ? "trait" : "surface", couleur, calque, epaisseurMm, points };
  }

  const nJson = lireU32("longueur du rapport");
  besoin(nJson, "rapport");
  const texte = decodeur.decode(new Uint8Array(vue.buffer, vue.byteOffset + pos, nJson));
  pos += nJson;
  if (pos !== taille) throw new Error(`fichier SCN1 invalide : ${taille - pos} octets en trop`);
  return { calques, primitives, rapport: JSON.parse(texte) };
}
