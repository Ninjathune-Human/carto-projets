use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

use acadrust::entities::hatch::{BoundaryEdge, BoundaryPath, PolylineEdge};
use acadrust::entities::{
    Block, Dimension, DimensionLinear, Hatch, Insert, LwVertex, Polyline2D, Polyline3D, Solid, Vertex2D,
};
use acadrust::objects::{
    BlockVisibilityParameter, BlockVisibilityState, Dictionary, ObjectType, SortEntitiesTable, SpatialFilter, XRecord,
};
use acadrust::tables::{BlockRecord, Layer};
use acadrust::types::Matrix4;
use acadrust::{
    Arc, CadDocument, Circle, Color, Ellipse, EntityType, Handle, Line, LineWeight, LwPolyline, Spline, Text,
    Vector2, Vector3,
};
use carto_cad::aplatir::{aplatir, Aplatissement, Dessins};
use carto_cad::proprietes::couleur_aci;
use carto_cad::{ErreurCad, Primitive};

// --- Aides (reprises de tests/lignes.rs) ---

fn ligne(x1: f64, y1: f64, x2: f64, y2: f64, calque: &str) -> EntityType {
    let mut l = Line::from_coords(x1, y1, 0., x2, y2, 0.);
    l.common.layer = calque.into();
    EntityType::Line(l)
}

/// Définit un bloc et renvoie les identifiants de ses entités, dans l'ordre.
fn definir_bloc(doc: &mut CadDocument, nom: &str, base: Vector3, contenu: Vec<EntityType>) -> Vec<Handle> {
    let mut record = BlockRecord::new(nom);
    record.handle = doc.allocate_handle();
    let proprietaire = record.handle;
    doc.block_records.add(record).unwrap();
    let mut bloc = Block::new(nom, base);
    bloc.common.owner_handle = proprietaire;
    let h = doc.add_entity(EntityType::Block(bloc)).unwrap();
    doc.block_records.get_mut(nom).unwrap().block_entity_handle = h;
    contenu
        .into_iter()
        .map(|mut e| {
            e.common_mut().owner_handle = proprietaire;
            doc.add_entity(e).unwrap()
        })
        .collect()
}

fn inserer(doc: &mut CadDocument, i: Insert) -> Handle {
    doc.add_entity(EntityType::Insert(i)).unwrap()
}

/// Ajoute un objet de dictionnaire `cle` → `h` au dictionnaire `dico`.
fn entree(doc: &mut CadDocument, dico: Handle, cle: &str, h: Handle) {
    let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dico) else { panic!("dictionnaire attendu") };
    d.add_entry(cle, h);
}

/// Sous-dictionnaire `cle` de `parent`.
fn sous_dictionnaire(doc: &mut CadDocument, parent: Handle, cle: &str) -> Handle {
    let mut d = Dictionary::new();
    d.handle = doc.allocate_handle();
    d.owner = parent;
    let h = d.handle;
    doc.objects.insert(h, ObjectType::Dictionary(d));
    entree(doc, parent, cle, h);
    h
}

/// XCLIP de l'INSERT `insertion` : dictionnaire d'extension → `ACAD_FILTER` → `SPATIAL`.
fn attacher_xclip(doc: &mut CadDocument, insertion: Handle, mut f: SpatialFilter) {
    let xdict = doc.ensure_extension_dictionary(insertion);
    let filtres = sous_dictionnaire(doc, xdict, "ACAD_FILTER");
    f.handle = doc.allocate_handle();
    f.owner = filtres;
    let h = f.handle;
    doc.objects.insert(h, ObjectType::SpatialFilter(f));
    entree(doc, filtres, "SPATIAL", h);
}

fn filtre(coins: &[(f64, f64)]) -> SpatialFilter {
    let mut f = SpatialFilter::new();
    f.boundary_points = coins.iter().map(|&(x, y)| Vector2::new(x, y)).collect();
    f
}

/// Table SORTENTS du bloc `bloc` (dictionnaire d'extension → `ACAD_SORTENTS`) : entité → clé de tri.
fn attacher_sortents(doc: &mut CadDocument, bloc: Handle, cles: &[(Handle, Handle)]) {
    let mut xdict = Dictionary::new();
    xdict.handle = doc.allocate_handle();
    xdict.owner = bloc;
    let hx = xdict.handle;
    doc.objects.insert(hx, ObjectType::Dictionary(xdict));
    let mut t = SortEntitiesTable::for_block(bloc);
    t.handle = doc.allocate_handle();
    t.owner_handle = hx;
    for &(e, cle) in cles {
        t.add_entry(e, cle);
    }
    let ht = t.handle;
    doc.objects.insert(ht, ObjectType::SortEntitiesTable(t));
    entree(doc, hx, "ACAD_SORTENTS", ht);
}

fn ajouter_calque(doc: &mut CadDocument, nom: &str, eteint: bool, gele: bool) {
    let mut c = Layer::new(nom);
    if eteint {
        c.turn_off();
    }
    if gele {
        c.freeze();
    }
    doc.layers.add(c).unwrap();
}

