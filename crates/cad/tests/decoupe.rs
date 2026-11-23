use carto_cad::decoupe::{contour_xclip, Zone};

fn aire(t: &[[f64; 2]]) -> f64 {
    t.chunks(3)
        .map(|c| ((c[1][0] - c[0][0]) * (c[2][1] - c[0][1]) - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1])).abs() / 2.0)
        .sum()
}

fn carre(x0: f64, y0: f64, c: f64) -> Zone {
    Zone::nouvelle(&contour_xclip(&[[x0, y0], [x0 + c, y0 + c]])).unwrap()
}

#[test]
fn deux_coins_font_un_rectangle() {
    assert_eq!(contour_xclip(&[[0., 0.], [2., 1.]]), vec![[0., 0.], [2., 0.], [2., 1.], [0., 1.]]);
    let triangle = [[0., 0.], [1., 0.], [0., 1.]];
    assert_eq!(contour_xclip(&triangle), triangle.to_vec());
}

#[test]
fn contour_sans_aire_sans_effet() {
    assert!(Zone::nouvelle(&[[0., 0.], [1., 1.], [2., 2.]]).is_none());
    assert!(Zone::nouvelle(&[[0., 0.], [1., f64::NAN], [2., 0.]]).is_none());
}

#[test]
fn interieur_garde_tel_quel_exterieur_supprime_bord_garde() {
    let z = carre(0., 0., 10.);
    let dedans = vec![[1.1, 1.3], [2.7, 9.9], [5., 5.]];
    assert_eq!(z.couper_trait(&dedans), vec![dedans.clone()]);
    assert!(z.couper_trait(&[[20., 0.], [30., 5.]]).is_empty());
    assert!(z.couper_trait(&[[11., -5.], [11., 15.]]).is_empty());
    assert_eq!(z.couper_trait(&[[0., 2.], [0., 8.]]).len(), 1);
}

#[test]
fn zone_non_convexe_coupe_en_deux() {
    // U ouvert vers le haut : un trait horizontal qui traverse l'échancrure sort en deux morceaux.
    let u = [[0., 0.], [9., 0.], [9., 9.], [6., 9.], [6., 3.], [3., 3.], [3., 9.], [0., 9.]];
    let z = Zone::nouvelle(&u).unwrap();
    let mut m = z.couper_trait(&[[-1., 6.], [10., 6.]]);
    assert_eq!(m.len(), 2, "{m:?}");
    m.sort_by(|a, b| a[0][0].min(a[1][0]).total_cmp(&b[0][0].min(b[1][0])));
    let longueurs: Vec<f64> = m.iter().map(|p| (p[1][0] - p[0][0]).abs()).collect();
    assert!(longueurs.iter().all(|l| (l - 3.).abs() < 1e-9), "{m:?}");
}

#[test]
fn intersection_de_zones() {
    let a = carre(0., 0., 10.);
    let plein = [[0., 0.], [20., 0.], [20., 20.], [0., 0.], [20., 20.], [0., 20.]];
    assert!((aire(&a.inter(&carre(5., 5., 10.)).couper_surface(&plein)) - 25.).abs() < 1e-9);
    let vide = a.inter(&carre(20., 20., 1.));
    assert!(vide.est_vide());
    assert!(vide.couper_trait(&[[0., 0.], [10., 10.]]).is_empty());
}

#[test]
fn surface_a_triangles_qui_se_recouvrent() {
    // Deux triangles de sens opposés qui se recouvrent : leur réunion, ni trou ni double compte.
    let z = carre(0., 0., 10.);
    let t = [[-5., 0.], [5., 0.], [5., 10.], [0., 0.], [0., 10.], [8., 0.]];
    let s = z.couper_surface(&t);
    assert_eq!(s.len() % 3, 0);
    assert!(s.iter().all(|p| p[0] >= -1e-9 && p[0] <= 10. + 1e-9));
    // Dans le carré : 37,5 (premier, coupé à x = 0) + 40 (second) - 2075/72 (partie commune).
    assert!((aire(&s) - 3505. / 72.).abs() < 1e-9, "{}", aire(&s));
}

#[test]
fn points_aberrants_ecartes() {
    let z = carre(0., 0., 10.);
    // Les segments qui touchent un point aberrant disparaissent, le reste est découpé normalement.
    let mut m = z.couper_trait(&[[-5., 1.], [5., 1.], [1e200, 1.], [5., 3.], [15., 3.]]);
    m.sort_by(|a, b| a[0][1].total_cmp(&b[0][1]));
    assert_eq!(m.len(), 2, "{m:?}");
    assert!(m[0].iter().all(|p| p[1] == 1.) && m[1].iter().all(|p| p[1] == 3.), "{m:?}");
    // Les triangles qui touchent un point aberrant disparaissent.
    let s = z.couper_surface(&[[0., 0.], [20., 0.], [0., 10.], [1., 1.], [1e200, 1.], [1., 2.]]);
    assert!((aire(&s) - 75.).abs() < 1e-9, "{}", aire(&s));
    // Contour aberrant : découpe sans effet.
    assert!(Zone::nouvelle(&[[0., 0.], [1e200, 0.], [0., 1.]]).is_none());
}

#[test]
fn zone_inversee_garde_l_exterieur_et_le_bord() {
    let z = Zone::inversee(&contour_xclip(&[[0., 0.], [10., 10.]])).unwrap();
    assert!(!z.est_vide());
    assert!(z.couper_trait(&[[2., 2.], [8., 8.]]).is_empty(), "dedans : masqué");
    assert_eq!(z.couper_trait(&[[20., 0.], [30., 5.]]), vec![vec![[20., 0.], [30., 5.]]], "dehors : gardé tel quel");
    assert_eq!(z.couper_trait(&[[0., 2.], [0., 8.]]).len(), 1, "sur le bord : gardé");
    let morceaux = z.couper_trait(&[[-5., 5.], [15., 5.]]);
    assert_eq!(morceaux.len(), 2, "{morceaux:?}");
    // Surface : le carré 20 × 20 privé du carré 10 × 10.
    let carre20 = [[0., 0.], [20., 0.], [20., 20.], [0., 0.], [20., 20.], [0., 20.]];
    assert!((aire(&z.couper_surface(&carre20)) - 300.).abs() < 1e-6);
    assert!(Zone::inversee(&[[0., 0.], [1., 1.], [2., 2.]]).is_none(), "sans aire : sans effet");
}
