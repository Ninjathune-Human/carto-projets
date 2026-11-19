use acadrust::{CadDocument, EntityType, Line, LwPolyline, Vector2};
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