fn aplatir_doc(doc: CadDocument) -> Aplatissement {
    let mut docs = BTreeMap::new();
    docs.insert("plan.dwg".to_string(), doc);
    aplatir(&Dessins { docs }, "plan.dwg").unwrap()
}

fn proche(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

fn points_proches(obtenu: &[[f64; 2]], attendu: &[[f64; 2]]) -> bool {
    obtenu.len() == attendu.len() && obtenu.iter().zip(attendu).all(|(a, b)| proche(*a, *b))
}

fn traits(a: &Aplatissement) -> Vec<&Vec<[f64; 2]>> {
    a.scene
        .primitives
        .iter()
        .filter_map(|p| match p {
            Primitive::Trait { points, .. } => Some(points),
            _ => None,
        })
        .collect()
}

fn couleur(p: &Primitive) -> [u8; 4] {
    match p {
        Primitive::Trait { couleur, .. } | Primitive::Surface { couleur, .. } => *couleur,
    }
}

fn nom_calque<'a>(a: &'a Aplatissement, p: &Primitive) -> &'a str {
    let i = match p {
        Primitive::Trait { calque, .. } | Primitive::Surface { calque, .. } => *calque,
    };
    &a.scene.calques[i as usize].nom
}

fn aire_triangles(t: &[[f64; 2]]) -> f64 {
    t.chunks(3)
        .map(|c| ((c[1][0] - c[0][0]) * (c[2][1] - c[0][1]) - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1])).abs() / 2.0)
        .sum()
}

fn rgba(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

fn carre(x0: f64, y0: f64, c: f64) -> BoundaryPath {
    let mut p = BoundaryPath::new();
    p.edges.push(BoundaryEdge::Polyline(PolylineEdge::new(
        vec![Vector2::new(x0, y0), Vector2::new(x0 + c, y0), Vector2::new(x0 + c, y0 + c), Vector2::new(x0, y0 + c)],
        true,
    )));
    p
}

fn hachure_carree() -> Hatch {
    let mut h = Hatch::solid();
    h.paths = vec![carre(0., 0., 10.)];
    h
}

// --- Tests ---

#[test]
fn entites_simples() {
    let mut doc = CadDocument::new();
    let mut bleu = Layer::new("Bleu");
    bleu.color = Color::Index(5);
    doc.layers.add(bleu).unwrap();

    let mut l = Line::from_coords(0., 0., 0., 10., 0., 0.);
    l.common.color = Color::Index(1);
    l.common.line_weight = LineWeight::Value(50);
    doc.add_entity(EntityType::Line(l)).unwrap();

    let mut a = Arc::from_coords(0., 0., 0., 2., 0., FRAC_PI_2);
    a.common.layer = "Bleu".into();
    doc.add_entity(EntityType::Arc(a)).unwrap();

    let mut c = Circle::from_coords(20., 0., 0., 3.);
    c.common.color = Color::Rgb { r: 10, g: 20, b: 30 };
    doc.add_entity(EntityType::Circle(c)).unwrap();

    let e = Ellipse::from_center_axes(Vector3::new(40., 0., 0.), Vector3::new(4., 0., 0.), 0.5);
    doc.add_entity(EntityType::Ellipse(e)).unwrap();

    let mut p = LwPolyline::from_points(vec![Vector2::new(0., 50.), Vector2::new(10., 50.)]);
    p.constant_width = 0.5;
    p.common.color = Color::Index(3);
    doc.add_entity(EntityType::LwPolyline(p)).unwrap();

    let mut h = hachure_carree();
    h.common.color = Color::Index(2);
    doc.add_entity(EntityType::Hatch(Box::new(h))).unwrap();

    let mut s = Solid::new(
        Vector3::new(100., 0., 0.),
        Vector3::new(110., 0., 0.),
        Vector3::new(100., 10., 0.),
        Vector3::new(110., 10., 0.),
    );
    s.common.color = Color::Index(4);
    doc.add_entity(EntityType::Solid(s)).unwrap();

    let r = aplatir_doc(doc);
    let p = &r.scene.primitives;
    assert_eq!(p.len(), 7, "{p:?}");
    assert_eq!(traits(&r).len(), 4);

    // LINE : rouge, 0,5 mm, deux points
    match &p[0] {
        Primitive::Trait { epaisseur_mm, points, .. } => {
            assert!((epaisseur_mm - 0.5).abs() < 1e-6);
            assert!(points_proches(points, &[[0., 0.], [10., 0.]]));
        }
        autre => panic!("LINE : {autre:?}"),
    }
    assert_eq!(couleur(&p[0]), rgba(couleur_aci(1)));

    // ARC : couleur du calque Bleu, rayon 2, de (2,0) à (0,2)
    let Primitive::Trait { points, .. } = &p[1] else { panic!("ARC") };
    assert_eq!(couleur(&p[1]), rgba(couleur_aci(5)));
    assert_eq!(nom_calque(&r, &p[1]), "Bleu");
    assert!(proche(points[0], [2., 0.]) && proche(*points.last().unwrap(), [0., 2.]));
    assert!(points.iter().all(|q| (q[0].hypot(q[1]) - 2.).abs() < 1e-9));

    // CIRCLE : RVB, fermé
    let Primitive::Trait { points, .. } = &p[2] else { panic!("CIRCLE") };
    assert_eq!(couleur(&p[2]), [10, 20, 30, 255]);
    assert!(points.len() > 8);
    assert_eq!(points[0], *points.last().unwrap());
    assert!(points.iter().all(|q| ((q[0] - 20.).hypot(q[1]) - 3.).abs() < 1e-9));

    // ELLIPSE : calque 0 (blanc/noir), fermée
    let Primitive::Trait { points, .. } = &p[3] else { panic!("ELLIPSE") };
    assert_eq!(couleur(&p[3]), [0, 0, 0, 255]);
    assert_eq!(points[0], *points.last().unwrap());
    assert!(points.iter().any(|q| proche(*q, [44., 0.])));

    // LWPOLYLINE à largeur : bande de 10 × 0,5
    let Primitive::Surface { triangles, .. } = &p[4] else { panic!("LWPOLYLINE") };
    assert_eq!(couleur(&p[4]), rgba(couleur_aci(3)));
    assert!((aire_triangles(triangles) - 5.).abs() < 1e-9);

    // HATCH pleine
    let Primitive::Surface { triangles, .. } = &p[5] else { panic!("HATCH") };
    assert_eq!(couleur(&p[5]), rgba(couleur_aci(2)));
    assert!((aire_triangles(triangles) - 100.).abs() < 1e-6);

    // SOLID (ordre 1, 2, 4, 3)
    let Primitive::Surface { triangles, .. } = &p[6] else { panic!("SOLID") };
    assert_eq!(couleur(&p[6]), rgba(couleur_aci(4)));
    assert!((aire_triangles(triangles) - 100.).abs() < 1e-6);
}

#[test]
fn bloc_rotation_echelle_base() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::new(1., 0., 0.), vec![ligne(1., 0., 2., 0., "0")]);
    inserer(&mut doc, Insert::new("B", Vector3::new(10., 20., 0.)).with_rotation(FRAC_PI_2).with_scale(2., 2., 1.));
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1);
    assert!(points_proches(t[0], &[[10., 20.], [10., 22.]]), "{:?}", t[0]);
}

