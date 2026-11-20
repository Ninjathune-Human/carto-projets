//! Crate wasm de carto-projets : mesure des DWG d'un ZIP eTransmit dans le navigateur, un DWG par appel
//! (un piège wasm ne coûte ainsi que le DWG en cours, voir R15).

use carto_cad::{ecrire_scene, importer_zip as importer_zip_cad, inventorier, lire_entree, mesurer, ErreurCad, Mesure, Primitive};
use carto_georef::{reconnaitre_points, Convertisseur};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use web_time::Instant;

#[derive(Serialize)]
struct Element {
    chemin: String,
    duree_ms: f64,
    #[serde(flatten)]
    resultat: Resultat,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Resultat {
    Mesure(Mesure),
    Erreur(String),
}

/// Liste les DWG du ZIP (chemins normalisés, triés) : tableau JSON de chaînes.
/// Un ZIP illisible donne `Err(message)`.
pub fn lister_dwg_json(octets: &[u8]) -> Result<String, String> {
    let inventaire = inventorier(octets).map_err(|e| e.to_string())?;
    serde_json::to_string(&inventaire.dwg).map_err(|e| e.to_string())
}

/// Mesure un seul DWG du ZIP, désigné par son chemin normalisé, et renvoie un objet JSON :
/// `{"chemin","duree_ms","mesure"}` ou `{"chemin","duree_ms","erreur"}`.
/// Toute erreur propre à ce DWG (absent, illisible, ZIP illisible) devient un élément `erreur` :
/// la page passe alors au DWG suivant.
pub fn mesurer_dwg_json(octets: &[u8], chemin: &str) -> Result<String, String> {
    let mut duree_ms = 0.0;
    let resultat = match lire_entree(octets, chemin) {
        Ok(o) => {
            let debut = Instant::now();
            let r = mesurer(&o);
            duree_ms = debut.elapsed().as_secs_f64() * 1000.0;
            match r {
                Ok(m) => Resultat::Mesure(m),
                Err(e) => Resultat::Erreur(e.to_string()),
            }
        }
        Err(e) => Resultat::Erreur(e.to_string()),
    };
    serde_json::to_string(&Element { chemin: chemin.to_string(), duree_ms, resultat })
        .map_err(|e| e.to_string())
}

#[wasm_bindgen]
pub fn lister_dwg(octets: &[u8]) -> Result<String, JsError> {
    lister_dwg_json(octets).map_err(|m| JsError::new(&m))
}

#[wasm_bindgen]
pub fn mesurer_dwg(octets: &[u8], chemin: &str) -> Result<String, JsError> {
    mesurer_dwg_json(octets, chemin).map_err(|m| JsError::new(&m))
}

/// Repère de sortie de la scène importée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repere {
    L93,
    Mercator,
}

impl Repere {
    /// `"l93"` ou `"mercator"`.
    pub fn depuis(texte: &str) -> Option<Self> {
        match texte {
            "l93" => Some(Self::L93),
            "mercator" => Some(Self::Mercator),
            _ => None,
        }
    }
}

const MESSAGE_NON_GEOREFERENCE: &str =
    "Plan non géoréférencé : aucun système reconnu (Lambert 93, CC43, CC44, Lambert II étendu, Lambert III Sud ou carto).";

fn erreur_json(code: &str, message: &str) -> String {
    serde_json::json!({ "code": code, "message": message }).to_string()
}

fn lecture(message: impl std::fmt::Display) -> String {
    erreur_json("lecture", &message.to_string())
}

/// Importe un ZIP eTransmit : aplatissement, reconnaissance du système sur l'emprise essentielle,
/// conversion de chaque point (dans le repère demandé) puis écriture SCN1.
/// L'erreur est une chaîne JSON `{"code", …}`.
pub fn importer_zip_scn(octets: &[u8], repere: Repere, maitre: Option<&str>) -> Result<Vec<u8>, String> {
    let mut a = importer_zip_cad(octets, maitre).map_err(|e| match e {
        ErreurCad::AucunDwg => serde_json::json!({ "code": "aucun_dwg" }).to_string(),
        ErreurCad::PlusieursMaitres(c) => serde_json::json!({ "code": "plusieurs_maitres", "candidats": c }).to_string(),
        e => lecture(e),
    })?;
    let points: Vec<[f64; 2]> = a.scene.points_mut().map(|p| *p).collect();
    let (reco, _emprise) = reconnaitre_points(&points)
        .ok_or_else(|| erreur_json("non_georeference", MESSAGE_NON_GEOREFERENCE))?;
    let facteur = reco.facteur;
    let conv = Convertisseur::nouveau(reco).map_err(lecture)?;
    a.rapport.systeme = Some(reco.systeme.nom().to_string());

    let mut perdus = 0usize;
    for p in a.scene.points_mut() {
        let (x, y) = (p[0] * facteur, p[1] * facteur);
        let r = match repere {
            Repere::L93 => conv.vers_l93(x, y),
            Repere::Mercator => conv.vers_mercator(x, y),
        };
        match r {
            Ok((x, y)) if x.is_finite() && y.is_finite() => *p = [x, y],
            _ => {
                *p = [f64::NAN, f64::NAN];
                perdus += 1;
            }
        }
    }
    if perdus > 0 {
        a.scene.primitives = std::mem::take(&mut a.scene.primitives)
            .into_iter()
            .filter_map(|prim| match prim {
                Primitive::Trait { calque, couleur, epaisseur_mm, points } => {
                    let points: Vec<[f64; 2]> = points.into_iter().filter(|p| p[0].is_finite()).collect();
                    (points.len() >= 2).then_some(Primitive::Trait { calque, couleur, epaisseur_mm, points })
                }
                Primitive::Surface { calque, couleur, triangles } => {
                    let triangles: Vec<[f64; 2]> = triangles
                        .chunks(3)
                        .filter(|t| t.len() == 3 && t.iter().all(|p| p[0].is_finite()))
                        .flatten()
                        .copied()
                        .collect();
                    (!triangles.is_empty()).then_some(Primitive::Surface { calque, couleur, triangles })
                }
            })
            .collect();
        a.rapport.avertissements.push(format!("{perdus} points hors de la grille de conversion"));
    }
    Ok(ecrire_scene(&a.scene, &a.rapport))
}

#[wasm_bindgen]
pub fn importer_zip(octets: &[u8], repere: &str, maitre: Option<String>) -> Result<Vec<u8>, JsError> {
    let repere = Repere::depuis(repere).ok_or_else(|| JsError::new(&lecture(format!("repère inconnu : {repere}"))))?;
    importer_zip_scn(octets, repere, maitre.as_deref()).map_err(|m| JsError::new(&m))
}
