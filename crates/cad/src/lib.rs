//! Crate cad de carto-projets.

pub mod aplatir;
pub mod decoupe;
pub mod erreur;
pub mod geometrie;
pub mod hachures;
pub mod inventaire;
pub mod lignes;
pub mod mesure;
pub mod proprietes;
pub mod scene;

pub use aplatir::{aplatir, Aplatissement, Dessins};
pub use erreur::ErreurCad;
pub use inventaire::{inventorier, lire_entree, Inventaire};
pub use lignes::{ecrire_lignes, extraire_lignes, Polyligne};
pub use mesure::{emprise_depuis, lire_document, mesurer, Mesure};
pub use scene::{ecrire_scene, Calque, Primitive, Rapport, Scene};
pub use geometrie::{arc_points, bande, ellipse_points, renflement_points, spline_points, trianguler};