#[test]
fn bloc_miroir_echelle_negative() {
    let mut doc = CadDocument::new();
    let arc = Arc::from_coords(0., 0., 0., 1., 0., FRAC_PI_2);
    definir_bloc(&mut doc, "M", Vector3::ZERO, vec![EntityType::Arc(arc)]);
    inserer(&mut doc, Insert::new("M", Vector3::new(10., 0., 0.)).with_scale(-1., 1., 1.));
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1);
    assert!(t[0].len() > 2);
    assert!(t[0].iter().all(|q| q[0] <= 10. + 1e-9), "{:?}", t[0]);
    assert!(proche(t[0][0], [9., 0.]), "{:?}", t[0][0]);
    assert!(proche(*t[0].last().unwrap(), [10., 1.]));
}

#[test]
fn normale_negative() {
    let mut doc = CadDocument::new();
    let mut c = Circle::from_coords(5., 0., 0., 1.);
    c.normal = Vector3::new(0., 0., -1.);
    doc.add_entity(EntityType::Circle(c)).unwrap();
    // Arc OCS de (1,0) à (0,1), normale -Z : réel de (-1,0) à (0,1) par le quart supérieur gauche.
    let mut a = Arc::from_coords(0., 0., 0., 1., 0., FRAC_PI_2);
    a.normal = Vector3::new(0., 0., -1.);
    doc.add_entity(EntityType::Arc(a)).unwrap();
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 2);
    assert!(t[0].iter().all(|q| ((q[0] + 5.).hypot(q[1]) - 1.).abs() < 1e-9), "{:?}", t[0]);
    assert!(proche(t[1][0], [-1., 0.]) && proche(*t[1].last().unwrap(), [0., 1.]), "{:?}", t[1]);
    assert!(t[1].iter().all(|q| q[0] <= 1e-9 && q[1] >= -1e-9));
}

#[test]
fn ellipse_normale_negative() {
    // Centre et grand axe en coordonnées générales ; petit axe = normale × grand axe, ici vers -Y.
    let mut doc = CadDocument::new();
    let mut e = Ellipse::from_center_axes(Vector3::new(5., 0., 0.), Vector3::new(2., 0., 0.), 0.5);
    e.end_parameter = FRAC_PI_2;
    e.normal = Vector3::new(0., 0., -1.);
    doc.add_entity(EntityType::Ellipse(e)).unwrap();
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1);
    assert!(proche(t[0][0], [7., 0.]) && proche(*t[0].last().unwrap(), [5., -1.]), "{:?}", t[0]);
    assert!(t[0].iter().all(|q| q[1] <= 1e-9 && q[0] >= 5. - 1e-9));
}

#[test]
fn insertion_normale_negative() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(1., 0., 2., 0., "0")]);
    inserer(&mut doc, Insert::new("B", Vector3::new(5., 0., 0.)).with_normal(Vector3::new(0., 0., -1.)));
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1);
    assert!(points_proches(t[0], &[[-6., 0.], [-7., 0.]]), "{:?}", t[0]);
}

