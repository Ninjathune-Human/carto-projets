//! Reconnaissance du système de coordonnées d'un plan et conversion vers Lambert 93.

use std::sync::OnceLock;

use proj4rs::nadgrids::{catalog, formats};
use proj4rs::proj::Proj;
use proj4rs::transform::transform;

use crate::{err, ErreurGeoref, R};

/// Grille NTF vers RGF93 (IGN, Licence Ouverte Etalab 2.0, via OSGeo/PROJ-data).
const GRILLE: &[u8] = include_bytes!("../data/fr_ign_ntf_r93.tif");
const CLE_GRILLE: &str = "fr_ign_ntf_r93.tif";

const L93: &str = "+proj=lcc +lat_0=46.5 +lon_0=3 +lat_1=49 +lat_2=44 +x_0=700000 +y_0=6600000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs";
const CC43: &str = "+proj=lcc +lat_0=43 +lon_0=3 +lat_1=42.25 +lat_2=43.75 +x_0=1700000 +y_0=2200000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs";
const CC44: &str = "+proj=lcc +lat_0=44 +lon_0=3 +lat_1=43.25 +lat_2=44.75 +x_0=1700000 +y_0=3200000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs";
const L2E: &str = "+proj=lcc +lat_1=46.8 +lat_0=46.8 +lon_0=0 +k_0=0.99987742 +x_0=600000 +y_0=2200000 +a=6378249.2 +rf=293.4660212936269 +pm=paris +nadgrids=fr_ign_ntf_r93.tif +units=m +no_defs";
const L3S: &str = "+proj=lcc +lat_1=44.1 +lat_0=44.1 +lon_0=0 +k_0=0.999877499 +x_0=600000 +y_0=200000 +a=6378249.2 +rf=293.4660212936269 +pm=paris +nadgrids=fr_ign_ntf_r93.tif +units=m +no_defs";
const L3C: &str = "+proj=lcc +lat_1=44.1 +lat_0=44.1 +lon_0=0 +k_0=0.999877499 +x_0=600000 +y_0=3200000 +a=6378249.2 +rf=293.4660212936269 +pm=paris +nadgrids=fr_ign_ntf_r93.tif +units=m +no_defs";
const GEO: &str = "+proj=longlat +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +no_defs";

/// Emprise de la Métropole en WGS84 : lon min, lat min, lon max, lat max (degrés).
const METROPOLE: [f64; 4] = [6.80, 43.60, 7.50, 44.40];

/// Facteurs unités du dessin vers mètres essayés par la reconnaissance.
const FACTEURS: [f64; 3] = [1.0, 0.01, 0.001];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Systeme {
    Lambert93,
    Cc43,
    Cc44,
    LambertIIEtendu,
    LambertIIISud,
    LambertIIICarto,
}

impl Systeme {
    pub const TOUS: [Systeme; 6] = [
        Systeme::Lambert93,
        Systeme::Cc43,
        Systeme::Cc44,
        Systeme::LambertIIEtendu,
        Systeme::LambertIIISud,
        Systeme::LambertIIICarto,
    ];

    pub fn nom(&self) -> &'static str {
        match self {
            Systeme::Lambert93 => "Lambert 93",
            Systeme::Cc43 => "CC43",
            Systeme::Cc44 => "CC44",
            Systeme::LambertIIEtendu => "Lambert II étendu",
            Systeme::LambertIIISud => "Lambert III Sud",
            Systeme::LambertIIICarto => "Lambert III carto",
        }
    }

    fn indice(&self) -> usize {
        Self::TOUS.iter().position(|s| s == self).unwrap_or(0)
    }

    fn definition(&self) -> &'static str {
        match self {
            Systeme::Lambert93 => L93,
            Systeme::Cc43 => CC43,
            Systeme::Cc44 => CC44,
            Systeme::LambertIIEtendu => L2E,
            Systeme::LambertIIISud => L3S,
            Systeme::LambertIIICarto => L3C,
        }
    }
}

