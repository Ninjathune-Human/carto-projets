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
    #[error("dessin maître inconnu : {0}")]
    MaitreInconnu(String),
    #[error("aucun dessin DWG dans l'archive")]
    AucunDwg,
    #[error("plusieurs dessins maîtres possibles : {}", .0.join(", "))]
    PlusieursMaitres(Vec<String>),
}
