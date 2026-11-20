//! Découpe XCLIP : le contour rattaché à une insertion, ramené dans le repère du bloc, devient une
//! zone visible dans le repère du dessin maître, qui coupe les traits et les surfaces du bloc. Les
//! découpes imbriquées se composent par intersection.

use crate::geometrie::trianguler;
use acadrust::entities::Insert;
use acadrust::objects::ObjectType;
use acadrust::types::Matrix4;
use acadrust::{CadDocument, Handle, Vector3};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::clip::FloatClip;
use i_overlay::float::overlay::FloatOverlay;
use i_overlay::string::clip::ClipRule;

/// Un trait posé sur le contour reste visible.
const BORD_INCLUS: ClipRule = ClipRule { invert: false, boundary_included: true };

/// Au-delà, une coordonnée est aberrante (aucun dessin réel n'en approche) : la découpe l'écarte,
/// i_overlay refusant les coordonnées de plus de 2^500 en paniquant.
const COORDONNEE_MAX: f64 = 1e15;

/// Contour d'un XCLIP dans son propre repère : deux points sont les coins opposés d'un rectangle
/// aux côtés parallèles aux axes de ce repère ; trois points ou plus, un polygone.
pub fn contour_xclip(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    match points {
        [a, b] => vec![*a, [b[0], a[1]], *b, [a[0], b[1]]],
        _ => points.to_vec(),
    }
}

/// Inverse d'une transformation affine (partie linéaire 3 × 3 et translation) ; `None` si singulière.
fn inverse_affine(m: &Matrix4) -> Option<Matrix4> {
    let l = m.to_matrix3().inverse()?;
    let t = l.transform_point(Vector3::new(m.m[0][3], m.m[1][3], m.m[2][3]));
    let mut inv = Matrix4::from_matrix3(l);
    for (k, v) in [-t.x, -t.y, -t.z].into_iter().enumerate() {
        inv.m[k][3] = v;
    }
    Some(inv)
}

/// Contour XCLIP actif d'une insertion, dans le repère de définition du bloc : dictionnaire
/// d'extension de l'INSERT → `ACAD_FILTER` → `SPATIAL`. La matrice du contour porte les points vers
/// le repère du contour : son inverse en ramène les sommets, puis l'inverse de la transformation du
/// bloc au moment de la découpe les place dans le bloc (`IBT · CBT⁻¹ · p`). Le sens inversé n'est
/// pas lu par acadrust.
pub fn xclip_de(doc: &CadDocument, i: &Insert) -> Option<Vec<[f64; 2]>> {
    let entree = |dico: Handle, cle: &str| match doc.objects.get(&dico)? {
        ObjectType::Dictionary(d) => d.get(cle),
        _ => None,
    };
    let h = entree(entree(i.common.xdictionary_handle?, "ACAD_FILTER")?, "SPATIAL")?;
    let Some(ObjectType::SpatialFilter(f)) = doc.objects.get(&h) else { return None };
    if !f.display_enabled || f.boundary_points.len() < 2 {
        return None;
    }
    let m = f.inverse_block_transform * inverse_affine(&f.clip_bound_transform)?;
    let local: Vec<[f64; 2]> = f.boundary_points.iter().map(|p| [p.x, p.y]).collect();
    let bloc = contour_xclip(&local).into_iter().map(|p| m.transform_point(Vector3::new(p[0], p[1], 0.)));
    Some(bloc.map(|q| [q.x, q.y]).collect())
}

/// Zone visible : contours bien formés (extérieurs dans le sens trigonométrique, trous dans le sens
/// horaire, sans recouvrement), vide quand des découpes imbriquées ne se recoupent pas.
#[derive(Clone, Debug)]
pub struct Zone {
    contours: Vec<Vec<[f64; 2]>>,
    min: [f64; 2],
    max: [f64; 2],
    /// Contour unique et convexe (cas du rectangle) : ce qui est dedans est gardé tel quel.
    convexe: bool,
}

/// Coordonnées finies et non aberrantes (faux pour NaN et les infinis).
fn fini(p: &[f64; 2]) -> bool {
    p[0].abs() <= COORDONNEE_MAX && p[1].abs() <= COORDONNEE_MAX
}

