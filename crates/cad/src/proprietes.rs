//! Propriétés héritées (calque, couleur, épaisseur, transparence) et visibilité des calques.

use acadrust::entities::EntityCommon;
use acadrust::tables::Layer;
use acadrust::types::aci_to_rgb;
use acadrust::{CadDocument, Color, LineWeight, Transparency};

/// Propriétés effectives d'une entité après résolution des héritages.
#[derive(Clone, Debug, PartialEq)]
pub struct Herite {
    pub calque: String,
    pub couleur: [u8; 3],
    pub epaisseur_mm: f32,
    pub alpha: u8,
}

const NOIR: [u8; 3] = [0, 0, 0];

/// Couleur RVB d'un indice ACI ; l'indice 7 (blanc/noir) et les indices inconnus donnent du noir.
pub fn couleur_aci(index: u8) -> [u8; 3] {
    if index == 7 {
        return NOIR;
    }
    aci_to_rgb(index).map_or(NOIR, |(r, g, b)| [r, g, b])
}

fn epaisseur_mm(p: LineWeight) -> Option<f32> {
    match p {
        LineWeight::Value(n) => Some(f32::from(n.max(0)) / 100.0),
        LineWeight::Default => Some(0.0),
        LineWeight::ByLayer | LineWeight::ByBlock => None,
    }
}

fn alpha_de(t: Transparency) -> Option<u8> {
    // Explicit(n) : 0 opaque, 255 transparent ; transparence = n / 255.
    t.explicit_alpha().map(|n| (255.0 * (1.0 - f64::from(n) / 255.0)).round() as u8)
}

fn couleur_propre(c: Color) -> Option<[u8; 3]> {
    match c {
        Color::Index(n) => Some(couleur_aci(n)),
        Color::Rgb { r, g, b } => Some([r, g, b]),
        Color::None => Some(NOIR),
        Color::ByLayer | Color::ByBlock => None,
    }
}

pub(crate) fn couleur_calque(c: Option<&Layer>) -> [u8; 3] {
    c.and_then(|l| couleur_propre(l.color)).unwrap_or(NOIR)
}

/// Résout les propriétés d'une entité ; `parent` est l'insertion englobante, s'il y en a une.
pub fn resoudre(doc: &CadDocument, e: &EntityCommon, parent: Option<&Herite>) -> Herite {
    let calque = match parent {
        Some(p) if e.layer == "0" => p.calque.clone(),
        _ => e.layer.clone(),
    };
    let couche = doc.layers.get(&calque);
    resoudre_avec(e, parent, calque, couche)
}

/// Résout les propriétés d'une entité dont le calque effectif `calque` est décrit par `couche`.
pub(crate) fn resoudre_avec(e: &EntityCommon, parent: Option<&Herite>, calque: String, couche: Option<&Layer>) -> Herite {
    let couleur = match e.color {
        Color::ByLayer => couleur_calque(couche),
        Color::ByBlock => parent.map_or(NOIR, |p| p.couleur),
        c => couleur_propre(c).unwrap_or(NOIR),
    };

    let epaisseur_mm = match e.line_weight {
        LineWeight::ByLayer => couche.and_then(|l| epaisseur_mm(l.line_weight)).unwrap_or(0.0),
        LineWeight::ByBlock => parent.map_or(0.0, |p| p.epaisseur_mm),
        p => epaisseur_mm(p).unwrap_or(0.0),
    };

    let alpha = match e.transparency {
        Transparency::ByLayer => couche.and_then(|l| alpha_de(l.transparency)).unwrap_or(255),
        Transparency::ByBlock => parent.map_or(255, |p| p.alpha),
        t => alpha_de(t).unwrap_or(255),
    };

    Herite { calque, couleur, epaisseur_mm, alpha }
}

/// Faux si le calque est éteint, gelé, non imprimable ou nommé `Defpoints` ; vrai s'il est absent de la table.
pub fn calque_visible(doc: &CadDocument, nom: &str) -> bool {
    couche_visible(nom, doc.layers.get(nom))
}

/// Visibilité du calque `nom` décrit par `couche` (`None` : absent de toute table, donc visible).
pub(crate) fn couche_visible(nom: &str, couche: Option<&Layer>) -> bool {
    !nom.eq_ignore_ascii_case("Defpoints") && couche.map_or(true, |l| !l.is_off() && !l.is_frozen() && l.is_plottable)
}

/// Vrai seulement pour un calque gelé de la table.
pub fn calque_gele(doc: &CadDocument, nom: &str) -> bool {
    doc.layers.get(nom).is_some_and(|l| l.is_frozen())
}

/// Mètres par unité du dessin, d'après INSUNITS.
pub fn metres_par_unite(insunits: i16) -> f64 {
    match insunits {
        1 => 0.0254,
        2 => 0.3048,
        4 => 0.001,
        5 => 0.01,
        _ => 1.0,
    }
}