/// Système reconnu et facteur unités du dessin vers mètres (1, 0.01 ou 0.001).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reconnaissance {
    pub systeme: Systeme,
    pub facteur: f64,
}

/// Enregistre la grille NTF vers RGF93 dans le catalogue de proj4rs, une seule fois.
fn enregistrer_grille() -> Result<(), ErreurGeoref> {
    static ETAT: OnceLock<Result<(), String>> = OnceLock::new();
    ETAT.get_or_init(|| {
        catalog::with_catalog(|c| {
            formats::read(c, CLE_GRILLE, &mut std::io::Cursor::new(GRILLE)).map_err(|e| e.to_string())
        })
    })
    .clone()
    .map_err(ErreurGeoref::Projection)
}

fn projection(def: &str) -> Result<Proj, ErreurGeoref> {
    enregistrer_grille()?;
    Proj::from_proj_string(def).map_err(err)
}

/// Rectangle de la Métropole projeté dans chaque système (calculé une fois).
fn rectangles() -> &'static [[f64; 4]; 6] {
    static RECTS: OnceLock<[[f64; 4]; 6]> = OnceLock::new();
    RECTS.get_or_init(|| {
        let geo = projection(GEO).expect("projection géographique");
        let mut sortie = [[0.0; 4]; 6];
        for s in Systeme::TOUS {
            let proj = projection(s.definition()).expect("projection du système");
            let mut r = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
            for i in 0..3 {
                for j in 0..3 {
                    let lon = METROPOLE[0] + (METROPOLE[2] - METROPOLE[0]) * i as f64 / 2.0;
                    let lat = METROPOLE[1] + (METROPOLE[3] - METROPOLE[1]) * j as f64 / 2.0;
                    let mut pt = (lon.to_radians(), lat.to_radians(), 0.0);
                    transform(&geo, &proj, &mut pt).expect("projection de la Métropole");
                    r[0] = r[0].min(pt.0);
                    r[1] = r[1].min(pt.1);
                    r[2] = r[2].max(pt.0);
                    r[3] = r[3].max(pt.1);
                }
            }
            sortie[s.indice()] = r;
        }
        sortie
    })
}

/// Rang `p` (0 à 100) au sens du plus proche rang dans un tableau trié non vide.
fn rang(tries: &[f64], p: f64) -> f64 {
    let n = tries.len();
    let k = ((p / 100.0 * n as f64).ceil() as usize).clamp(1, n);
    tries[k - 1]
}

/// Emprise `[xmin, ymin, xmax, ymax]` entre les percentiles 2 et 98 de x et de y.
/// Écarte les objets égarés ; `None` si aucun point fini.
pub fn emprise_essentielle(points: &[[f64; 2]]) -> Option<[f64; 4]> {
    let mut xs: Vec<f64> = points.iter().map(|p| p[0]).filter(|v| v.is_finite()).collect();
    let mut ys: Vec<f64> = points.iter().map(|p| p[1]).filter(|v| v.is_finite()).collect();
    if xs.is_empty() || ys.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    ys.sort_by(|a, b| a.total_cmp(b));
    Some([rang(&xs, 2.0), rang(&ys, 2.0), rang(&xs, 98.0), rang(&ys, 98.0)])
}

/// Premier système, puis premier facteur, dont l'emprise de la Métropole contient
/// `emprise × facteur` (`[xmin, ymin, xmax, ymax]` en unités du dessin).
pub fn reconnaitre(emprise: [f64; 4]) -> Option<Reconnaissance> {
    if emprise.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let rects = rectangles();
    for systeme in Systeme::TOUS {
        let r = rects[systeme.indice()];
        for facteur in FACTEURS {
            let e = emprise.map(|v| v * facteur);
            if e[0] >= r[0] && e[1] >= r[1] && e[2] <= r[2] && e[3] <= r[3] {
                return Some(Reconnaissance { systeme, facteur });
            }
        }
    }
    None
}