#[test]
fn normale_inclinee_signalee() {
    let mut doc = CadDocument::new();
    let mut c = Circle::from_coords(5., 0., 0., 1.);
    c.normal = Vector3::new(1., 0., 0.);
    doc.add_entity(EntityType::Circle(c)).unwrap();
    let r = aplatir_doc(doc);
    assert!(r.scene.primitives.is_empty());
    assert_eq!(r.rapport.avertissements.len(), 1, "{:?}", r.rapport.avertissements);
}

#[test]
fn minsert_deux_colonnes() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0")]);
    let mut i = Insert::new("B", Vector3::ZERO).with_rotation(FRAC_PI_2).with_scale(2., 2., 1.);
    i.column_count = 2;
    i.column_spacing = 10.;
    inserer(&mut doc, i);
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 2);
    // L'espacement est tourné mais pas mis à l'échelle.
    assert!(points_proches(t[0], &[[0., 0.], [0., 2.]]), "{:?}", t[0]);
    assert!(points_proches(t[1], &[[0., 10.], [0., 12.]]), "{:?}", t[1]);
}

#[test]
fn bloc_imbrique() {
    let mut doc = CadDocument::new();
    let mut l = Line::from_coords(1., 0., 0., 2., 0., 0.);
    l.common.color = Color::ByBlock;
    definir_bloc(&mut doc, "B", Vector3::new(1., 0., 0.), vec![EntityType::Line(l)]);
    let mut ib = Insert::new("B", Vector3::new(5., 0., 0.));
    ib.common.color = Color::ByBlock;
    definir_bloc(&mut doc, "A", Vector3::ZERO, vec![EntityType::Insert(ib)]);
    let mut ia = Insert::new("A", Vector3::new(100., 0., 0.));
    ia.common.color = Color::Index(1);
    inserer(&mut doc, ia);
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1);
    assert!(points_proches(t[0], &[[105., 0.], [106., 0.]]), "{:?}", t[0]);
    assert_eq!(couleur(&r.scene.primitives[0]), rgba(couleur_aci(1)));
}

/// Bloc A inséré sur `calque` : A contient une LINE sur Visible et une INSERT de B sur le calque 0 ;
/// B contient une LINE sur 0 et une LINE sur Visible.
fn doc_imbrique_sur(calque: &str, eteint: bool, gele: bool) -> CadDocument {
    let mut doc = CadDocument::new();
    ajouter_calque(&mut doc, "Visible", false, false);
    ajouter_calque(&mut doc, calque, eteint, gele);
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0"), ligne(0., 0., 0., 1., "Visible")]);
    definir_bloc(
        &mut doc,
        "A",
        Vector3::ZERO,
        vec![ligne(5., 5., 6., 6., "Visible"), EntityType::Insert(Insert::new("B", Vector3::ZERO))],
    );
    let mut i = Insert::new("A", Vector3::ZERO);
    i.common.layer = calque.into();
    inserer(&mut doc, i);
    doc
}

#[test]
fn calque_eteint_masque_le_calque_0_seulement() {
    let r = aplatir_doc(doc_imbrique_sur("Eteint", true, false));
    let t = traits(&r);
    assert_eq!(t.len(), 2, "{t:?}");
    assert!(points_proches(t[0], &[[5., 5.], [6., 6.]]));
    assert!(points_proches(t[1], &[[0., 0.], [0., 1.]]));
    assert!(r.scene.primitives.iter().all(|p| nom_calque(&r, p) == "Visible"));
}

#[test]
fn calque_gele_masque_tout() {
    let r = aplatir_doc(doc_imbrique_sur("Gele", false, true));
    assert!(r.scene.primitives.is_empty(), "{:?}", r.scene.primitives);
}

#[test]
fn entite_invisible_masquee() {
    let mut doc = CadDocument::new();
    let mut l = Line::from_coords(0., 0., 0., 1., 0., 0.);
    l.common.invisible = true;
    doc.add_entity(EntityType::Line(l)).unwrap();
    assert!(aplatir_doc(doc).scene.primitives.is_empty());
}

#[test]
fn ignorees_comptees() {
    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Text(Text::with_value("A", Vector3::ZERO))).unwrap();
    let d = DimensionLinear::new(Vector3::ZERO, Vector3::new(10., 0., 0.));
    doc.add_entity(EntityType::Dimension(Box::new(Dimension::Linear(d)))).unwrap();
    let r = aplatir_doc(doc);
    assert!(r.scene.primitives.is_empty());
    let attendu: BTreeMap<String, u64> = [("TEXT".to_string(), 1), ("DIMENSION_LINEAR".to_string(), 1)].into();
    assert_eq!(r.rapport.ignorees, attendu);
}

#[test]
fn hachure_a_motif_ignoree() {
    let mut doc = CadDocument::new();
    let mut h = hachure_carree();
    h.is_solid = false;
    doc.add_entity(EntityType::Hatch(Box::new(h))).unwrap();
    let r = aplatir_doc(doc);
    assert!(r.scene.primitives.is_empty());
    assert_eq!(r.rapport.ignorees.get("HATCH"), Some(&1));
}

