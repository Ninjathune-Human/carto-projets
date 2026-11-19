//! Crate wasm de carto-projets : mesure des DWG d'un ZIP eTransmit dans le navigateur.

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

/// Mesure chaque DWG du ZIP et renvoie un tableau JSON, un élément par DWG :
/// `{"chemin","duree_ms","mesure"}` ou `{"chemin","duree_ms","erreur"}`.
/// Un ZIP dont l'inventaire échoue donne `Err(message)`.
pub fn mesurer_zip_json(octets: &[u8]) -> Result<String, String> {
    let inventaire = inventorier(octets).map_err(|e| e.to_string())?;
    let elements: Vec<Element> = inventaire
        .dwg
        .into_iter()
        .map(|chemin| {
            let mut duree_ms = 0.0;
            let resultat = match lire_entree(octets, &chemin) {
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
            Element { chemin, duree_ms, resultat }
        })
        .collect();
    serde_json::to_string(&elements).map_err(|e| e.to_string())
}

#[wasm_bindgen]
pub fn mesurer_zip(octets: &[u8]) -> Result<String, JsError> {
    mesurer_zip_json(octets).map_err(|m| JsError::new(&m))
}
