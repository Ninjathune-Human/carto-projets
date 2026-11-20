//! Aplatissement d'un dessin : l'espace objet, les blocs insérés et les xréfs deviennent une scène
//! de traits et de surfaces, en unités du dessin maître, dans l'ordre d'affichage (table SORTENTS de
//! chaque bloc quand elle existe, sinon ordre de la base), XCLIP et visibilité dynamique appliqués.
//! Une xréf apporte l'espace objet de son dessin, ses calques nommés `NOMXREF|calque`.

use crate::decoupe::{xclip_de, Zone};
use crate::geometrie::{arc_points, bande, ellipse_points, renflement_points, spline_points, trianguler};
use crate::hachures::anneaux_hachure;
use crate::importer::{est_xref, resoudre_chemin};
use crate::mesure::nom_de_type;
use crate::proprietes::{couche_visible, couleur_calque, metres_par_unite, resoudre_avec, Herite};
use crate::scene::{Primitive, Rapport, Scene};
use crate::ErreurCad;
use acadrust::entities::{EntityCommon, Insert, Spline};
use acadrust::objects::{BlockVisibilityParameter, BlockVisibilityState, ObjectType, SortEntitiesTable};
use acadrust::tables::Layer;
use acadrust::{BlockRecord, CadDocument, EntityType, Handle, Vector3};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::f64::consts::TAU;

/// Garde contre les blocs qui s'insèrent eux-mêmes.
const PROFONDEUR_MAX: usize = 16;
/// Garde contre les réseaux MINSERT démesurés (même borne qu'acadrust).
const CASES_MAX: usize = 1_000_000;
/// Drapeau de sommet de contrôle d'une polyligne lissée par spline (non affiché).
const SOMMET_DE_CONTROLE: u8 = 16;

/// Dessins d'un projet, par chemin normalisé dans le ZIP.
#[derive(Default)]
pub struct Dessins {
    pub docs: BTreeMap<String, CadDocument>,
    /// Dessins illisibles, par chemin normalisé, avec la cause.
    pub illisibles: BTreeMap<String, String>,
}

impl Dessins {
    /// Chemins de tous les dessins, lisibles ou non, triés.
    pub fn chemins(&self) -> Vec<String> {
        let mut v: Vec<String> = self.docs.keys().chain(self.illisibles.keys()).cloned().collect();
        v.sort();
        v.dedup();
        v
    }
}

/// Scène d'un dessin aplati et compte rendu de l'import.
pub struct Aplatissement {
    pub scene: Scene,
    pub rapport: Rapport,
}

/// Aplatit l'espace objet du dessin `maitre`, xréfs comprises ; les dessins illisibles sont signalés.
/// Tolérance de corde : 1 mm réel d'après l'INSUNITS du maître.
pub fn aplatir(dessins: &Dessins, maitre: &str) -> Result<Aplatissement, ErreurCad> {
    aplatir_avec_tolerance(dessins, maitre, None)
}