#[test]
fn degrade_en_couleur_pleine() {
    let mut doc = CadDocument::new();
    let mut h = hachure_carree();
    h.is_solid = false;
    h.common.color = Color::Index(3);
    h.gradient_color.enabled = true;
    h.gradient_color.add_color(0.0, Color::Index(1));
    h.gradient_color.add_color(1.0, Color::Index(5));
    doc.add_entity(EntityType::Hatch(Box::new(h))).unwrap();
    let r = aplatir_doc(doc);
    assert_eq!(r.scene.primitives.len(), 1);
    let Primitive::Surface { triangles, couleur, .. } = &r.scene.primitives[0] else { panic!("surface attendue") };
    assert_eq!(*couleur, rgba(couleur_aci(1)));
    assert!((aire_triangles(triangles) - 100.).abs() < 1e-6);
}

#[test]
fn mpolygon_comme_une_hachure() {
    let mut doc = CadDocument::new();
    let mut h = hachure_carree();
    h.is_mpolygon = true;
    doc.add_entity(EntityType::Hatch(Box::new(h))).unwrap();
    let r = aplatir_doc(doc);
    assert_eq!(r.scene.primitives.len(), 1);
    assert!(matches!(r.scene.primitives[0], Primitive::Surface { .. }));
}

#[test]
fn hachure_normale_negative_en_miroir() {
    let mut doc = CadDocument::new();
    let mut h = hachure_carree();
    h.normal = Vector3::new(0., 0., -1.);
    doc.add_entity(EntityType::Hatch(Box::new(h))).unwrap();
    let r = aplatir_doc(doc);
    let Primitive::Surface { triangles, .. } = &r.scene.primitives[0] else { panic!("surface attendue") };
    assert!(triangles.iter().all(|q| q[0] <= 1e-9 && q[0] >= -10. - 1e-9));
    assert!((aire_triangles(triangles) - 100.).abs() < 1e-6);
}

#[test]
fn ordre_conserve() {
    let mut doc = CadDocument::new();
    for x in [3., 1., 2.] {
        doc.add_entity(ligne(x, 0., x, 1., "0")).unwrap();
    }
    let r = aplatir_doc(doc);
    let premiers: Vec<f64> = traits(&r).iter().map(|t| t[0][0]).collect();
    assert_eq!(premiers, vec![3., 1., 2.]);
}

#[test]
fn polyligne_a_renflement_fermee() {
    let mut doc = CadDocument::new();
    let mut p = LwPolyline::new();
    for x in [0., 10.] {
        let mut v = LwVertex::new(Vector2::new(x, 0.));
        v.bulge = 1.;
        p.vertices.push(v);
    }
    p.is_closed = true;
    doc.add_entity(EntityType::LwPolyline(p)).unwrap();
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1);
    assert!(t[0].len() > 8);
    assert_eq!(t[0][0], *t[0].last().unwrap());
    assert!(t[0].iter().all(|q| ((q[0] - 5.).hypot(q[1]) - 5.).abs() < 1e-9));
}

fn lwpolyligne(sommets: &[(f64, f64, f64, f64)]) -> LwPolyline {
    let mut p = LwPolyline::new();
    for &(x, y, l0, l1) in sommets {
        let mut v = LwVertex::new(Vector2::new(x, y));
        v.start_width = l0;
        v.end_width = l1;
        p.vertices.push(v);
    }
    p
}

#[test]
fn fleche_tige_fine_tete_effilee() {
    let mut doc = CadDocument::new();
    let p = lwpolyligne(&[(0., 0., 0., 0.), (10., 0., 1., 0.), (12., 0., 0., 0.)]);
    doc.add_entity(EntityType::LwPolyline(p)).unwrap();
    let r = aplatir_doc(doc);
    let p = &r.scene.primitives;
    assert_eq!(p.len(), 2, "{p:?}");
    let Primitive::Trait { points, .. } = &p[0] else { panic!("tige : {:?}", p[0]) };
    assert!(points_proches(points, &[[0., 0.], [10., 0.]]), "{points:?}");
    let Primitive::Surface { triangles, .. } = &p[1] else { panic!("tête : {:?}", p[1]) };
    assert!((aire_triangles(triangles) - 1.).abs() < 1e-9);
}

#[test]
fn largeur_du_dernier_sommet_ignoree() {
    let mut doc = CadDocument::new();
    let p = lwpolyligne(&[(0., 0., 0., 0.), (10., 0., 0., 0.), (20., 0., 1., 1.)]);
    doc.add_entity(EntityType::LwPolyline(p)).unwrap();
    let r = aplatir_doc(doc);
    assert_eq!(r.scene.primitives.len(), 1);
    let Primitive::Trait { points, .. } = &r.scene.primitives[0] else { panic!("trait attendu") };
    assert!(points_proches(points, &[[0., 0.], [10., 0.], [20., 0.]]));
}

