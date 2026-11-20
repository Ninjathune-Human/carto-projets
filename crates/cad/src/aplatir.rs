//! Aplatissement d'un dessin : l'espace objet et les blocs insérés deviennent une scène de
//! traits et de surfaces, en unités du dessin maître, dans l'ordre de la base.

use crate::geometrie::{arc_points, bande, ellipse_points, renflement_points, spline_points, trianguler};
use crate::hachures::anneaux_hachure;
use crate::mesure::nom_de_type;
use crate::proprietes::{calque_gele, calque_visible, couleur_calque, metres_par_unite, resoudre, Herite};
use crate::scene::{Primitive, Rapport, Scene};
use crate::ErreurCad;
use acadrust::entities::{Insert, Spline};
use acadrust::{CadDocument, EntityType, Vector3};
use std::collections::BTreeMap;
use std::f64::consts::TAU;

/// Garde contre les blocs qui s'insèrent eux-mêmes.
const PROFONDEUR_MAX: usize = 16;
/// Garde contre les réseaux MINSERT démesurés (même borne qu'acadrust).
const CASES_MAX: usize = 1_000_000;
/// Drapeau de sommet de contrôle d'une polyligne lissée par spline (non affiché).
const SOMMET_DE_CONTROLE: u8 = 16;

/// Dessins d'un projet, par chemin normalisé dans le ZIP.
pub struct Dessins {
    pub docs: BTreeMap<String, CadDocument>,
}

/// Scène d'un dessin aplati et compte rendu de l'import.
pub struct Aplatissement {
    pub scene: Scene,
    pub rapport: Rapport,
}

/// Aplatit l'espace objet du dessin `maitre` ; les xréfs ne sont pas encore résolues.
pub fn aplatir(dessins: &Dessins, maitre: &str) -> Result<Aplatissement, ErreurCad> {
    let doc = dessins.docs.get(maitre).ok_or_else(|| ErreurCad::MaitreInconnu(maitre.to_string()))?;
    let mut p = Parcours {
        doc,
        tol: 0.001 / metres_par_unite(doc.header.insertion_units),
        scene: Scene::default(),
        rapport: Rapport { maitre: maitre.to_string(), dessins: vec![maitre.to_string()], ..Rapport::default() },
        inclinees: 0,
    };
    for e in doc.model_space_entities() {
        p.entite(e, &Affine::IDENTITE, None, 0);
    }
    if p.inclinees > 0 {
        p.rapport
            .avertissements
            .push(format!("objets hors du plan XY ignorés (normale d'extrusion non verticale) : {}", p.inclinees));
    }
    Ok(Aplatissement { scene: p.scene, rapport: p.rapport })
}

/// Transformation affine du plan : `(x, y) → (a x + b y + c, d x + e y + f)`.
#[derive(Clone, Copy, Debug)]
struct Affine([f64; 6]);

impl Affine {
    const IDENTITE: Affine = Affine([1., 0., 0., 0., 1., 0.]);
    /// Passage d'un repère d'objet de normale −Z au repère général : x → −x.
    const MIROIR_X: Affine = Affine([-1., 0., 0., 0., 1., 0.]);

    fn translation(x: f64, y: f64) -> Self {
        Affine([1., 0., x, 0., 1., y])
    }

    fn rotation(angle: f64) -> Self {
        let (s, c) = angle.sin_cos();
        Affine([c, -s, 0., s, c, 0.])
    }

    fn echelle(sx: f64, sy: f64) -> Self {
        Affine([sx, 0., 0., 0., sy, 0.])
    }

    /// Composition : `m` d'abord, puis `self`.
    fn fois(&self, m: &Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [p, q, r, s, t, u] = m.0;
        Affine([a * p + b * s, a * q + b * t, a * r + b * u + c, d * p + e * s, d * q + e * t, d * r + e * u + f])
    }

    fn point(&self, p: [f64; 2]) -> [f64; 2] {
        let [a, b, c, d, e, f] = self.0;
        [a * p[0] + b * p[1] + c, d * p[0] + e * p[1] + f]
    }

    /// Plus grande valeur singulière de la partie linéaire.
    fn echelle_max(&self) -> f64 {
        let [a, b, _, d, e, _] = self.0;
        let s = a * a + b * b + d * d + e * e;
        let det = a * e - b * d;
        ((s + (s * s - 4. * det * det).max(0.).sqrt()) / 2.).sqrt()
    }
}

