use thiserror::Error;

/// Erreurs du moteur CAD.
#[derive(Debug, Error)]
pub enum ErreurCad {
    #[error("archive ZIP illisible : {0}")]
    Zip(String),
    #[error("DWG illisible : {0}")]
    Dwg(String),
    #[error("entrée absente de l'archive : {0}")]
    EntreeAbsente(String),
}