#[test]
fn polyligne_fermee_mixte() {
    // Carré fermé dont seul le côté droit est large : la suite fine fait le tour par le premier sommet.
    let mut doc = CadDocument::new();
    let mut p = lwpolyligne(&[(0., 0., 0., 0.), (10., 0., 2., 2.), (10., 10., 0., 0.), (0., 10., 0., 0.)]);
    p.is_closed = true;
    doc.add_entity(EntityType::LwPolyline(p)).unwrap();
    let r = aplatir_doc(doc);
    let p = &r.scene.primitives;
    assert_eq!(p.len(), 2, "{p:?}");
    let Primitive::Surface { triangles, .. } = &p[0] else { panic!("côté large : {:?}", p[0]) };
    assert!((aire_triangles(triangles) - 20.).abs() < 1e-9);
    let Primitive::Trait { points, .. } = &p[1] else { panic!("côtés fins : {:?}", p[1]) };
    assert!(points_proches(points, &[[10., 10.], [0., 10.], [0., 0.], [10., 0.]]), "{points:?}");
}

#[test]
fn polyligne_2d_a_largeur_par_sommet() {
    let mut doc = CadDocument::new();
    let mut p = Polyline2D::new();
    let mut a = Vertex2D::new(Vector3::new(0., 0., 0.));
    a.start_width = 2.;
    a.end_width = 0.;
    p.add_vertex(a);
    p.add_vertex(Vertex2D::new(Vector3::new(10., 0., 0.)));
    doc.add_entity(EntityType::Polyline2D(p)).unwrap();
    let r = aplatir_doc(doc);
    assert_eq!(r.scene.primitives.len(), 1);
    let Primitive::Surface { triangles, .. } = &r.scene.primitives[0] else { panic!("surface attendue") };
    assert!((aire_triangles(triangles) - 10.).abs() < 1e-9);
}

#[test]
fn spline_et_polyligne_3d() {
    let mut doc = CadDocument::new();
    let ctrl = vec![Vector3::new(0., 0., 0.), Vector3::new(5., 5., 1.), Vector3::new(10., 0., 2.)];
    doc.add_entity(EntityType::Spline(Spline::from_control_points(1, ctrl))).unwrap();
    let mut p = Polyline3D::new();
    for (x, y) in [(0., 0.), (1., 0.), (1., 1.)] {
        p.add_vertex(Vector3::new(x, y, 3.));
    }
    p.close();
    doc.add_entity(EntityType::Polyline3D(p)).unwrap();
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 2);
    assert!(proche(t[0][0], [0., 0.]) && proche(*t[0].last().unwrap(), [10., 0.]));
    assert!(t[0].iter().any(|q| proche(*q, [5., 5.])));
    assert!(points_proches(t[1], &[[0., 0.], [1., 0.], [1., 1.], [0., 0.]]), "{:?}", t[1]);
}

fn nb_points_cercle(insunits: i16, rayon: f64, echelle: Option<f64>) -> usize {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = insunits;
    let c = EntityType::Circle(Circle::from_coords(0., 0., 0., rayon));
    match echelle {
        None => {
            doc.add_entity(c).unwrap();
        }
        Some(s) => {
            definir_bloc(&mut doc, "C", Vector3::ZERO, vec![c]);
            inserer(&mut doc, Insert::new("C", Vector3::ZERO).with_scale(s, s, 1.));
        }
    }
    traits(&aplatir_doc(doc))[0].len()
}

#[test]
fn tolerance_d_un_millimetre() {
    // 1 mm en millimètres : rayon 1000, flèche 1 → 2 acos(0,999) ≈ 0,0894 rad par segment, 71 segments.
    assert_eq!(nb_points_cercle(4, 1000., None), 72);
    // En mètres, la même géométrie est bien plus fine.
    assert!(nb_points_cercle(6, 1000., None) > 2000);
    // Dans un bloc à l'échelle 10, la tolérance est divisée par 10.
    assert_eq!(nb_points_cercle(4, 100., Some(10.)), 72);
}

#[test]
fn xref_non_resolue_signalee() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "TOPO", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0")]);
    doc.block_records.get_mut("TOPO").unwrap().flags.is_xref = true;
    inserer(&mut doc, Insert::new("TOPO", Vector3::ZERO));
    let r = aplatir_doc(doc);
    assert!(r.scene.primitives.is_empty());
    assert_eq!(r.rapport.avertissements, vec!["xréf non résolue : TOPO".to_string()]);
}

#[test]
fn rapport_et_maitre_inconnu() {
    let mut docs = BTreeMap::new();
    docs.insert("plan.dwg".to_string(), CadDocument::new());
    let dessins = Dessins { docs };
    let r = aplatir(&dessins, "plan.dwg").unwrap();
    assert_eq!(r.rapport.maitre, "plan.dwg");
    assert_eq!(r.rapport.dessins, vec!["plan.dwg".to_string()]);
    match aplatir(&dessins, "autre.dwg") {
        Err(ErreurCad::MaitreInconnu(m)) => assert_eq!(m, "autre.dwg"),
        Err(e) => panic!("erreur inattendue : {e}"),
        Ok(_) => panic!("erreur attendue"),
    }
}

#[test]
fn bloc_auto_insere_sans_panique() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "R", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0")]);
    let h = doc.block_records.get("R").unwrap().handle;
    let mut i = Insert::new("R", Vector3::new(1., 0., 0.));
    i.common.owner_handle = h;
    doc.add_entity(EntityType::Insert(i)).unwrap();
    inserer(&mut doc, Insert::new("R", Vector3::ZERO));
    let r = aplatir_doc(doc);
    assert_eq!(traits(&r).len(), 16);
    assert_eq!(r.rapport.avertissements.len(), 1, "{:?}", r.rapport.avertissements);
}

