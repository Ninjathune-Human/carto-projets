use acadrust::{CadDocument, Circle, DwgWriter, EntityType, Line};
use carto_cad::{emprise_depuis, mesurer};

fn dwg_de_test() -> Vec<u8> {
    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Line(Line::from_coords(0., 0., 0., 10., 0., 0.))).unwrap();
    doc.add_entity(EntityType::Line(Line::from_coords(0., 0., 0., 0., 10., 0.))).unwrap();
    doc.add_entity(EntityType::Circle(Circle::from_coords(5., 5., 0., 2.))).unwrap();
    DwgWriter::write_to_vec(&doc).unwrap()
}

#[test]
fn compte_par_type_dans_l_espace_objet() {
    let m = mesurer(&dwg_de_test()).unwrap();
    assert_eq!(m.par_type.get("LINE"), Some(&2));
    assert_eq!(m.par_type.get("CIRCLE"), Some(&1));
    assert_eq!((m.total, m.inconnues), (3, 0));
}

#[test]
fn dwg_tronque_ne_panique_pas() {
    let o = dwg_de_test();
    match mesurer(&o[..o.len() / 2]) {
        Err(_) => {}
        Ok(m) => assert!(m.total < 3 || m.erreurs_recuperees > 0),
    }
}

#[test]
fn emprise_absente_si_en_tete_non_renseigne() {
    assert_eq!(emprise_depuis([1e20, 1e20], [-1e20, -1e20]), None);
    assert_eq!(emprise_depuis([1.0, 2.0], [3.0, 4.0]), Some([1.0, 2.0, 3.0, 4.0]));
}

#[test]
#[ignore]
fn generer_fixture() {
    std::fs::write("tests/fixtures/deux_lignes_un_cercle.dwg", dwg_de_test()).unwrap();
}