/// Position du repère d'objet (OCS) d'une entité par rapport au plan XY.
enum Plan {
    Direct,
    Miroir,
    Incline,
}

fn plan(n: Vector3) -> Plan {
    let l = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
    if !(l.is_finite() && l > 0.) {
        return Plan::Direct;
    }
    if n.x.abs() > 1e-6 * l || n.y.abs() > 1e-6 * l {
        Plan::Incline
    } else if n.z < 0. {
        Plan::Miroir
    } else {
        Plan::Direct
    }
}

/// Géométrie d'une entité dans son repère local, avant transformation.
enum Forme {
    Trait(Vec<[f64; 2]>),
    Surface(Vec<[f64; 2]>),
}

/// Sommet de polyligne : position, renflement du segment sortant, largeurs de début et de fin.
struct Sommet {
    p: [f64; 2],
    renflement: f64,
    l0: f64,
    l1: f64,
}

fn xy(v: Vector3) -> [f64; 2] {
    [v.x, v.y]
}

/// Un trait fermé répète exactement son premier point.
fn fermer(v: &mut [[f64; 2]]) {
    if let Some(&p0) = v.first() {
        let n = v.len();
        v[n - 1] = p0;
    }
}

/// Polyligne à renflements : `Trait` sans largeur, sinon `Surface` (bande).
fn polyligne(s: &[Sommet], fermee: bool, tol: f64) -> Option<Forme> {
    let n = s.len();
    if n < 2 {
        return None;
    }
    let mut pts = vec![s[0].p];
    let mut largeurs = Vec::new();
    for i in 0..if fermee { n } else { n - 1 } {
        let (a, b) = (&s[i], &s[(i + 1) % n]);
        let sous = renflement_points(a.p, b.p, a.renflement, tol);
        let k = sous.len() as f64;
        for j in 0..sous.len() {
            let (t0, t1) = (j as f64 / k, (j + 1) as f64 / k);
            largeurs.push((a.l0 + (a.l1 - a.l0) * t0, a.l0 + (a.l1 - a.l0) * t1));
        }
        pts.extend(sous);
    }
    if s.iter().all(|v| v.l0 == 0. && v.l1 == 0.) {
        return Some(Forme::Trait(pts));
    }
    if fermee {
        pts.pop(); // le dernier point est le premier : `bande` ferme elle-même
    }
    Some(Forme::Surface(bande(&pts, &largeurs, fermee)))
}

/// Largeurs par sommet si l'une est non nulle, sinon largeurs par défaut pour tous les segments.
fn sommets(v: impl Iterator<Item = ([f64; 2], f64, f64, f64)>, defaut: (f64, f64)) -> Vec<Sommet> {
    let mut s: Vec<Sommet> = v.map(|(p, renflement, l0, l1)| Sommet { p, renflement, l0: l0.abs(), l1: l1.abs() }).collect();
    if s.iter().all(|v| v.l0 == 0. && v.l1 == 0.) {
        for v in &mut s {
            (v.l0, v.l1) = (defaut.0.abs(), defaut.1.abs());
        }
    }
    s
}

fn courbe_spline(s: &Spline, tol: f64) -> Vec<[f64; 2]> {
    if s.control_points.len() < 2 {
        // Spline définie par ses seuls points d'ajustement : approchée par la polyligne qui les relie.
        return s.fit_points.iter().map(|p| xy(*p)).collect();
    }
    let ctrl: Vec<[f64; 2]> = s.control_points.iter().map(|p| xy(*p)).collect();
    let poids = (s.weights.len() == ctrl.len()).then_some(s.weights.as_slice());
    spline_points(usize::try_from(s.degree).unwrap_or(3).max(1), &s.knots, &ctrl, poids, tol)
}

struct Parcours<'a> {
    doc: &'a CadDocument,
    /// Tolérance de corde en unités du dessin maître.
    tol: f64,
    scene: Scene,
    rapport: Rapport,
    inclinees: u64,
}