// --- XCLIP ---

fn bbox(p: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for q in p {
        for k in 0..2 {
            min[k] = min[k].min(q[k]);
            max[k] = max[k].max(q[k]);
        }
    }
    (min, max)
}

fn surfaces(a: &Aplatissement) -> Vec<&Vec<[f64; 2]>> {
    a.scene
        .primitives
        .iter()
        .filter_map(|p| match p {
            Primitive::Surface { triangles, .. } => Some(triangles),
            _ => None,
        })
        .collect()
}

#[test]
fn xclip_coupe_un_trait() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(-5., 5., 15., 5., "0")]);
    let hi = inserer(&mut doc, Insert::new("B", Vector3::ZERO));
    attacher_xclip(&mut doc, hi, filtre(&[(0., 0.), (10., 10.)]));
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1, "{t:?}");
    assert!(points_proches(t[0], &[[0., 5.], [10., 5.]]), "{:?}", t[0]);
}

#[test]
fn xclip_coupe_une_surface() {
    let mut doc = CadDocument::new();
    let mut h = Hatch::solid();
    h.paths = vec![carre(0., 0., 20.)];
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![EntityType::Hatch(Box::new(h))]);
    let hi = inserer(&mut doc, Insert::new("B", Vector3::ZERO));
    attacher_xclip(&mut doc, hi, filtre(&[(0., 0.), (10., 10.)]));
    let r = aplatir_doc(doc);
    let s = surfaces(&r);
    assert_eq!(s.len(), 1, "{:?}", r.scene.primitives);
    assert_eq!(s[0].len() % 3, 0);
    assert!((aire_triangles(s[0]) - 100.).abs() < 1e-6, "{}", aire_triangles(s[0]));
    let (min, max) = bbox(s[0]);
    assert!(proche(min, [0., 0.]) && proche(max, [10., 10.]), "{min:?} {max:?}");
}

#[test]
fn xclip_desactive() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(-5., 5., 15., 5., "0")]);
    let hi = inserer(&mut doc, Insert::new("B", Vector3::ZERO));
    let mut f = filtre(&[(0., 0.), (10., 10.)]);
    f.display_enabled = false;
    attacher_xclip(&mut doc, hi, f);
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1);
    assert!(points_proches(t[0], &[[-5., 5.], [15., 5.]]), "{:?}", t[0]);
}

#[test]
fn xclip_polygonal_ramene_dans_le_bloc() {
    // Contour triangulaire saisi en coordonnées générales autour d'une INSERT placée en (100, 0) :
    // la transformation inverse du bloc le ramène dans le repère du bloc.
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(-5., 2., 15., 2., "0")]);
    let hi = inserer(&mut doc, Insert::new("B", Vector3::new(100., 0., 0.)));
    let mut f = filtre(&[(100., 0.), (110., 0.), (100., 10.)]);
    f.inverse_block_transform = Matrix4::translation(-100., 0., 0.);
    attacher_xclip(&mut doc, hi, f);
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1, "{t:?}");
    assert!(points_proches(t[0], &[[100., 2.], [108., 2.]]), "{:?}", t[0]);
}

#[test]
fn xclip_rectangulaire_d_une_insertion_tournee() {
    // Le rectangle de deux coins est parallèle aux axes du contour, pas à ceux du bloc tourné.
    let mut doc = CadDocument::new();
    let mut h = Hatch::solid();
    h.paths = vec![carre(-100., -100., 200.)];
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![EntityType::Hatch(Box::new(h))]);
    let hi = inserer(&mut doc, Insert::new("B", Vector3::ZERO).with_rotation(FRAC_PI_4));
    let mut f = filtre(&[(0., 0.), (10., 10.)]);
    f.inverse_block_transform = Matrix4::rotation_z(-FRAC_PI_4);
    attacher_xclip(&mut doc, hi, f);
    let r = aplatir_doc(doc);
    let s = surfaces(&r);
    assert_eq!(s.len(), 1);
    assert!((aire_triangles(s[0]) - 100.).abs() < 1e-6, "{}", aire_triangles(s[0]));
    let (min, max) = bbox(s[0]);
    assert!(proche(min, [0., 0.]) && proche(max, [10., 10.]), "{min:?} {max:?}");
}

#[test]
fn xclip_imbriques_se_composent() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(-5., 5., 15., 5., "0")]);
    let hb = definir_bloc(&mut doc, "A", Vector3::ZERO, vec![EntityType::Insert(Insert::new("B", Vector3::ZERO))])[0];
    attacher_xclip(&mut doc, hb, filtre(&[(0., 0.), (10., 10.)]));
    let ha = inserer(&mut doc, Insert::new("A", Vector3::ZERO));
    attacher_xclip(&mut doc, ha, filtre(&[(5., 0.), (20., 10.)]));
    let r = aplatir_doc(doc);
    let t = traits(&r);
    assert_eq!(t.len(), 1, "{t:?}");
    assert!(points_proches(t[0], &[[5., 5.], [10., 5.]]), "{:?}", t[0]);
}

