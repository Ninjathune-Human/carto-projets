use acadrust::entities::EntityCommon;
use acadrust::tables::Layer;
use acadrust::{CadDocument, Color, LineWeight, Transparency};
use carto_cad::proprietes::{calque_gele, calque_visible, couleur_aci, metres_par_unite, resoudre, Herite};

fn ajouter_calque(doc: &mut CadDocument, nom: &str, couleur: Color) {
    let mut c = Layer::new(nom);
    c.color = couleur;
    doc.layers.add(c).unwrap();
}

fn commun(calque: &str) -> EntityCommon {
    let mut e = EntityCommon::default();
    e.layer = calque.into();
    e
}

fn parent(calque: &str, couleur: [u8; 3], epaisseur_mm: f32, alpha: u8) -> Herite {
    Herite { calque: calque.into(), couleur, epaisseur_mm, alpha }
}

#[test]
fn couleur_7_et_bybloc_premier_niveau_en_noir() {
    let doc = CadDocument::new();
    let mut e = commun("X");
    e.color = Color::Index(7);
    assert_eq!(resoudre(&doc, &e, None).couleur, [0, 0, 0]);
    e.color = Color::ByBlock;
    assert_eq!(resoudre(&doc, &e, None).couleur, [0, 0, 0]);
    e.color = Color::None;
    assert_eq!(resoudre(&doc, &e, None).couleur, [0, 0, 0]);
}

#[test]
fn couleurs_explicites() {
    let doc = CadDocument::new();
    let mut e = commun("X");
    e.color = Color::Index(1);
    assert_eq!(resoudre(&doc, &e, None).couleur, couleur_aci(1));
    e.color = Color::Rgb { r: 10, g: 20, b: 30 };
    assert_eq!(resoudre(&doc, &e, None).couleur, [10, 20, 30]);
}

#[test]
fn bycalque_prend_la_couleur_du_calque() {
    let mut doc = CadDocument::new();
    ajouter_calque(&mut doc, "Murs", Color::Index(1));
    let e = commun("Murs");
    let h = resoudre(&doc, &e, None);
    assert_eq!(h.couleur, couleur_aci(1));
    assert_eq!(h.calque, "Murs");
}

#[test]
fn calque_0_herite_du_parent() {
    let mut doc = CadDocument::new();
    ajouter_calque(&mut doc, "Murs", Color::Index(1));
    let p = parent("Murs", couleur_aci(1), 0., 255);
    let h = resoudre(&doc, &commun("0"), Some(&p));
    assert_eq!(h.calque, "Murs");
    assert_eq!(h.couleur, couleur_aci(1));
}

#[test]
fn bybloc_herite_du_parent() {
    let doc = CadDocument::new();
    let p = parent("Murs", [1, 2, 3], 0.5, 200);
    let mut e = commun("Autre");
    e.color = Color::ByBlock;
    e.line_weight = LineWeight::ByBlock;
    e.transparency = Transparency::ByBlock;
    let h = resoudre(&doc, &e, Some(&p));
    assert_eq!(h.couleur, [1, 2, 3]);
    assert_eq!(h.epaisseur_mm, 0.5);
    assert_eq!(h.alpha, 200);
    assert_eq!(h.calque, "Autre");
}

#[test]
fn epaisseur_value_en_mm() {
    let doc = CadDocument::new();
    let mut e = commun("X");
    e.line_weight = LineWeight::Value(35);
    assert!((resoudre(&doc, &e, None).epaisseur_mm - 0.35).abs() < 1e-6);
    e.line_weight = LineWeight::Default;
    assert_eq!(resoudre(&doc, &e, None).epaisseur_mm, 0.0);
}

#[test]
fn epaisseur_bycalque() {
    let mut doc = CadDocument::new();
    let mut c = Layer::new("L");
    c.line_weight = LineWeight::Value(50);
    doc.layers.add(c).unwrap();
    assert!((resoudre(&doc, &commun("L"), None).epaisseur_mm - 0.5).abs() < 1e-6);
}

#[test]
fn transparence_en_alpha() {
    let doc = CadDocument::new();
    let mut e = commun("X");
    e.transparency = Transparency::from_percent(0.0);
    assert_eq!(resoudre(&doc, &e, None).alpha, 255);
    e.transparency = Transparency::from_percent(0.5);
    let a = resoudre(&doc, &e, None).alpha as i32;
    assert!((a - 128).abs() <= 1, "alpha {a}");
    e.transparency = Transparency::ByLayer;
    assert_eq!(resoudre(&doc, &e, None).alpha, 255);
}

#[test]
fn calques_masques() {
    let mut doc = CadDocument::new();
    let mut eteint = Layer::new("Eteint");
    eteint.turn_off();
    let mut gele = Layer::new("Gele");
    gele.freeze();
    let mut np = Layer::new("NonImp");
    np.is_plottable = false;
    for c in [eteint, gele, np, Layer::new("defpoints"), Layer::new("Ok")] {
        doc.layers.add(c).unwrap();
    }
    for n in ["Eteint", "Gele", "NonImp", "defpoints", "DEFPOINTS", "Defpoints"] {
        assert!(!calque_visible(&doc, n), "{n}");
    }
    assert!(calque_visible(&doc, "Ok"));
    assert!(calque_visible(&doc, "Absent"));
    assert!(calque_gele(&doc, "Gele"));
    assert!(!calque_gele(&doc, "Eteint"));
    assert!(!calque_gele(&doc, "Absent"));
}

#[test]
fn unites() {
    assert_eq!(metres_par_unite(5), 0.01);
    assert_eq!(metres_par_unite(4), 0.001);
    assert_eq!(metres_par_unite(0), 1.0);
    assert_eq!(metres_par_unite(1), 0.0254);
    assert_eq!(metres_par_unite(2), 0.3048);
    assert_eq!(metres_par_unite(6), 1.0);
}