/// Comme [`aplatir`], avec une tolérance de corde imposée en unités du dessin maître (`None` :
/// 1 mm réel d'après INSUNITS), quand les unités reconnues contredisent INSUNITS.
pub fn aplatir_avec_tolerance(dessins: &Dessins, maitre: &str, tolerance: Option<f64>) -> Result<Aplatissement, ErreurCad> {
    if let Some(cause) = dessins.illisibles.get(maitre) {
        return Err(ErreurCad::Dwg(format!("{maitre} : {cause}")));
    }
    let (chemin, doc) = dessins.docs.get_key_value(maitre).ok_or_else(|| ErreurCad::MaitreInconnu(maitre.to_string()))?;
    let avertissements = dessins.illisibles.iter().map(|(c, cause)| format!("dessin illisible : {c} ({cause})")).collect();
    let cx = Contexte { chemin, doc, xref: None };
    let mut p = Parcours {
        dessins,
        maitre: doc,
        chemins: dessins.chemins(),
        tol: tolerance.unwrap_or_else(|| 0.001 / metres_par_unite(doc.header.insertion_units)),
        scene: Scene::default(),
        rapport: Rapport { maitre: maitre.to_string(), dessins: dessins.chemins(), avertissements, ..Rapport::default() },
        inclinees: 0,
        zone: None,
        tables: HashMap::from([(chemin.as_str(), tables_sortents(doc))]),
        calques_xref: HashMap::new(),
        pile: vec![chemin.as_str()],
    };
    if let Some(espace_objet) = doc.block_records.get("*Model_Space") {
        for e in p.contenu(cx, espace_objet) {
            p.entite(cx, e, &Affine::IDENTITE, None, 0);
        }
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

/// Polyligne à renflements : chaque suite de segments sans largeur donne un `Trait`, chaque suite
/// de segments larges une `Surface` (bande). Le segment i prend les largeurs du sommet i.
fn polyligne(s: &[Sommet], fermee: bool, tol: f64) -> Vec<Forme> {
    let n = s.len();
    if n < 2 {
        return Vec::new();
    }
    let nb = if fermee { n } else { n - 1 };
    let large = |i: usize| s[i].l0 != 0. || s[i].l1 != 0.;
    let uniforme = (0..nb).all(|i| large(i) == large(0));
    // Fermée et mixte : on part du début d'une suite pour ne pas la couper au premier sommet.
    let debut = if fermee && !uniforme { (0..nb).find(|&i| large(i) != large((i + nb - 1) % nb)).unwrap_or(0) } else { 0 };
    let fermee_entiere = fermee && uniforme;
    let mut formes = Vec::new();
    let mut k = 0;
    while k < nb {
        let suite_large = large((debut + k) % nb);
        let mut pts = vec![s[(debut + k) % nb].p];
        let mut largeurs = Vec::new();
        while k < nb && large((debut + k) % nb) == suite_large {
            let i = (debut + k) % nb;
            let (a, b) = (&s[i], &s[(i + 1) % n]);
            let sous = renflement_points(a.p, b.p, a.renflement, tol);
            let q = sous.len() as f64;
            for j in 0..sous.len() {
                let (t0, t1) = (j as f64 / q, (j + 1) as f64 / q);
                largeurs.push((a.l0 + (a.l1 - a.l0) * t0, a.l0 + (a.l1 - a.l0) * t1));
            }
            pts.extend(sous);
            k += 1;
        }
        formes.push(if !suite_large {
            Forme::Trait(pts)
        } else {
            if fermee_entiere {
                pts.pop(); // le dernier point est le premier : `bande` ferme elle-même
            }
            Forme::Surface(bande(&pts, &largeurs, fermee_entiere))
        });
    }
    formes
}

/// Largeurs par sommet si l'une des largeurs utilisées est non nulle, sinon largeurs par défaut
/// pour tous les segments ; le dernier sommet d'une polyligne ouverte ne porte aucun segment.
fn sommets(v: impl Iterator<Item = ([f64; 2], f64, f64, f64)>, defaut: (f64, f64), fermee: bool) -> Vec<Sommet> {
    let mut s: Vec<Sommet> = v.map(|(p, renflement, l0, l1)| Sommet { p, renflement, l0: l0.abs(), l1: l1.abs() }).collect();
    let utilises = if fermee { s.len() } else { s.len().saturating_sub(1) };
    if s[..utilises].iter().all(|v| v.l0 == 0. && v.l1 == 0.) {
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

/// Tables SORTENTS par bloc. Chaque table est rangée sous la clé `ACAD_SORTENTS` du dictionnaire
/// d'extension de son bloc et le désigne elle-même (`block_owner_handle`) : une seule passe sur les
/// objets évite de chercher ce dictionnaire bloc par bloc.
fn tables_sortents(doc: &CadDocument) -> HashMap<Handle, &SortEntitiesTable> {
    let mut tables: HashMap<Handle, &SortEntitiesTable> = HashMap::new();
    for o in doc.objects.values() {
        let ObjectType::SortEntitiesTable(t) = o else { continue };
        let bloc = match t.block_owner_handle {
            h if h.is_null() => doc.object_owner(t.owner_handle).unwrap_or(Handle::NULL),
            h => h,
        };
        match tables.entry(bloc) {
            _ if bloc.is_null() => {}
            Entry::Vacant(v) => {
                v.insert(t);
            }
            // Deux tables pour un bloc : la plus ancienne, pour un résultat indépendant du hachage.
            Entry::Occupied(mut o) if t.handle < o.get().handle => {
                o.insert(t);
            }
            Entry::Occupied(_) => {}
        }
    }
    tables
}

/// Entrée `cle` d'un dictionnaire.
fn entree_de(doc: &CadDocument, dico: Handle, cle: &str) -> Option<Handle> {
    match doc.objects.get(&dico)? {
        ObjectType::Dictionary(d) => d.get(cle),
        _ => None,
    }
}

/// État de visibilité courant : celui du cache d'évaluation de l'INSERT (dictionnaire d'extension
/// → `AcDbBlockRepresentation` → `AppDataCache` → `ACAD_ENHANCEDBLOCKDATA` → numéro du nœud, XRECORD
/// dont le code 1 nomme l'état), sinon le premier de la liste, état par défaut d'AutoCAD.
fn etat_courant<'p>(doc: &CadDocument, i: &Insert, p: &'p BlockVisibilityParameter) -> Option<&'p BlockVisibilityState> {
    let noeud = p.eval_node_id.to_string();
    let mut h = i.common.xdictionary_handle;
    for cle in ["AcDbBlockRepresentation", "AppDataCache", "ACAD_ENHANCEDBLOCKDATA", noeud.as_str()] {
        h = h.and_then(|d| entree_de(doc, d, cle));
    }
    let nom = match h.and_then(|h| doc.objects.get(&h)) {
        Some(ObjectType::XRecord(x)) => x.entries.iter().find(|e| e.code == 1).and_then(|e| e.value.as_string()),
        _ => None,
    };
    nom.and_then(|n| p.states.iter().find(|s| s.name == n)).or(p.states.first())
}

/// Entités qu'un bloc dynamique référencé directement masque : celles que gouverne son paramètre de
/// visibilité et que l'état courant ne montre pas. Une représentation anonyme (`*U…`) porte déjà
/// l'état évalué (entités des autres états marquées invisibles) : rien à filtrer.
fn masquees_par_visibilite(doc: &CadDocument, i: &Insert, br: &BlockRecord) -> HashSet<Handle> {
    if doc.block_visibility_params.is_empty() || br.flags.anonymous || i.block_name.starts_with('*') {
        return HashSet::new();
    }
    let Some((_, p)) = doc.dynamic_visibility_for_insert(i.common.handle) else { return HashSet::new() };
    let Some(etat) = etat_courant(doc, i, p) else { return HashSet::new() };
    let visibles: HashSet<&Handle> = etat.visible_blocks.iter().collect();
    let gouvernees = p.all_blocks.iter().chain(p.states.iter().flat_map(|s| &s.visible_blocks));
    gouvernees.filter(|h| !visibles.contains(h)).copied().collect()
}

/// Dessin en cours de parcours : le maître ou une xréf.
#[derive(Clone, Copy)]
struct Contexte<'a> {
    chemin: &'a str,
    doc: &'a CadDocument,
    /// Nom de la xréf dans son hôte, préfixe de ses calques ; `None` dans le maître.
    xref: Option<&'a str>,
}

struct Parcours<'a> {
    dessins: &'a Dessins,
    /// Dessin ouvert : sa table des calques porte les surcharges des calques de xréfs (VISRETAIN).
    maitre: &'a CadDocument,
    /// Chemins de tous les dessins de l'archive, pour résoudre les xréfs.
    chemins: Vec<String>,
    /// Tolérance de corde en unités du dessin maître.
    tol: f64,
    scene: Scene,
    rapport: Rapport,
    inclinees: u64,
    /// Zone visible des XCLIP englobants, dans le repère du maître ; `None` : pas de découpe.
    zone: Option<Zone>,
    /// Tables SORTENTS de chaque dessin parcouru (par chemin), par identifiant de bloc.
    tables: HashMap<&'a str, HashMap<Handle, &'a SortEntitiesTable>>,
    /// Calque d'origine, dans sa xréf, de chaque calque `NOMXREF|calque` rencontré.
    calques_xref: HashMap<String, Option<&'a Layer>>,
    /// Dessins ouverts, du maître à la xréf en cours : un dessin déjà ouvert ferme un cycle.
    pile: Vec<&'a str>,
}

