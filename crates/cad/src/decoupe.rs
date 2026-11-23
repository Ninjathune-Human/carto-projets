//! Découpe XCLIP : le contour rattaché à une insertion, ramené dans le repère du bloc, devient une
//! zone visible dans le repère du dessin maître, qui coupe les traits et les surfaces du bloc. Une
//! découpe inversée masque l'intérieur du contour au lieu de l'extérieur. Les découpes imbriquées
//! se composent par intersection des zones visibles.

use crate::geometrie::trianguler;
use acadrust::entities::Insert;
use acadrust::objects::{ObjectType, XRecord};
use acadrust::types::Matrix4;
use acadrust::{CadDocument, Handle, Vector3};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::clip::FloatClip;
use i_overlay::float::overlay::FloatOverlay;
use i_overlay::string::clip::ClipRule;

/// Un trait posé sur le contour reste visible.
const BORD_INCLUS: ClipRule = ClipRule { invert: false, boundary_included: true };
/// Découpe inversée : un trait posé sur le contour reste visible aussi (i_overlay inverse le sens
/// de `boundary_included` avec `invert`).
const BORD_INCLUS_INVERSE: ClipRule = ClipRule { invert: true, boundary_included: false };

/// XRECORD de l'extension du SPATIAL_FILTER où AutoCAD note l'inversion (lu comme ezdxf).
const ALLER_RETOUR: &str = "ACAD_XREC_ROUNDTRIP";
/// Section (code 102) de cet XRECORD qui marque une découpe inversée ; ses points (code 10) sont
/// le contour intérieur.
const SECTION_INVERSEE: &str = "ACAD_INVERTEDCLIP_ROUNDTRIP";

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

/// Découpe XCLIP d'une insertion : contour dans le repère de définition du bloc, et sens.
#[derive(Clone, Debug, PartialEq)]
pub struct Xclip {
    pub contour: Vec<[f64; 2]>,
    /// Découpe inversée : l'intérieur du contour est masqué.
    pub inversee: bool,
}

/// Sommets (code 10) de la section `nom` (code 102) d'un XRECORD, s'il a cette section.
fn section(x: &XRecord, nom: &str) -> Option<Vec<[f64; 2]>> {
    let debut = x.entries.iter().position(|e| e.code == 102 && e.value.as_string() == Some(nom))?;
    let suite = x.entries[debut + 1..].iter().take_while(|e| e.code != 102);
    Some(suite.filter(|e| e.code == 10).filter_map(|e| e.value.as_point3d()).map(|(x, y, _)| [x, y]).collect())
}

/// Aire de la boîte englobante.
fn aire_boite(points: &[[f64; 2]]) -> f64 {
    let (min, max) = emprise(points.iter().copied());
    (max[0] - min[0]) * (max[1] - min[1])
}

/// Découpe XCLIP active d'une insertion, dans le repère de définition du bloc : dictionnaire
/// d'extension de l'INSERT → `ACAD_FILTER` → `SPATIAL`. La matrice du contour porte les points vers
/// le repère du contour : son inverse en ramène les sommets, puis l'inverse de la transformation du
/// bloc au moment de la découpe les place dans le bloc (`IBT · CBT⁻¹ · p`).
/// Inversée quand l'XRECORD `ACAD_XREC_ROUNDTRIP` du filtre a une section
/// `ACAD_INVERTEDCLIP_ROUNDTRIP` : AutoCAD range alors dans le filtre un contour « en trou de
/// serrure » (rectangle englobant relié au contour intérieur) et le contour intérieur dans cette
/// section ; comme ezdxf, le contour masqué est le plus petit des deux (par boîte englobante).
pub fn xclip_de(doc: &CadDocument, i: &Insert) -> Option<Xclip> {
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
    let vers_bloc = |local: &[[f64; 2]]| -> Vec<[f64; 2]> {
        let bloc = contour_xclip(local).into_iter().map(|p| m.transform_point(Vector3::new(p[0], p[1], 0.)));
        bloc.map(|q| [q.x, q.y]).collect()
    };
    let local: Vec<[f64; 2]> = f.boundary_points.iter().map(|p| [p.x, p.y]).collect();
    let mut contour = vers_bloc(&local);
    let inverse = doc.xrecord(f.handle, ALLER_RETOUR).and_then(|x| section(x, SECTION_INVERSEE));
    if let Some(interieur) = inverse.as_ref().filter(|v| v.len() >= 2) {
        let interieur = vers_bloc(interieur);
        if aire_boite(&interieur) < aire_boite(&contour) {
            contour = interieur;
        }
    }
    Some(Xclip { contour, inversee: inverse.is_some() })
}