impl Parcours<'_> {
    fn avertir(&mut self, message: String) {
        if !self.rapport.avertissements.contains(&message) {
            self.rapport.avertissements.push(message);
        }
    }

    fn ignorer(&mut self, e: &EntityType) {
        let nom = nom_de_type(e);
        if !matches!(nom.as_str(), "ATTRIB" | "SEQEND" | "VERTEX") {
            *self.rapport.ignorees.entry(nom).or_insert(0) += 1;
        }
    }

    /// Une entité placée par `repere` (repère du bloc vers repère du maître) ; `parent` est l'insertion englobante.
    fn entite(&mut self, e: &EntityType, repere: &Affine, parent: Option<&Herite>, profondeur: usize) {
        let commun = e.common();
        match e {
            _ if commun.invisible => return,
            EntityType::Block(_) | EntityType::BlockEnd(_) | EntityType::Seqend(_) => return,
            // Hors de l'éditeur de blocs, seules les définitions d'attribut constantes s'affichent.
            EntityType::AttributeDefinition(a) if parent.is_some() && !a.flags.constant => return,
            _ => {}
        }
        let h = resoudre(self.doc, commun, parent);
        if let EntityType::Insert(i) = e {
            return self.insertion(i, &h, repere, profondeur);
        }
        if !calque_visible(self.doc, &h.calque) {
            return;
        }
        let normale = match e {
            EntityType::Arc(a) => a.normal,
            EntityType::Circle(c) => c.normal,
            EntityType::Ellipse(el) => el.normal,
            EntityType::LwPolyline(p) => p.normal,
            EntityType::Polyline2D(p) => p.normal,
            EntityType::Solid(s) => s.normal,
            EntityType::Hatch(hh) => hh.normal,
            _ => Vector3::UNIT_Z, // entités en coordonnées générales
        };
        let (m, miroir) = match plan(normale) {
            Plan::Direct => (*repere, false),
            Plan::Miroir => (repere.fois(&Affine::MIROIR_X), true),
            Plan::Incline => {
                self.inclinees += 1;
                return;
            }
        };
        let echelle = m.echelle_max();
        let tol = if echelle.is_finite() && echelle > 0. { self.tol / echelle } else { self.tol };
        let mut couleur = h.couleur;
        let forme = match e {
            EntityType::Line(l) => Forme::Trait(vec![xy(l.start), xy(l.end)]),
            EntityType::Arc(a) => Forme::Trait(arc_points(xy(a.center), a.radius, a.start_angle, a.end_angle, tol)),
            EntityType::Circle(c) => {
                let mut v = arc_points(xy(c.center), c.radius, 0., TAU, tol);
                fermer(&mut v);
                Forme::Trait(v)
            }
            EntityType::Ellipse(el) => {
                // Coordonnées générales : avec une normale −Z, on passe par le miroir pour le sens.
                let s = if miroir { -1. } else { 1. };
                let (t0, mut t1) = (el.start_parameter, el.end_parameter);
                let complete = (t1 - t0).abs() < 1e-12 || ((t1 - t0).abs() - TAU).abs() < 1e-10;
                if complete {
                    t1 = t0 + TAU;
                }
                let c = [s * el.center.x, el.center.y];
                let axe = [s * el.major_axis.x, el.major_axis.y];
                let mut v = ellipse_points(c, axe, el.minor_axis_ratio, t0, t1, tol);
                if complete {
                    fermer(&mut v);
                }
                Forme::Trait(v)
            }
            EntityType::LwPolyline(p) => {
                let v = p.vertices.iter().map(|v| ([v.location.x, v.location.y], v.bulge, v.start_width, v.end_width));
                let s = sommets(v, (p.constant_width, p.constant_width));
                let Some(f) = polyligne(&s, p.is_closed, tol) else { return };
                f
            }
            EntityType::Polyline2D(p) => {
                let v = p
                    .vertices
                    .iter()
                    .filter(|v| v.flags.bits() & SOMMET_DE_CONTROLE == 0)
                    .map(|v| (xy(v.location), v.bulge, v.start_width, v.end_width));
                let s = sommets(v, (p.start_width, p.end_width));
                let Some(f) = polyligne(&s, p.is_closed(), tol) else { return };
                f
            }
            EntityType::Polyline3D(p) => {
                let v = p.vertices.iter().filter(|v| v.flags & i32::from(SOMMET_DE_CONTROLE) == 0);
                let mut pts: Vec<[f64; 2]> = v.map(|v| xy(v.position)).collect();
                if p.flags.closed && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                Forme::Trait(pts)
            }
            EntityType::Polyline(p) => {
                let v = p.vertices.iter().filter(|v| v.flags.bits() & SOMMET_DE_CONTROLE == 0);
                let mut pts: Vec<[f64; 2]> = v.map(|v| xy(v.location)).collect();
                if p.is_closed() && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                Forme::Trait(pts)
            }
            EntityType::Spline(s) => Forme::Trait(courbe_spline(s, tol)),
            EntityType::Solid(s) => {
                // SOLID et TRACE : contour 1, 2, 4, 3 (un « nœud papillon » reste en deux triangles).
                let mut anneau: Vec<[f64; 2]> = Vec::new();
                for p in [s.first_corner, s.second_corner, s.fourth_corner, s.third_corner].map(xy) {
                    if anneau.last() != Some(&p) {
                        anneau.push(p);
                    }
                }
                if anneau.len() > 1 && anneau.first() == anneau.last() {
                    anneau.pop();
                }
                Forme::Surface(trianguler(&[anneau]))
            }
            EntityType::Hatch(hh) => {
                // HATCH et MPOLYGON : pleines, ou à dégradé rendu dans sa première couleur.
                if hh.gradient_color.enabled {
                    if let Some(g) = hh.gradient_color.colors.first() {
                        let mut c = commun.clone();
                        c.color = g.color;
                        couleur = resoudre(self.doc, &c, parent).couleur;
                    }
                } else if !hh.is_solid {
                    return self.ignorer(e);
                }
                Forme::Surface(trianguler(&anneaux_hachure(hh, tol)))
            }
            _ => return self.ignorer(e),
        };
        self.emettre(&h, couleur, forme, &m);
    }

    fn emettre(&mut self, h: &Herite, couleur: [u8; 3], forme: Forme, m: &Affine) {
        let (surface, points, minimum) = match forme {
            Forme::Trait(p) => (false, p, 2),
            Forme::Surface(t) => (true, t, 3),
        };
        if points.len() < minimum {
            return;
        }
        let points: Vec<[f64; 2]> = points.into_iter().map(|p| m.point(p)).collect();
        if !points.iter().all(|p| p[0].is_finite() && p[1].is_finite()) {
            return;
        }
        let calque = self.scene.calque(&h.calque, couleur_calque(self.doc.layers.get(&h.calque)));
        let couleur = [couleur[0], couleur[1], couleur[2], h.alpha];
        self.scene.primitives.push(if surface {
            Primitive::Surface { calque, couleur, triangles: points }
        } else {
            Primitive::Trait { calque, couleur, epaisseur_mm: h.epaisseur_mm, points }
        });
    }

    /// INSERT et MINSERT : `T(insertion) · R(rotation) · T(c · espacement, r · espacement) · S(échelles) · T(−base)`,
    /// précédé du miroir x → −x si la normale est −Z.
    fn insertion(&mut self, i: &Insert, h: &Herite, repere: &Affine, profondeur: usize) {
        let doc = self.doc;
        if calque_gele(doc, &h.calque) {
            return;
        }
        let Some(br) = doc.block_records.get(&i.block_name) else { return };
        if br.flags.is_xref || br.flags.is_xref_overlay || !br.xref_path.is_empty() {
            return self.avertir(format!("xréf non résolue : {}", i.block_name));
        }
        if profondeur >= PROFONDEUR_MAX {
            return self.avertir(format!("imbrication de blocs trop profonde : {}", i.block_name));
        }
        let miroir = match plan(i.normal) {
            Plan::Direct => Affine::IDENTITE,
            Plan::Miroir => Affine::MIROIR_X,
            Plan::Incline => {
                self.inclinees += 1;
                return;
            }
        };
        let (colonnes, lignes) = if i.is_minsert() { (i.column_count.max(1), i.row_count.max(1)) } else { (1, 1) };
        if usize::from(colonnes) * usize::from(lignes) > CASES_MAX {
            return self.avertir(format!("réseau MINSERT trop grand, ignoré : {}", i.block_name));
        }
        let base = match doc.get_entity(br.block_entity_handle) {
            Some(EntityType::Block(b)) => b.base_point,
            _ => br.base_point,
        };
        let tete = repere
            .fois(&miroir)
            .fois(&Affine::translation(i.insert_point.x, i.insert_point.y))
            .fois(&Affine::rotation(i.rotation));
        let pied = Affine::echelle(i.x_scale(), i.y_scale()).fois(&Affine::translation(-base.x, -base.y));
        let contenu: Vec<&EntityType> = doc.entities_in_block(&i.block_name).collect();
        for r in 0..lignes {
            for c in 0..colonnes {
                let case = Affine::translation(f64::from(c) * i.column_spacing, f64::from(r) * i.row_spacing);
                let t = tete.fois(&case).fois(&pied);
                for e in &contenu {
                    self.entite(e, &t, Some(h), profondeur + 1);
                }
            }
        }
    }
}
