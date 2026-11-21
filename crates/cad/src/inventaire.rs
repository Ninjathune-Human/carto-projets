use crate::erreur::ErreurCad;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Contenu d'un ZIP eTransmit, chemins normalisés (séparateur `/`), listes triées.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Inventaire {
    pub dwg: Vec<String>,
    pub polices: Vec<String>,
    pub images: Vec<String>,
    pub styles_trace: Vec<String>,
    pub autres: Vec<String>,
}

pub(crate) fn normaliser(nom: &str) -> String {
    nom.replace('\\', "/")
}

pub(crate) fn nom_de<R: Read>(entree: &zip::read::ZipFile<'_, R>) -> Result<String, ErreurCad> {
    entree
        .name()
        .map(|n| n.into_owned())
        .map_err(|e| ErreurCad::Zip(e.to_string()))
}

pub(crate) fn ouvrir(octets: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, ErreurCad> {
    ZipArchive::new(Cursor::new(octets)).map_err(|e| ErreurCad::Zip(e.to_string()))
}

/// Classe les fichiers du ZIP par extension (sans tenir compte de la casse).
pub fn inventorier(octets: &[u8]) -> Result<Inventaire, ErreurCad> {
    let mut archive = ouvrir(octets)?;
    let mut inv = Inventaire::default();
    for i in 0..archive.len() {
        let entree = archive
            .by_index_raw(i)
            .map_err(|e| ErreurCad::Zip(e.to_string()))?;
        if entree.is_dir() {
            continue;
        }
        let chemin = normaliser(&nom_de(&entree)?);
        let extension = chemin
            .rsplit_once('.')
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_default();
        let liste = match extension.as_str() {
            "dwg" => &mut inv.dwg,
            "shx" | "ttf" | "ttc" | "otf" => &mut inv.polices,
            "jpg" | "jpeg" | "png" | "tif" | "tiff" | "bmp" | "pdf" => &mut inv.images,
            "ctb" | "stb" => &mut inv.styles_trace,
            _ => &mut inv.autres,
        };
        liste.push(chemin);
    }
    for liste in [
        &mut inv.dwg,
        &mut inv.polices,
        &mut inv.images,
        &mut inv.styles_trace,
        &mut inv.autres,
    ] {
        liste.sort();
    }
    Ok(inv)
}

/// Lit le contenu d'une entrée désignée par son chemin normalisé.
pub fn lire_entree(octets: &[u8], chemin: &str) -> Result<Vec<u8>, ErreurCad> {
    let mut archive = ouvrir(octets)?;
    for i in 0..archive.len() {
        let mut entree = archive
            .by_index(i)
            .map_err(|e| ErreurCad::Zip(e.to_string()))?;
        if normaliser(&nom_de(&entree)?) == chemin {
            let mut contenu = Vec::new();
            entree
                .read_to_end(&mut contenu)
                .map_err(|e| ErreurCad::Zip(e.to_string()))?;
            return Ok(contenu);
        }
    }
    Err(ErreurCad::EntreeAbsente(chemin.to_string()))
}
