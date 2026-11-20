use std::f64::consts::FRAC_PI_2;

use acadrust::entities::{Block, Insert, Polyline2D, Vertex2D};
use acadrust::tables::{BlockRecord, Layer};
use acadrust::{CadDocument, EntityType, Line, LwPolyline, Vector2, Vector3};
use carto_cad::{ecrire_lignes, extraire_lignes, Polyligne};

fn doc_avec_deux_lignes_et_une_lwpolyligne_de_trois_sommets() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Line(Line::from_coords(0., 0., 0., 10., 0., 0.))).unwrap();
    doc.add_entity(EntityType::Line(Line::from_coords(0., 0., 0., 0., 10., 0.))).unwrap();
    let p = LwPolyline::from_points(vec![Vector2::new(1., 1.), Vector2::new(2., 2.), Vector2::new(3., 1.)]);
    doc.add_entity(EntityType::LwPolyline(p)).unwrap();
    doc
}

#[test]
fn extrait_lignes_et_lwpolylignes() {
    let doc = doc_avec_deux_lignes_et_une_lwpolyligne_de_trois_sommets();
    let p = extraire_lignes(&doc);
    assert_eq!(p.iter().map(|l| l.points.len()).collect::<Vec<_>>(), vec![2, 2, 3]);
}

#[test]
fn lwpolyligne_fermee_repete_le_premier_sommet() {
    let mut doc = CadDocument::new();
    let mut p = LwPolyline::from_points(vec![Vector2::new(1., 1.), Vector2::new(2., 2.), Vector2::new(3., 1.)]);
    p.is_closed = true;
    doc.add_entity(EntityType::LwPolyline(p)).unwrap();
    let l = extraire_lignes(&doc);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].points.len(), 4);
    assert_eq!(l[0].points[3], l[0].points[0]);
}

fn proche(obtenu: &[[f64; 2]], attendu: &[[f64; 2]]) -> bool {
    obtenu.len() == attendu.len()
        && obtenu.iter().zip(attendu).all(|(a, b)| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9)
}

fn ligne(x1: f64, y1: f64, x2: f64, y2: f64, calque: &str) -> EntityType {
    let mut l = Line::from_coords(x1, y1, 0., x2, y2, 0.);
    l.common.layer = calque.into();
    EntityType::Line(l)
}

fn definir_bloc(doc: &mut CadDocument, nom: &str, base: Vector3, contenu: Vec<EntityType>) {
    let mut record = BlockRecord::new(nom);
    record.handle = doc.allocate_handle();
    let proprietaire = record.handle;
    doc.block_records.add(record).unwrap();
    let mut bloc = Block::new(nom, base);
    bloc.common.owner_handle = proprietaire;
    let h = doc.add_entity(EntityType::Block(bloc)).unwrap();
    doc.block_records.get_mut(nom).unwrap().block_entity_handle = h;
    for mut e in contenu {
        e.common_mut().owner_handle = proprietaire;
        doc.add_entity(e).unwrap();
    }
}

fn inserer(doc: &mut CadDocument, i: Insert) {
    doc.add_entity(EntityType::Insert(i)).unwrap();
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

#[test]
fn polyligne_2d_fermee_repete_le_premier_sommet() {
    let mut doc = CadDocument::new();
    let mut p = Polyline2D::new();
    for (x, y) in [(1., 1.), (2., 2.), (3., 1.)] {
        p.add_vertex(Vertex2D::new(Vector3::new(x, y, 0.)));
    }
    p.close();
    doc.add_entity(EntityType::Polyline2D(p)).unwrap();
    let l = extraire_lignes(&doc);
    assert_eq!(l.len(), 1);
    assert!(proche(&l[0].points, &[[1., 1.], [2., 2.], [3., 1.], [1., 1.]]));
}

#[test]
fn bloc_insere_avec_point_de_base_rotation_et_echelle() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::new(1., 0., 0.), vec![ligne(1., 0., 2., 0., "0")]);
    inserer(&mut doc, Insert::new("B", Vector3::new(10., 20., 0.)).with_rotation(FRAC_PI_2).with_scale(2., 2., 1.));
    let l = extraire_lignes(&doc);
    assert_eq!(l.len(), 1);
    assert!(proche(&l[0].points, &[[10., 20.], [10., 22.]]), "{:?}", l[0].points);
}