fn emprise(points: impl Iterator<Item = [f64; 2]>) -> ([f64; 2], [f64; 2]) {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for p in points {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    (min, max)
}

fn vectoriel(o: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

impl Zone {
    /// Zone intérieure (pair-impair) d'un contour fermé implicitement ; `None` s'il n'enclôt
    /// aucune aire ou a un sommet aberrant : la découpe est alors sans effet.
    pub fn nouvelle(contour: &[[f64; 2]]) -> Option<Zone> {
        if contour.len() < 3 || !contour.iter().all(fini) {
            return None;
        }
        let formes = FloatOverlay::<[f64; 2], i64>::from_subj(contour).overlay(OverlayRule::Subject, FillRule::EvenOdd);
        let zone = Zone::depuis(formes.into_iter().flatten().collect());
        (!zone.est_vide()).then_some(zone)
    }

    fn depuis(contours: Vec<Vec<[f64; 2]>>) -> Zone {
        let (min, max) = emprise(contours.iter().flatten().copied());
        let convexe = match contours.as_slice() {
            [c] => {
                let n = c.len();
                let eps = 1e-12 * ((max[0] - min[0]).powi(2) + (max[1] - min[1]).powi(2));
                (0..n).all(|i| vectoriel(c[i], c[(i + 1) % n], c[(i + 2) % n]) >= -eps)
            }
            _ => false,
        };
        Zone { contours, min, max, convexe }
    }

    /// Plus rien n'est visible.
    pub fn est_vide(&self) -> bool {
        self.contours.is_empty()
    }

    /// La zone vue à travers la découpe des blocs englobants, s'il y en a une.
    pub fn dans(self, englobante: Option<&Zone>) -> Zone {
        match englobante {
            Some(e) => e.inter(&self),
            None => self,
        }
    }

    /// Partie commune de deux zones : découpe d'un bloc dans un bloc découpé.
    pub fn inter(&self, autre: &Zone) -> Zone {
        if !self.touche(autre.min, autre.max) {
            return Zone::depuis(Vec::new());
        }
        let formes = FloatOverlay::<[f64; 2], i64>::from_subj_and_clip(&self.contours, &autre.contours)
            .overlay(OverlayRule::Intersect, FillRule::EvenOdd);
        Zone::depuis(formes.into_iter().flatten().collect())
    }

    /// Les emprises se touchent (bords compris).
    fn touche(&self, min: [f64; 2], max: [f64; 2]) -> bool {
        min[0] <= self.max[0] && max[0] >= self.min[0] && min[1] <= self.max[1] && max[1] >= self.min[1]
    }

    /// Tous les points sont dans la zone convexe (bord compris) : rien à couper.
    fn contient(&self, points: &[[f64; 2]]) -> bool {
        if !self.convexe {
            return false;
        }
        let c = &self.contours[0];
        let tol = 1e-9 * (self.max[0] - self.min[0]).hypot(self.max[1] - self.min[1]);
        (0..c.len()).all(|i| {
            let (a, b) = (c[i], c[(i + 1) % c.len()]);
            let l = (b[0] - a[0]).hypot(b[1] - a[1]);
            l == 0. || points.iter().all(|&p| vectoriel(a, b, p) >= -tol * l)
        })
    }

    /// Parties visibles d'une polyligne ; les segments qui touchent un point aberrant sont écartés.
    pub fn couper_trait(&self, points: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
        if !points.iter().all(fini) {
            let suites = points.split(|p| !fini(p)).filter(|s| s.len() >= 2);
            return suites.flat_map(|s| self.couper_trait(s)).collect();
        }
        let (min, max) = emprise(points.iter().copied());
        if self.est_vide() || !self.touche(min, max) {
            return Vec::new();
        }
        if self.contient(points) {
            return vec![points.to_vec()];
        }
        points.clip_by_as::<i64>(&self.contours, FillRule::EvenOdd, BORD_INCLUS)
    }

    /// Partie visible d'une surface (triangles à plat), retriangulée ; les triangles qui touchent
    /// un point aberrant sont écartés.
    pub fn couper_surface(&self, triangles: &[[f64; 2]]) -> Vec<[f64; 2]> {
        if !triangles.iter().all(fini) {
            let sains: Vec<[f64; 2]> =
                triangles.chunks_exact(3).filter(|t| t.iter().all(fini)).flatten().copied().collect();
            return self.couper_surface(&sains);
        }
        let (min, max) = emprise(triangles.iter().copied());
        if self.est_vide() || !self.touche(min, max) {
            return Vec::new();
        }
        if self.contient(triangles) {
            return triangles.to_vec();
        }
        // Triangles orientés dans le même sens : leur réunion est l'intérieur non nul, même s'ils se recouvrent.
        let sujets: Vec<Vec<[f64; 2]>> = triangles
            .chunks_exact(3)
            .filter_map(|t| match vectoriel(t[0], t[1], t[2]) {
                a if a > 0. => Some(vec![t[0], t[1], t[2]]),
                a if a < 0. => Some(vec![t[0], t[2], t[1]]),
                _ => None,
            })
            .collect();
        let formes = FloatOverlay::<[f64; 2], i64>::from_subj_and_clip(&sujets, &self.contours)
            .overlay(OverlayRule::Intersect, FillRule::NonZero);
        let anneaux: Vec<Vec<[f64; 2]>> = formes.into_iter().flatten().collect();
        trianguler(&anneaux)
    }
}
