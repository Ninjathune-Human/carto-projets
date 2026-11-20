//! Banc natif : mesure des DWG des ZIP d'un dossier et écriture de CSV.
use carto_cad::{ecrire_lignes, extraire_lignes, inventorier, lire_document, lire_entree, mesurer, ErreurCad};
use carto_georef::l93_vers_mercator;
use carto_wasm::{importer_zip_scn, Repere};
use std::fs::File;
use std::io;
use std::path::Path;
use std::time::Instant;

fn io_err(e: csv::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

fn texte_erreur(e: &ErreurCad) -> String {
    e.to_string()
}

/// Mesure chaque DWG de chaque `*.zip` de `zips` ; écrit `acadrust.csv` et `emprises.csv` dans `sortie`.
/// Ne s'arrête jamais sur un fichier défectueux : l'erreur est consignée dans le CSV.
pub fn mesurer_dossier(zips: &Path, sortie: &Path) -> io::Result<()> {
    std::fs::create_dir_all(sortie)?;
    let mut chemins: Vec<_> = std::fs::read_dir(zips)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("zip"))
        })
        .collect();
    chemins.sort();

    let mut mesures = csv::Writer::from_writer(File::create(sortie.join("acadrust.csv"))?);
    let mut emprises = csv::Writer::from_writer(File::create(sortie.join("emprises.csv"))?);
    mesures
        .write_record(["fichier", "chemin", "lecteur", "version", "duree_ms", "type", "nombre", "erreur"])
        .map_err(io_err)?;
    emprises
        .write_record(["fichier", "chemin", "insunits", "xmin", "ymin", "xmax", "ymax", "geodata"])
        .map_err(io_err)?;

    for chemin in chemins {
        let fichier = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let inventaire = std::fs::read(&chemin)
            .map_err(|e| ErreurCad::Zip(e.to_string()))
            .and_then(|octets| inventorier(&octets).map(|inv| (octets, inv)));
        let (octets, inv) = match inventaire {
            Ok(x) => x,
            Err(e) => {
                mesures
                    .write_record([&fichier, "", "acadrust", "", "0", "", "0", &texte_erreur(&e)])
                    .map_err(io_err)?;
                continue;
            }
        };
        for dwg in &inv.dwg {
            // Seule la mesure (lecture + comptage du DWG) est chronométrée, pas l'extraction du ZIP.
            let (resultat, duree) = match lire_entree(&octets, dwg) {
                Ok(o) => {
                    let debut = Instant::now();
                    let r = mesurer(&o);
                    (r, debut.elapsed().as_millis().to_string())
                }
                Err(e) => (Err(e), "0".to_string()),
            };
            match resultat {
                Ok(m) => {
                    if m.par_type.is_empty() {
                        mesures
                            .write_record([&fichier, dwg, "acadrust", &m.version, &duree, "", "0", ""])
                            .map_err(io_err)?;
                    }
                    for (type_, nombre) in &m.par_type {
                        mesures
                            .write_record([
                                &fichier, dwg, "acadrust", &m.version, &duree, type_, &nombre.to_string(), "",
                            ])
                            .map_err(io_err)?;
                    }
                    let (xmin, ymin, xmax, ymax) = match m.emprise {
                        Some([a, b, c, d]) => (a.to_string(), b.to_string(), c.to_string(), d.to_string()),
                        None => Default::default(),
                    };
                    let geodata = if m.geodata { "true" } else { "false" };
                    emprises
                        .write_record([
                            fichier.as_str(),
                            dwg.as_str(),
                            &m.insunits.to_string(),
                            &xmin,
                            &ymin,
                            &xmax,
                            &ymax,
                            geodata,
                        ])
                        .map_err(io_err)?;
                }
                Err(e) => {
                    mesures
                        .write_record([&fichier, dwg, "acadrust", "", &duree, "", "0", &texte_erreur(&e)])
                        .map_err(io_err)?;
                }
            }
        }
    }
    mesures.flush()?;
    emprises.flush()?;
    Ok(())
}

/// Exporte les lignes d'un DWG d'un ZIP vers un `.lignes` en Web Mercator.
/// Coordonnées × `facteur` (vers mètres L93), puis `l93_vers_mercator`. Retourne (polylignes, points).
pub fn exporter_lignes(zip: &Path, chemin: &str, facteur: f64, sortie: &Path) -> Result<(usize, usize), String> {
    let octets = std::fs::read(zip).map_err(|e| format!("{} : {e}", zip.display()))?;
    let dwg = lire_entree(&octets, chemin).map_err(|e| e.to_string())?;
    let doc = lire_document(&dwg).map_err(|e| e.to_string())?;
    let mut polys = extraire_lignes(&doc);
    for p in &mut polys {
        for pt in &mut p.points {
            let (x, y) = l93_vers_mercator(pt[0] * facteur, pt[1] * facteur).map_err(|e| e.to_string())?;
            *pt = [x, y];
        }
    }
    std::fs::write(sortie, ecrire_lignes(&polys)).map_err(|e| format!("{} : {e}", sortie.display()))?;
    Ok((polys.len(), polys.iter().map(|p| p.points.len()).sum()))
}

fn u32_le(o: &[u8], i: usize) -> Result<usize, String> {
    o.get(i..i + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
        .ok_or_else(|| "SCN1 tronqué".to_string())
}

/// Extrait le rapport JSON en fin de SCN1.
fn rapport_du_scn(scn: &[u8]) -> Result<String, String> {
    if scn.get(..4) != Some(b"SCN1".as_slice()) {
        return Err("SCN1 invalide".into());
    }
    let nc = u32_le(scn, 4)?;
    let mut i = 8;
    for _ in 0..nc {
        i += 4 + u32_le(scn, i)? + 4;
    }
    let n = u32_le(scn, i)?;
    i += 4;
    for _ in 0..n {
        let pts = u32_le(scn, i + 1 + 4 + 4 + 4)?;
        i += 1 + 4 + 4 + 4 + 4 + 16 * pts;
    }
    let r = u32_le(scn, i)?;
    let o = scn.get(i + 4..i + 4 + r).ok_or("SCN1 tronqué")?;
    String::from_utf8(o.to_vec()).map_err(|e| e.to_string())
}

/// Importe le ZIP en scène SCN1, l'écrit dans `sortie` et rend le rapport JSON.
/// En cas d'échec, l'erreur est le JSON structuré du moteur.
pub fn exporter_scene(zip: &Path, repere: Repere, maitre: Option<&str>, sortie: &Path) -> Result<String, String> {
    let octets = std::fs::read(zip).map_err(|e| format!("{} : {e}", zip.display()))?;
    let scn = importer_zip_scn(&octets, repere, maitre)?;
    std::fs::write(sortie, &scn).map_err(|e| format!("{} : {e}", sortie.display()))?;
    rapport_du_scn(&scn)
}