#[test]
fn bloc_imbrique() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::new(1., 0., 0.), vec![ligne(1., 0., 2., 0., "0")]);
    definir_bloc(&mut doc, "A", Vector3::ZERO, vec![EntityType::Insert(Insert::new("B", Vector3::new(5., 0., 0.)))]);
    inserer(&mut doc, Insert::new("A", Vector3::new(100., 0., 0.)));
    let l = extraire_lignes(&doc);
    assert_eq!(l.len(), 1);
    assert!(proche(&l[0].points, &[[105., 0.], [106., 0.]]), "{:?}", l[0].points);
}

#[test]
fn minsert_produit_une_copie_par_case() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0")]);
    let mut i = Insert::new("B", Vector3::ZERO);
    i.column_count = 2;
    i.column_spacing = 10.;
    inserer(&mut doc, i);
    let mut l = extraire_lignes(&doc);
    l.sort_by(|a, b| a.points[0][0].total_cmp(&b.points[0][0]));
    assert_eq!(l.len(), 2);
    assert!(proche(&l[1].points, &[[10., 0.], [11., 0.]]), "{:?}", l[1].points);
}

#[test]
fn bloc_non_decomposable_affiche() {
    let mut doc = CadDocument::new();
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0")]);
    doc.block_records.get_mut("B").unwrap().explodable = false;
    inserer(&mut doc, Insert::new("B", Vector3::ZERO));
    assert_eq!(extraire_lignes(&doc).len(), 1);
}

#[test]
fn calques_eteints_et_geles_ecartes() {
    let mut doc = CadDocument::new();
    ajouter_calque(&mut doc, "Visible", false, false);
    ajouter_calque(&mut doc, "Eteint", true, false);
    ajouter_calque(&mut doc, "Gele", false, true);
    for c in ["Visible", "Eteint", "Gele"] {
        doc.add_entity(ligne(0., 0., 1., 0., c)).unwrap();
    }
    assert_eq!(extraire_lignes(&doc).len(), 1);
}

#[test]
fn bloc_sur_calque_gele_entierement_ecarte() {
    let mut doc = CadDocument::new();
    ajouter_calque(&mut doc, "Visible", false, false);
    ajouter_calque(&mut doc, "Gele", false, true);
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0"), ligne(0., 0., 0., 1., "Visible")]);
    let mut i = Insert::new("B", Vector3::ZERO);
    i.common.layer = "Gele".into();
    inserer(&mut doc, i);
    assert!(extraire_lignes(&doc).is_empty());
}

#[test]
fn bloc_sur_calque_eteint_ne_masque_que_le_calque_0() {
    let mut doc = CadDocument::new();
    ajouter_calque(&mut doc, "Visible", false, false);
    ajouter_calque(&mut doc, "Eteint", true, false);
    definir_bloc(&mut doc, "B", Vector3::ZERO, vec![ligne(0., 0., 1., 0., "0"), ligne(0., 0., 0., 1., "Visible")]);
    let mut i = Insert::new("B", Vector3::ZERO);
    i.common.layer = "Eteint".into();
    inserer(&mut doc, i);
    let l = extraire_lignes(&doc);
    assert_eq!(l.len(), 1);
    assert!(proche(&l[0].points, &[[0., 0.], [0., 1.]]), "{:?}", l[0].points);
}

fn deux_polylignes() -> Vec<Polyligne> {
    vec![
        Polyligne { points: vec![[0., 0.], [1., 1.]] },
        Polyligne { points: vec![[2., 2.], [3., 3.], [4., 4.]] },
    ]
}

#[test]
fn format_lignes() {
    let o = ecrire_lignes(&deux_polylignes());
    assert_eq!(&o[..4], b"LIG0");
    assert_eq!(o.len(), 96);
}

#[test]
#[ignore]
fn generer_fixture() {
    std::fs::write("../../essais/carte/test/deux_polylignes.lignes", ecrire_lignes(&deux_polylignes())).unwrap();
}