impl<'a> Parcours<'a> {
    /// Entités d'un bloc dans l'ordre d'affichage : clés croissantes de sa table SORTENTS quand elle
    /// existe (une entité absente de la table, ou à clé nulle, a pour clé son propre identifiant ;
    /// à clé égale, ordre de la base), sinon ordre de la base.
    fn contenu(&self, cx: Contexte<'a>, br: &BlockRecord) -> Vec<&'a EntityType> {
        let doc = cx.doc;
        let mut v: Vec<&'a EntityType> = br.entity_handles.iter().filter_map(|h| doc.get_entity(*h)).collect();
        if let Some(t) = self.tables.get(cx.chemin).and_then(|t| t.get(&br.handle)) {
            v.sort_by_key(|e| {
                let h = e.common().handle;
                t.get_sort_handle(h).filter(|c| !c.is_null()).unwrap_or(h)
            });
        }
        v
    }

    /// Nom dans la scène du calque `nom` du dessin `cx` : `NOMXREF|calque` dans une xréf, sauf pour
    /// les calques `0` et `Defpoints`, communs à tous les dessins ; tel quel dans le maître.
    fn nom_calque(&mut self, cx: Contexte<'a>, nom: &str) -> String {
        match cx.xref {
            Some(x) if nom != "0" && !nom.eq_ignore_ascii_case("Defpoints") => {
                let n = format!("{x}|{nom}");
                self.calques_xref.entry(n.clone()).or_insert_with(|| cx.doc.layers.get(nom));
                n
            }
            _ => nom.to_string(),
        }
    }

    /// Calque de la scène tel que le maître le décrit (surcharges VISRETAIN des calques de xréfs),
    /// sinon tel que sa xréf d'origine le décrit.
    fn couche(&self, nom: &str) -> Option<&'a Layer> {
        self.maitre.layers.get(nom).or_else(|| self.calques_xref.get(nom).copied().flatten())
    }

