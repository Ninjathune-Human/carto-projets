//! Import d'un ZIP eTransmit : lecture de tous les DWG, résolution des chemins enregistrés des
//! xréfs dans l'archive, choix du dessin maître, puis aplatissement.

use crate::aplatir::{aplatir, Aplatissement, Dessins};
use crate::inventaire::{nom_de, normaliser, ouvrir};
use crate::mesure::lire_document;
use crate::ErreurCad;
use acadrust::BlockRecord;
use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::io::Read;

/// Vrai pour un bloc qui désigne un dessin externe (rattaché, en superposition ou déchargé).
pub(crate) fn est_xref(br: &BlockRecord) -> bool {
    br.flags.is_xref || br.flags.is_xref_overlay || !br.xref_path.is_empty()
}

/// Lit tous les `.dwg` du ZIP (extension sans tenir compte de la casse), par chemin normalisé
/// (séparateur `/`) ; un dessin illisible est rangé à part avec sa cause, sans faire échouer la lecture.
pub fn charger_dessins(zip: &[u8]) -> Result<Dessins, ErreurCad> {
    let mut archive = ouvrir(zip)?;
    let mut dessins = Dessins::default();
    for i in 0..archive.len() {
        let chemin = {
            let entree = archive.by_index_raw(i).map_err(|e| ErreurCad::Zip(e.to_string()))?;
            if entree.is_dir() {
                continue;
            }
            normaliser(&nom_de(&entree)?)
        };
        if !chemin.rsplit_once('.').is_some_and(|(_, e)| e.eq_ignore_ascii_case("dwg")) {
            continue;
        }
        let mut octets = Vec::new();
        let lu = match archive.by_index(i).map(|mut e| e.read_to_end(&mut octets)) {
            Ok(Ok(_)) => lire_document(&octets).map_err(|e| match e {
                ErreurCad::Dwg(m) => m,
                autre => autre.to_string(),
            }),
            Ok(Err(e)) => Err(format!("archive : {e}")),
            Err(e) => Err(format!("archive : {e}")),
        };
        match lu {
            Ok(doc) => {
                dessins.illisibles.remove(&chemin);
                dessins.docs.insert(chemin, doc);
            }
            Err(m) => {
                dessins.docs.remove(&chemin);
                dessins.illisibles.insert(chemin, m);
            }
        }
    }
    Ok(dessins)
}

/// Dossier d'un chemin normalisé (`""` à la racine de l'archive).
fn dossier(chemin: &str) -> &str {
    chemin.rsplit_once('/').map_or("", |(d, _)| d)
}

/// `base` suivi des segments `rel` (`.` ignoré, `..` remonte) ; `None` au-dessus de la racine.
fn joindre(base: &str, rel: &[&str]) -> Option<String> {
    let mut v: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for s in rel {
        match *s {
            "" | "." => {}
            ".." => {
                v.pop()?;
            }
            s => v.push(s),
        }
    }
    Some(v.join("/"))
}

/// Nombre de dossiers de tête communs à deux dossiers.
fn tete_commune(a: &str, b: &str) -> usize {
    a.split('/').zip(b.split('/')).take_while(|(x, y)| !x.is_empty() && x.eq_ignore_ascii_case(y)).count()
}

/// Chemin dans l'archive d'une xréf enregistrée sous `chemin_enregistre` par le dessin `depuis`,
/// casse ignorée, dans l'ordre :
/// 1. le chemin enregistré normalisé (séparateur `/`) ;
/// 2. le chemin relatif au dossier de l'hôte : résolu tel quel s'il est relatif, puis ses fins de
///    chemin (lecteur et racine ôtés), de la plus longue à la plus courte, depuis le dossier de
///    l'hôte puis chacun de ses parents (un eTransmit garde l'arborescence sous une racine commune) ;
/// 3. le nom de fichier seul, n'importe où ; entre homonymes, le plus proche du dossier de l'hôte.
pub fn resoudre_chemin(chemin_enregistre: &str, depuis: &str, chemins: &[String]) -> Option<String> {
    let enregistre = normaliser(chemin_enregistre.trim());
    let minuscules: Vec<String> = chemins.iter().map(|c| c.to_lowercase()).collect();
    let trouver = |c: &str| {
        let c = c.to_lowercase();
        minuscules.iter().position(|m| *m == c).map(|i| chemins[i].clone())
    };
    // 1. Chemin enregistré.
    if let Some(c) = trouver(enregistre.trim_start_matches("./")) {
        return Some(c);
    }
    // 2. Relatif au dossier de l'hôte.
    let morceaux: Vec<&str> = enregistre.split('/').collect();
    let absolu = enregistre.starts_with('/') || morceaux[0].ends_with(':');
    let hote = dossier(depuis);
    if !absolu {
        if let Some(c) = joindre(hote, &morceaux).and_then(|c| trouver(&c)) {
            return Some(c);
        }
    }
    let utiles: Vec<&str> =
        morceaux.iter().copied().filter(|s| !matches!(*s, "" | "." | "..") && !s.ends_with(':')).collect();
    let (mut dossiers, mut d) = (vec![hote], hote);
    while !d.is_empty() {
        d = dossier(d);
        dossiers.push(d);
    }
    for k in 0..utiles.len() {
        for d in &dossiers {
            if let Some(c) = joindre(d, &utiles[k..]).and_then(|c| trouver(&c)) {
                return Some(c);
            }
        }
    }
    // 3. Nom de fichier seul.
    let nom = utiles.last()?.to_lowercase();
    (0..chemins.len())
        .filter(|&i| minuscules[i].rsplit('/').next() == Some(nom.as_str()))
        .min_by_key(|&i| (Reverse(tete_commune(hote, dossier(&chemins[i]))), &chemins[i]))
        .map(|i| chemins[i].clone())
}

/// Dessins lisibles qu'aucun autre ne référence comme xréf, triés : les dessins maîtres possibles.
pub fn trouver_maitres(dessins: &Dessins) -> Vec<String> {
    let chemins = dessins.chemins();
    let mut references = BTreeSet::new();
    for (hote, doc) in &dessins.docs {
        for br in doc.block_records.iter().filter(|br| est_xref(br)) {
            match resoudre_chemin(&br.xref_path, hote, &chemins) {
                Some(c) if c != *hote => {
                    references.insert(c);
                }
                _ => {}
            }
        }
    }
    dessins.docs.keys().filter(|c| !references.contains(*c)).cloned().collect()
}

/// Importe un ZIP eTransmit : `maitre` désigne le dessin à ouvrir ; sans lui, le seul dessin
/// maître possible (si tous se référencent en boucle, tous le sont), sinon `PlusieursMaitres`.
pub fn importer_zip(zip: &[u8], maitre: Option<&str>) -> Result<Aplatissement, ErreurCad> {
    let dessins = charger_dessins(zip)?;
    if dessins.docs.is_empty() && dessins.illisibles.is_empty() {
        return Err(ErreurCad::AucunDwg);
    }
    let maitre = match maitre {
        Some(m) => m.to_string(),
        // Aucun dessin lisible : l'aplatissement du premier en donne la cause.
        None if dessins.docs.is_empty() => dessins.illisibles.keys().next().cloned().unwrap_or_default(),
        None => {
            let mut candidats = trouver_maitres(&dessins);
            if candidats.is_empty() {
                candidats = dessins.docs.keys().cloned().collect();
            }
            if candidats.len() > 1 {
                return Err(ErreurCad::PlusieursMaitres(candidats));
            }
            candidats.remove(0)
        }
    };
    aplatir(&dessins, &maitre)
}
