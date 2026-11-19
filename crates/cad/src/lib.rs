//! Crate cad de carto-projets.

pub mod erreur;
pub mod inventaire;

pub use erreur::ErreurCad;
pub use inventaire::{inventorier, lire_entree, Inventaire};
