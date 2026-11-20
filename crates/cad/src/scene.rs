//! Scène de primitives (traits et surfaces) et format binaire `SCN1`.
use serde::Serialize;
use std::collections::BTreeMap;

/// Calque de la scène : nom et couleur du calque.
#[derive(Debug, Clone, PartialEq)]
pub struct Calque {
    pub nom: String,
    pub couleur: [u8; 3],
}

/// Primitive affichable. Un trait garde ses points tels quels (la fermeture est l'affaire du producteur) ;
/// une surface est une suite de triangles (3 points chacun).
#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Trait { calque: u32, couleur: [u8; 4], epaisseur_mm: f32, points: Vec<[f64; 2]> },
    Surface { calque: u32, couleur: [u8; 4], triangles: Vec<[f64; 2]> },
}

/// Scène : calques puis primitives dans l'ordre d'affichage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub calques: Vec<Calque>,
    pub primitives: Vec<Primitive>,
}

/// Compte rendu de l'import, joint à la scène en JSON.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Rapport {
    pub maitre: String,
    pub dessins: Vec<String>,
    pub ignorees: BTreeMap<String, u64>,
    pub avertissements: Vec<String>,
    pub systeme: Option<String>,
    /// Emprise essentielle `[xmin, ymin, xmax, ymax]` dans le repère de la scène (objets égarés
    /// écartés), pour cadrer la vue ; `null` en JSON si elle n'est pas connue.
    pub emprise: Option<[f64; 4]>,
}

impl Scene {
    /// Indice du calque de ce nom, créé au besoin.
    pub fn calque(&mut self, nom: &str, couleur: [u8; 3]) -> u32 {
        if let Some(i) = self.calques.iter().position(|c| c.nom == nom) {
            return i as u32;
        }
        self.calques.push(Calque { nom: nom.to_string(), couleur });
        (self.calques.len() - 1) as u32
    }

    /// Tous les points de toutes les primitives.
    pub fn points_mut(&mut self) -> impl Iterator<Item = &mut [f64; 2]> {
        self.primitives.iter_mut().flat_map(|p| match p {
            Primitive::Trait { points, .. } => points.iter_mut(),
            Primitive::Surface { triangles, .. } => triangles.iter_mut(),
        })
    }
}

/// Sérialise la scène au format `SCN1` (petit-boutiste), rapport JSON en fin de fichier.
pub fn ecrire_scene(scene: &Scene, rapport: &Rapport) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(b"SCN1");
    o.extend_from_slice(&(scene.calques.len() as u32).to_le_bytes());
    for c in &scene.calques {
        o.extend_from_slice(&(c.nom.len() as u32).to_le_bytes());
        o.extend_from_slice(c.nom.as_bytes());
        o.extend_from_slice(&[c.couleur[0], c.couleur[1], c.couleur[2], 0]);
    }
    o.extend_from_slice(&(scene.primitives.len() as u32).to_le_bytes());
    for p in &scene.primitives {
        let (genre, calque, couleur, epaisseur, points) = match p {
            Primitive::Trait { calque, couleur, epaisseur_mm, points } => (0u8, *calque, *couleur, *epaisseur_mm, points),
            Primitive::Surface { calque, couleur, triangles } => (1u8, *calque, *couleur, 0.0f32, triangles),
        };
        o.push(genre);
        o.extend_from_slice(&couleur);
        o.extend_from_slice(&calque.to_le_bytes());
        o.extend_from_slice(&epaisseur.to_le_bytes());
        o.extend_from_slice(&(points.len() as u32).to_le_bytes());
        for q in points {
            o.extend_from_slice(&q[0].to_le_bytes());
            o.extend_from_slice(&q[1].to_le_bytes());
        }
    }
    let json = serde_json::to_vec(rapport).unwrap_or_else(|_| b"{}".to_vec());
    o.extend_from_slice(&(json.len() as u32).to_le_bytes());
    o.extend_from_slice(&json);
    o
}
