// Banc libredwg-web : compte les entités de l'espace objet de chaque DWG des ZIP d'un dossier.
// Sortie : schéma CSV commun (fichier,chemin,lecteur,version,duree_ms,type,nombre,erreur).
import { readFileSync, readdirSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import { unzipSync } from "fflate";
import { LibreDwg, Dwg_File_Type } from "@mlightcad/libredwg-web";

const LECTEUR = "libredwg";
const SOUS_ENTITES = new Set(["ATTRIB", "SEQEND", "VERTEX"]);
const ENTETE = ["fichier", "chemin", "lecteur", "version", "duree_ms", "type", "nombre", "erreur"];

let libre = null;
async function lib() {
  libre ??= await LibreDwg.create();
  return libre;
}

function ligne(fichier, chemin, extra = {}) {
  return { fichier, chemin, lecteur: LECTEUR, version: "", duree_ms: "", type: "", nombre: 0, erreur: "", ...extra };
}

function message(e) {
  return String(e?.message ?? e).replace(/\s+/g, " ");
}

async function mesurerDwg(fichier, chemin, octets) {
  try {
    const l = await lib();
    const buffer = octets.buffer.slice(octets.byteOffset, octets.byteOffset + octets.byteLength);
    const debut = performance.now();
    const ptr = l.dwg_read_data(buffer, Dwg_File_Type.DWG);
    if (ptr === undefined || ptr === null) {
      return [ligne(fichier, chemin, { duree_ms: Math.round(performance.now() - debut), erreur: "lecture DWG impossible (libredwg)" })];
    }
    // libredwg-web rend un pointeur même pour un fichier invalide : on contrôle la version détectée.
    let db;
    let vt = null;
    try {
      vt = l.dwg_get_version_type?.(ptr) ?? null;
      if (vt && vt.type === "invalid") {
        return [ligne(fichier, chemin, { duree_ms: Math.round(performance.now() - debut), erreur: "DWG invalide (version non reconnue par libredwg)" })];
      }
      db = l.convert(ptr);
    } finally {
      l.dwg_free(ptr);
    }
    const duree_ms = Math.round(performance.now() - debut);
    const version = db?.header?.ACADVER || vt?.hdr || "";
    const comptes = new Map();
    // R12 : espace objet seulement. db.entities mêle espace objet et espace papier ; on lit donc
    // les entités du BLOCK_RECORD "*Model_Space" (même test de nom que libredwg-web : isModelSpace).
    const modele = (db?.tables?.BLOCK_RECORD?.entries ?? []).filter((b) => String(b.name ?? "").toUpperCase() === "*MODEL_SPACE");
    for (const e of modele.flatMap((b) => b.entities ?? [])) {
      if (!e.type || SOUS_ENTITES.has(e.type)) continue;
      comptes.set(e.type, (comptes.get(e.type) ?? 0) + 1);
    }
    if (comptes.size === 0) return [ligne(fichier, chemin, { version, duree_ms })];
    return [...comptes].sort(([a], [b]) => (a < b ? -1 : 1)).map(([type, nombre]) =>
      ligne(fichier, chemin, { version, duree_ms, type, nombre }));
  } catch (e) {
    return [ligne(fichier, chemin, { erreur: message(e) })];
  }
}

export async function mesurerZip(octetsZip, nomZip) {
  let entrees;
  try {
    entrees = unzipSync(new Uint8Array(octetsZip), { filter: (f) => /\.dwg$/i.test(f.name) });
  } catch (e) {
    return [ligne(nomZip, "", { erreur: `ZIP illisible : ${message(e)}` })];
  }
  const dwgs = Object.entries(entrees)
    .map(([nom, octets]) => [nom.replace(/\\/g, "/"), octets])
    .filter(([nom]) => /\.dwg$/i.test(nom))
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  const lignes = [];
  for (const [chemin, octets] of dwgs) lignes.push(...(await mesurerDwg(nomZip, chemin, octets)));
  return lignes;
}

function csv(valeur) {
  const s = String(valeur);
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

export function versCsv(lignes) {
  return [ENTETE.join(","), ...lignes.map((l) => ENTETE.map((c) => csv(l[c])).join(","))].join("\r\n") + "\r\n";
}

async function main(dossier, sortie) {
  const zips = readdirSync(dossier, { withFileTypes: true })
    .filter((d) => d.isFile() && /\.zip$/i.test(d.name))
    .map((d) => d.name)
    .sort();
  const lignes = [];
  for (const nom of zips) {
    try {
      lignes.push(...(await mesurerZip(readFileSync(join(dossier, nom)), nom)));
    } catch (e) {
      lignes.push(ligne(nom, "", { erreur: message(e) }));
    }
  }
  mkdirSync(dirname(sortie), { recursive: true });
  writeFileSync(sortie, versCsv(lignes), "utf8");
  console.log(`${zips.length} ZIP mesurés -> ${sortie}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const [dossier, sortie] = process.argv.slice(2);
  if (!dossier || !sortie) {
    console.error("usage : node mesurer.mjs <dossier_zips> <sortie.csv>");
    process.exit(2);
  }
  await main(dossier, sortie);
}