    /// Propriétés effectives d'une entité du dessin `cx`.
    fn herite(&mut self, cx: Contexte<'a>, commun: &EntityCommon, parent: Option<&Herite>) -> Herite {
        let calque = match parent {
            Some(p) if commun.layer == "0" => p.calque.clone(),
            _ => self.nom_calque(cx, &commun.layer),
        };
        let couche = self.couche(&calque);
        resoudre_avec(commun, parent, calque, couche)
    }

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

    /// Une entité du dessin `cx` placée par `repere` (repère du bloc vers repère du maître) ; `parent`
    /// est l'insertion englobante.
    fn entite(&mut self, cx: Contexte<'a>, e: &'a EntityType, repere: &Affine, parent: Option<&Herite>, profondeur: usize) {
        let commun = e.common();
        match e {
            _ if commun.invisible => return,
            EntityType::Block(_) | EntityType::BlockEnd(_) | EntityType::Seqend(_) => return,
            // Hors de l'éditeur de blocs, seules les définitions d'attribut constantes s'affichent.
            EntityType::AttributeDefinition(a) if parent.is_some() && !a.flags.constant => return,
            _ => {}
        }
        let h = self.herite(cx, commun, parent);
        if let EntityType::Insert(i) = e {
            return self.insertion(cx, i, &h, repere, profondeur);
        }
        if !couche_visible(&h.calque, self.couche(&h.calque)) {
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
                let s = sommets(v, (p.constant_width, p.constant_width), p.is_closed);
                for f in polyligne(&s, p.is_closed, tol) {
                    self.emettre(&h, couleur, f, &m);
                }
                return;
            }
            EntityType::Polyline2D(p) => {
                let v = p
                    .vertices
                    .iter()
                    .filter(|v| v.flags.bits() & SOMMET_DE_CONTROLE == 0)
                    .map(|v| (xy(v.location), v.bulge, v.start_width, v.end_width));
                let s = sommets(v, (p.start_width, p.end_width), p.is_closed());
                for f in polyligne(&s, p.is_closed(), tol) {
                    self.emettre(&h, couleur, f, &m);
                }
                return;
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
                        couleur = self.herite(cx, &c, parent).couleur;
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
        let mut morceaux = match (&self.zone, surface) {
            (None, _) => vec![points],
            (Some(z), false) => z.couper_trait(&points),
            (Some(z), true) => vec![z.couper_surface(&points)],
        };
        morceaux.retain(|p| p.len() >= minimum);
        if morceaux.is_empty() {
            return;
        }
        let calque = self.scene.calque(&h.calque, couleur_calque(self.couche(&h.calque)));
        let couleur = [couleur[0], couleur[1], couleur[2], h.alpha];
        for points in morceaux {
            self.scene.primitives.push(if surface {
                Primitive::Surface { calque, couleur, triangles: points }
            } else {
                Primitive::Trait { calque, couleur, epaisseur_mm: h.epaisseur_mm, points }
            });
        }
    }

    /// Dessin désigné par la xréf `br` du dessin `cx`, ou `None` si elle n'est pas chargée : superposition
    /// sous une autre xréf (AutoCAD ne la charge que rattachée directement au maître), xréf déchargée,
    /// fichier introuvable ou illisible (déjà signalé), cycle.
    fn ouvrir_xref(&mut self, cx: Contexte<'a>, br: &'a BlockRecord) -> Option<Contexte<'a>> {
        if br.flags.is_xref_overlay && cx.xref.is_some() {
            return None;
        }
        if br.flags.is_xref_unloaded {
            self.avertir(format!("xréf déchargée : {}", br.name));
            return None;
        }
        let Some(chemin) = resoudre_chemin(&br.xref_path, cx.chemin, &self.chemins) else {
            let enregistre = if br.xref_path.is_empty() { &br.name } else { &br.xref_path };
            self.avertir(format!("xréf introuvable : {enregistre}"));
            return None;
        };
        let (chemin, doc) = self.dessins.docs.get_key_value(&chemin)?;
        if let Some(k) = self.pile.iter().position(|c| *c == chemin.as_str()) {
            let boucle: Vec<&str> = self.pile[k..].iter().copied().chain([chemin.as_str()]).collect();
            self.avertir(format!("cycle de xréfs : {}", boucle.join(" → ")));
            return None;
        }
        self.tables.entry(chemin.as_str()).or_insert_with(|| tables_sortents(doc));
        Some(Contexte { chemin, doc, xref: Some(br.name.as_str()) })
    }