/// Part minimale des primitives qui doivent voter pour un couple (système, facteur).
const PART_MIN: f64 = 0.1;

/// Reconnaissance par vote : chaque primitive (suite de points, unités du dessin) vote par son
/// premier point fini pour le couple (système, facteur) dont le rectangle de la Métropole le
/// contient. Le couple qui a le plus de voix (premier dans l'ordre des essais en cas d'égalité)
/// l'emporte s'il en a au moins 10 % ; un cartouche ou des détails en coordonnées locales ne
/// font donc pas refuser un plan géoréférencé, quel que soit leur nombre de points.
/// Renvoie le couple, l'emprise essentielle des seuls points contenus dans son rectangle (unités
/// du dessin) et la part des primitives votantes (0 à 1) qui l'ont choisi.
pub fn reconnaitre_primitives(primitives: &[&[[f64; 2]]]) -> Option<(Reconnaissance, [f64; 4], f64)> {
    let fini = |p: &&[f64; 2]| p[0].is_finite() && p[1].is_finite();
    let voix: Vec<[f64; 2]> = primitives.iter().filter_map(|v| v.iter().find(fini).copied()).collect();
    if voix.is_empty() {
        return None;
    }
    let rects = rectangles();
    let dedans = |r: &[f64; 4], f: f64, p: &[f64; 2]| {
        let (x, y) = (p[0] * f, p[1] * f);
        x >= r[0] && y >= r[1] && x <= r[2] && y <= r[3]
    };
    let mut meilleur: Option<(Reconnaissance, usize)> = None;
    for systeme in Systeme::TOUS {
        let r = &rects[systeme.indice()];
        for facteur in FACTEURS {
            let n = voix.iter().filter(|p| dedans(r, facteur, p)).count();
            if n > meilleur.map_or(0, |(_, m)| m) {
                meilleur = Some((Reconnaissance { systeme, facteur }, n));
            }
        }
    }
    let (reco, n) = meilleur?;
    let part = n as f64 / voix.len() as f64;
    if part < PART_MIN {
        return None;
    }
    let r = &rects[reco.systeme.indice()];
    let contenus: Vec<[f64; 2]> =
        primitives.iter().flat_map(|v| v.iter()).filter(fini).filter(|p| dedans(r, reco.facteur, p)).copied().collect();
    Some((reco, emprise_essentielle(&contenus)?, part))
}

/// Convertisseur d'un système vers Lambert 93 et Web Mercator ; projections analysées une fois.
pub struct Convertisseur {
    source: Proj,
    l93: Proj,
    geo: Proj,
    identite: bool,
}

impl Convertisseur {
    pub fn nouveau(r: Reconnaissance) -> Result<Self, ErreurGeoref> {
        Ok(Self {
            source: projection(r.systeme.definition())?,
            l93: projection(L93)?,
            geo: projection(GEO)?,
            identite: r.systeme == Systeme::Lambert93,
        })
    }

    /// Coordonnées du système (m) vers Lambert 93 (m).
    pub fn vers_l93(&self, x: f64, y: f64) -> Result<(f64, f64), ErreurGeoref> {
        if self.identite {
            return Ok((x, y));
        }
        let mut pt = (x, y, 0.0);
        transform(&self.source, &self.l93, &mut pt).map_err(err)?;
        Ok((pt.0, pt.1))
    }

    /// Coordonnées du système (m) vers Web Mercator (m).
    pub fn vers_mercator(&self, x: f64, y: f64) -> Result<(f64, f64), ErreurGeoref> {
        let mut pt = (x, y, 0.0);
        transform(&self.source, &self.geo, &mut pt).map_err(err)?;
        let (lambda, phi) = (pt.0, pt.1);
        Ok((R * lambda, R * (std::f64::consts::FRAC_PI_4 + phi / 2.0).tan().ln()))
    }
}
