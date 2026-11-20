//! Crate georef de carto-projets : passage du Lambert 93 au Web Mercator.

mod systemes;

pub use systemes::{emprise_essentielle, reconnaitre, Convertisseur, Reconnaissance, Systeme};

use proj4rs::proj::Proj;
use proj4rs::transform::transform;

/// Rayon de la sphère Web Mercator (EPSG:3857), en mètres.
const R: f64 = 6_378_137.0;

const L93: &str = "+proj=lcc +lat_0=46.5 +lon_0=3 +lat_1=49 +lat_2=44 +x_0=700000 +y_0=6600000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs";
const GEO_GRS80: &str = "+proj=longlat +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +no_defs";

#[derive(Debug, thiserror::Error)]
pub enum ErreurGeoref {
    #[error("erreur de projection : {0}")]
    Projection(String),
}

fn err<E: std::fmt::Display>(e: E) -> ErreurGeoref {
    ErreurGeoref::Projection(e.to_string())
}

/// Convertit des coordonnées Lambert 93 (m) en Web Mercator (m).
pub fn l93_vers_mercator(x: f64, y: f64) -> Result<(f64, f64), ErreurGeoref> {
    let src = Proj::from_proj_string(L93).map_err(err)?;
    let dst = Proj::from_proj_string(GEO_GRS80).map_err(err)?;
    let mut pt = (x, y, 0.0);
    transform(&src, &dst, &mut pt).map_err(err)?;
    // proj4rs fournit des radians pour une projection longlat.
    let (lambda, phi) = (pt.0, pt.1);
    Ok((R * lambda, R * (std::f64::consts::FRAC_PI_4 + phi / 2.0).tan().ln()))
}
