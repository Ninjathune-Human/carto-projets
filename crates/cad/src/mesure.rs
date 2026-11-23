//! Mesure d'un DWG : comptage des entités, emprise, géoréférencement.
use crate::ErreurCad;
use acadrust::objects::ObjectType;
use acadrust::{CadDocument, DwgReader, EntityType};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Résultat de la mesure d'un DWG.
#[derive(Debug, Clone, Serialize)]
pub struct Mesure {
    pub version: String,
    pub par_type: BTreeMap<String, u64>,
    pub total: u64,
    pub inconnues: u64,
    pub insunits: i16,
    pub emprise: Option<[f64; 4]>,
    pub geodata: bool,
    pub erreurs_recuperees: usize,
}

fn message_de_panique(p: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "cause inconnue".to_string()
    }
}

fn lire(octets: &[u8]) -> Result<acadrust::ReadOutcome, ErreurCad> {
    let resultat = catch_unwind(AssertUnwindSafe(|| {
        DwgReader::from_stream(Cursor::new(octets)).read_with_stats()
    }));
    match resultat {
        Ok(Ok(sortie)) => Ok(sortie),
        Ok(Err(e)) => Err(ErreurCad::Dwg(e.to_string())),
        Err(p) => Err(ErreurCad::Dwg(format!("panique du lecteur : {}", message_de_panique(p)))),
    }
}

/// Lit un DWG en mémoire.
pub fn lire_document(octets: &[u8]) -> Result<CadDocument, ErreurCad> {
    lire(octets).map(|s| s.document)
}

/// Emprise `[xmin, ymin, xmax, ymax]`, ou `None` si l'en-tête n'est pas renseigné.
pub fn emprise_depuis(min: [f64; 2], max: [f64; 2]) -> Option<[f64; 4]> {
    let valeurs = [min[0], min[1], max[0], max[1]];
    if valeurs.iter().any(|v| !v.is_finite() || v.abs() >= 1e19) || min[0] > max[0] || min[1] > max[1] {
        return None;
    }
    Some(valeurs)
}

/// Nom de type d'une entité tel que la mesure le compte (`?NOM` pour une entité inconnue).
pub(crate) fn nom_de_type(e: &EntityType) -> String {
    match e {
        EntityType::Unknown(u) => format!("?{}", u.dxf_name),
        autre => autre.as_entity().entity_type().to_string(),
    }
}

/// Mesure un DWG.
pub fn mesurer(octets: &[u8]) -> Result<Mesure, ErreurCad> {
    let sortie = lire(octets)?;
    let doc = sortie.document;
    let mut par_type: BTreeMap<String, u64> = BTreeMap::new();
    let (mut total, mut inconnues) = (0u64, 0u64);
    for e in doc.model_space_entities() {
        if matches!(e, EntityType::Unknown(_)) {
            inconnues += 1;
        }
        let nom = nom_de_type(e);
        if matches!(nom.as_str(), "ATTRIB" | "SEQEND" | "VERTEX") {
            continue;
        }
        *par_type.entry(nom).or_insert(0) += 1;
        total += 1;
    }
    let h = &doc.header;
    let emprise = emprise_depuis(
        [h.model_space_extents_min.x, h.model_space_extents_min.y],
        [h.model_space_extents_max.x, h.model_space_extents_max.y],
    );
    Ok(Mesure {
        version: sortie.stats.source_version,
        par_type,
        total,
        inconnues,
        insunits: h.insertion_units,
        emprise,
        geodata: doc.objects.values().any(|o| matches!(o, ObjectType::GeoData(_))),
        erreurs_recuperees: sortie.stats.recovered_errors,
    })
}
