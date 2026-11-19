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

// Noms d'entrées de ZIP, décodés comme le crate zip côté Rust : UTF-8 si les octets le sont
// (cas de l'indicateur de l'octet 11), sinon CP437. fflate, lui, lit en Latin-1 sans l'indicateur.
const CP437_HAUT = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■ ";

function decoderNom(octets) {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(octets);
  } catch {
    return Array.from(octets, (o) => (o < 0x80 ? String.fromCharCode(o) : CP437_HAUT[o - 0x80])).join("");
  }
}

// Table nom fflate -> nom corrigé, lue dans le répertoire central ; null si la structure n'est pas lisible
// (alors les noms de fflate restent tels quels).
function nomsCorriges(zip) {
  const vue = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
  let eocd = -1;
  for (let i = zip.length - 22; i >= Math.max(0, zip.length - 22 - 65535); i--) {
    if (vue.getUint32(i, true) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) return null;
  const total = vue.getUint16(eocd + 10, true);
  let p = vue.getUint32(eocd + 16, true);
  if (total === 0xffff || p === 0xffffffff) return null; // ZIP64
  const table = new Map();
  for (let n = 0; n < total; n++) {
    if (p + 46 > zip.length || vue.getUint32(p, true) !== 0x02014b50) return null;
    const drapeaux = vue.getUint16(p + 8, true);
    const [ln, le, lc] = [vue.getUint16(p + 28, true), vue.getUint16(p + 30, true), vue.getUint16(p + 32, true)];
    const brut = zip.subarray(p + 46, p + 46 + ln);
    const nomFflate = drapeaux & 0x800 ? new TextDecoder().decode(brut) : String.fromCharCode(...brut);
    table.set(nomFflate, decoderNom(brut));
    p += 46 + ln + le + lc;
  }
  return table;
}

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
  let corriges = null;
  try {
    const zip = new Uint8Array(octetsZip);
    entrees = unzipSync(zip, { filter: (f) => /\.dwg$/i.test(f.name) });
    corriges = nomsCorriges(zip);
  } catch (e) {
    return [ligne(nomZip, "", { erreur: `ZIP illisible : ${message(e)}` })];
  }
  const dwgs = Object.entries(entrees)
    .map(([nom, octets]) => [(corriges?.get(nom) ?? nom).replace(/\\/g, "/"), octets])
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