// --- Blocs dynamiques ---

/// Bloc dynamique `nom` à deux états de visibilité, A (première LINE) puis B (seconde LINE),
/// inséré directement ; `etat` est l'état enregistré dans le cache d'évaluation de l'INSERT.
fn doc_dynamique(nom: &str, etat: Option<&str>) -> CadDocument {
    let mut doc = CadDocument::new();
    let h = definir_bloc(&mut doc, nom, Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0"), ligne(0., 5., 1., 5., "0")]);
    let bloc = doc.block_records.get(nom).unwrap().handle;
    let mut p = BlockVisibilityParameter::default();
    p.handle = doc.allocate_handle();
    p.owner = bloc;
    p.eval_node_id = 7;
    p.all_blocks = h.clone();
    for (n, e) in [("A", h[0]), ("B", h[1])] {
        p.states.push(BlockVisibilityState { name: n.into(), visible_blocks: vec![e], visible_params: vec![] });
    }
    doc.block_visibility_params.insert(p.handle, p);
    let hi = inserer(&mut doc, Insert::new(nom, Vector3::ZERO));
    if let Some(etat) = etat {
        let mut d = doc.ensure_extension_dictionary(hi);
        for cle in ["AcDbBlockRepresentation", "AppDataCache", "ACAD_ENHANCEDBLOCKDATA"] {
            d = sous_dictionnaire(&mut doc, d, cle);
        }
        let mut x = XRecord::new();
        x.handle = doc.allocate_handle();
        x.owner = d;
        x.add_string(1, etat);
        let hx = x.handle;
        doc.objects.insert(hx, ObjectType::XRecord(x));
        entree(&mut doc, d, "7", hx);
    }
    doc
}

fn ordonnees(r: &Aplatissement) -> Vec<f64> {
    traits(r).iter().map(|t| t[0][1]).collect()
}

#[test]
fn visibilite_dynamique() {
    let r = aplatir_doc(doc_dynamique("PORTE", Some("B")));
    assert_eq!(ordonnees(&r), vec![5.]);
}

#[test]
fn visibilite_dynamique_etat_par_defaut() {
    // Sans état enregistré, AutoCAD affiche le premier état de la liste.
    let r = aplatir_doc(doc_dynamique("PORTE", None));
    assert_eq!(ordonnees(&r), vec![0.]);
}

#[test]
fn visibilite_dynamique_bloc_anonyme_non_filtre() {
    // Une représentation anonyme porte déjà l'état évalué : rien n'est filtré.
    let r = aplatir_doc(doc_dynamique("*U12", Some("B")));
    assert_eq!(ordonnees(&r), vec![0., 5.]);
}

// --- SORTENTS ---

#[test]
fn sortents_reordonne() {
    let mut doc = CadDocument::new();
    let h: Vec<Handle> = [1., 2., 3.].iter().map(|&x| doc.add_entity(ligne(x, 0., x, 1., "0")).unwrap()).collect();
    let espace_objet = doc.block_records.get("*Model_Space").unwrap().handle;
    // Permutation de clés à la manière d'AutoCAD : la 3e LINE prend la clé de la 1re, et ainsi de suite.
    attacher_sortents(&mut doc, espace_objet, &[(h[2], h[0]), (h[0], h[1]), (h[1], h[2])]);
    let r = aplatir_doc(doc);
    let premiers: Vec<f64> = traits(&r).iter().map(|t| t[0][0]).collect();
    assert_eq!(premiers, vec![3., 1., 2.]);
}

#[test]
fn sortents_dans_un_bloc() {
    let mut doc = CadDocument::new();
    let h = definir_bloc(&mut doc, "B", Vector3::ZERO, (1..=3).map(|x| ligne(x as f64, 0., x as f64, 1., "0")).collect());
    let bloc = doc.block_records.get("B").unwrap().handle;
    // Entité absente de la table : sa propre clé. Clé nulle : sa propre clé aussi.
    attacher_sortents(&mut doc, bloc, &[(h[2], Handle::new(1)), (h[0], Handle::NULL)]);
    inserer(&mut doc, Insert::new("B", Vector3::ZERO));
    let r = aplatir_doc(doc);
    let premiers: Vec<f64> = traits(&r).iter().map(|t| t[0][0]).collect();
    assert_eq!(premiers, vec![3., 1., 2.]);
}

#[test]
fn xclip_garde_le_trou_d_une_hachure() {
    let mut doc = CadDocument::new();
    let mut h = Hatch::solid();
    h.paths = vec![carre(0., 0., 20.), carre(4., 4., 2.)];
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![EntityType::Hatch(Box::new(h))]);
    let hi = inserer(&mut doc, Insert::new("B", Vector3::ZERO));
    attacher_xclip(&mut doc, hi, filtre(&[(0., 0.), (10., 10.)]));
    let r = aplatir_doc(doc);
    let s = surfaces(&r);
    assert_eq!(s.len(), 1);
    assert!((aire_triangles(s[0]) - 96.).abs() < 1e-6, "{}", aire_triangles(s[0]));
}