/// Zone visible : contours bien formés (extérieurs dans le sens trigonométrique, trous dans le sens
/// horaire, sans recouvrement), vide quand des découpes imbriquées ne se recoupent pas. Inversée,
/// la zone visible est l'extérieur de ces contours.
#[derive(Clone, Debug)]
pub struct Zone {
    contours: Vec<Vec<[f64; 2]>>,
    min: [f64; 2],
    max: [f64; 2],
    /// Contour unique et convexe (cas du rectangle) : ce qui est dedans est gardé tel quel.
    convexe: bool,
    /// L'intérieur des contours est masqué, l'extérieur visible.
    inverse: bool,
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
        Zone::construire(contour, false)
    }

    /// Zone extérieure d'un contour (découpe inversée) ; `None` dans les mêmes cas que [`Zone::nouvelle`].
    pub fn inversee(contour: &[[f64; 2]]) -> Option<Zone> {
        Zone::construire(contour, true)
    }

    fn construire(contour: &[[f64; 2]], inverse: bool) -> Option<Zone> {
        if contour.len() < 3 || !contour.iter().all(fini) {
            return None;
        }
        let formes = FloatOverlay::<[f64; 2], i64>::from_subj(contour).overlay(OverlayRule::Subject, FillRule::EvenOdd);
        let contours: Vec<Vec<[f64; 2]>> = formes.into_iter().flatten().collect();
        (!contours.is_empty()).then(|| Zone::depuis(contours, inverse))
    }

    fn depuis(contours: Vec<Vec<[f64; 2]>>, inverse: bool) -> Zone {
        let (min, max) = emprise(contours.iter().flatten().copied());
        let convexe = match contours.as_slice() {
            [c] => {
                let n = c.len();
                let eps = 1e-12 * ((max[0] - min[0]).powi(2) + (max[1] - min[1]).powi(2));
                (0..n).all(|i| vectoriel(c[i], c[(i + 1) % n], c[(i + 2) % n]) >= -eps)
            }
            _ => false,
        };
        Zone { contours, min, max, convexe, inverse }
    }

    /// Plus rien n'est visible.
    pub fn est_vide(&self) -> bool {
        !self.inverse && self.contours.is_empty()
    }

    /// La zone vue à travers la découpe des blocs englobants, s'il y en a une.
    pub fn dans(self, englobante: Option<&Zone>) -> Zone {
        match englobante {
            Some(e) => e.inter(&self),
            None => self,
        }
    }

    /// Partie commune de deux zones : découpe d'un bloc dans un bloc découpé. Avec une zone
    /// inversée, c'est une différence ; deux zones inversées masquent la réunion de leurs contours.
    pub fn inter(&self, autre: &Zone) -> Zone {
        let (regle, inverse) = match (self.inverse, autre.inverse) {
            (false, false) if !self.touche(autre.min, autre.max) => return Zone::depuis(Vec::new(), false),
            (false, false) => (OverlayRule::Intersect, false),
            (false, true) => (OverlayRule::Difference, false),
            (true, false) => (OverlayRule::InverseDifference, false),
            (true, true) => (OverlayRule::Union, true),
        };
        let formes = FloatOverlay::<[f64; 2], i64>::from_subj_and_clip(&self.contours, &autre.contours)
            .overlay(regle, FillRule::EvenOdd);
        Zone::depuis(formes.into_iter().flatten().collect(), inverse)
    }

    /// Les emprises se touchent (bords compris).
    fn touche(&self, min: [f64; 2], max: [f64; 2]) -> bool {
        min[0] <= self.max[0] && max[0] >= self.min[0] && min[1] <= self.max[1] && max[1] >= self.min[1]
    }

    /// Tous les points sont dans la zone convexe (bord compris) : rien à couper.
    fn contient(&self, points: &[[f64; 2]]) -> bool {
        if !self.convexe || self.inverse {
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
        if self.inverse {
            if self.contours.is_empty() || !self.touche(min, max) {
                return vec![points.to_vec()];
            }
            return points.clip_by_as::<i64>(&self.contours, FillRule::EvenOdd, BORD_INCLUS_INVERSE);
        }
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
        if self.inverse && (self.contours.is_empty() || !self.touche(min, max)) {
            return triangles.to_vec();
        }
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
        let regle = if self.inverse { OverlayRule::Difference } else { OverlayRule::Intersect };
        let formes = FloatOverlay::<[f64; 2], i64>::from_subj_and_clip(&sujets, &self.contours).overlay(regle, FillRule::NonZero);
        let anneaux: Vec<Vec<[f64; 2]>> = formes.into_iter().flatten().collect();
        trianguler(&anneaux)
    }
}
