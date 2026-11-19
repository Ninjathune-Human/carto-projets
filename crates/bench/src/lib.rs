//! Banc natif : mesure des DWG des ZIP d'un dossier et écriture de CSV.
use carto_cad::{inventorier, lire_entree, mesurer, ErreurCad};
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
            let debut = Instant::now();
            let resultat = lire_entree(&octets, dwg).and_then(|o| mesurer(&o));
            let duree = debut.elapsed().as_millis().to_string();
            match resultat {
                Ok(m) => {
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
