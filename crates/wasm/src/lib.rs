//! Crate wasm de carto-projets : mesure des DWG d'un ZIP eTransmit dans le navigateur, un DWG par appel
//! (un piège wasm ne coûte ainsi que le DWG en cours, voir R15).

use carto_cad::{inventorier, lire_entree, mesurer, Mesure};
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