    /// INSERT et MINSERT : `T(insertion) · R(rotation) · T(c · espacement, r · espacement) · S(échelles) · T(−base)`,
    /// précédé du miroir x → −x si la normale est −Z. Le contenu suit l'ordre d'affichage du bloc, privé
    /// des entités masquées par l'état de visibilité courant, et découpé par l'XCLIP de l'INSERT. Une
    /// xréf apporte l'espace objet de son dessin, de point de base son INSBASE ; la conversion d'unités
    /// est déjà dans l'échelle de l'INSERT (AutoCAD l'y inscrit au rattachement), rien ne s'y ajoute.
    fn insertion(&mut self, cx: Contexte<'a>, i: &Insert, h: &Herite, repere: &Affine, profondeur: usize) {
        if self.couche(&h.calque).is_some_and(|l| l.is_frozen()) {
            return;
        }
        let Some(br) = cx.doc.block_records.get(&i.block_name) else { return };
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
        let xref = est_xref(br);
        let (cible, bloc, base) = if xref {
            let Some(cible) = self.ouvrir_xref(cx, br) else { return };
            let Some(espace_objet) = cible.doc.block_records.get("*Model_Space") else { return };
            (cible, espace_objet, cible.doc.header.model_space_insertion_base)
        } else {
            let base = match cx.doc.get_entity(br.block_entity_handle) {
                Some(EntityType::Block(b)) => b.base_point,
                _ => br.base_point,
            };
            (cx, br, base)
        };
        let tete = repere
            .fois(&miroir)
            .fois(&Affine::translation(i.insert_point.x, i.insert_point.y))
            .fois(&Affine::rotation(i.rotation));
        let pied = Affine::echelle(i.x_scale(), i.y_scale()).fois(&Affine::translation(-base.x, -base.y));
        let mut contenu = self.contenu(cible, bloc);
        if !xref {
            let masquees = masquees_par_visibilite(cx.doc, i, br);
            contenu.retain(|e| !masquees.contains(&e.common().handle));
        }
        let decoupe = xclip_de(cx.doc, i);
        if xref {
            self.pile.push(cible.chemin);
        }
        for r in 0..lignes {
            for c in 0..colonnes {
                let case = Affine::translation(f64::from(c) * i.column_spacing, f64::from(r) * i.row_spacing);
                let t = tete.fois(&case).fois(&pied);
                // La découpe, portée dans le repère du maître, se compose avec celles des blocs englobants.
                let zone = decoupe.as_ref().and_then(|d| Zone::nouvelle(&d.iter().map(|p| t.point(*p)).collect::<Vec<_>>()));
                let englobante = match zone.map(|z| z.dans(self.zone.as_ref())) {
                    Some(z) if z.est_vide() => continue,
                    Some(z) => Some(self.zone.replace(z)),
                    None => None,
                };
                for e in &contenu {
                    self.entite(cible, e, &t, Some(h), profondeur + 1);
                }
                if let Some(z) = englobante {
                    self.zone = z;
                }
            }
        }
        if xref {
            self.pile.pop();
        }
    }
}
