use carto_cad::{arc_points, bande, ellipse_points, renflement_points, spline_points, trianguler};

fn aire(t: &[[f64; 2]]) -> f64 {
    t.chunks(3)
        .map(|c| {
            ((c[1][0] - c[0][0]) * (c[2][1] - c[0][1]) - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1])).abs() / 2.0
        })
        .sum()
}

fn proche(a: [f64; 2], b: [f64; 2], e: f64) -> bool {
    (a[0] - b[0]).abs() < e && (a[1] - b[1]).abs() < e
}

#[test]
fn quart_de_cercle() {
    let p = arc_points([0.0, 0.0], 10.0, 0.0, std::f64::consts::FRAC_PI_2, 0.001);
    assert!(proche(p[0], [10.0, 0.0], 1e-9));
    assert!(proche(*p.last().unwrap(), [0.0, 10.0], 1e-9));
    for q in &p {
        assert!((q[0].hypot(q[1]) - 10.0).abs() < 1e-9);
    }
    for w in p.windows(2) {
        let m = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
        assert!(10.0 - m[0].hypot(m[1]) <= 0.001 + 1e-12);
    }
}

#[test]
fn renflement_1_demi_cercle() {
    let p = renflement_points([0.0, 0.0], [2.0, 0.0], 1.0, 0.001);
    assert!(proche(p[p.len() / 2], [1.0, -1.0], 1e-9));
    assert!(proche(*p.last().unwrap(), [2.0, 0.0], 1e-9));
    assert_eq!(renflement_points([0.0, 0.0], [2.0, 0.0], 0.0, 0.001), vec![[2.0, 0.0]]);
}

#[test]
fn ellipse_complete() {
    let p = ellipse_points([0.0, 0.0], [2.0, 0.0], 0.5, 0.0, std::f64::consts::TAU, 0.001);
    assert!(p.len() > 8);
    for q in &p {
        assert!(((q[0] / 2.0).powi(2) + q[1].powi(2) - 1.0).abs() < 1e-9);
    }
}

#[test]
fn spline_degre_2() {
    let c = [[0.0, 0.0], [1.0, 2.0], [2.0, 0.0]];
    let p = spline_points(2, &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0], &c, None, 0.001);
    assert!(proche(p[0], [0.0, 0.0], 1e-9));
    assert!(proche(*p.last().unwrap(), [2.0, 0.0], 1e-9));
    assert!(p.iter().any(|q| proche(*q, [1.0, 1.0], 1e-6)));
    let q = spline_points(2, &[], &c, None, 0.001);
    assert!(proche(*q.last().unwrap(), [2.0, 0.0], 1e-9));
}

#[test]
fn spline_degre_1_rend_les_controles() {
    let c = [[0.0, 0.0], [1.0, 2.0], [3.0, 1.0]];
    let p = spline_points(1, &[0.0, 0.0, 0.5, 1.0, 1.0], &c, None, 0.001);
    for k in &c {
        assert!(p.iter().any(|q| proche(*q, *k, 1e-9)));
    }
    assert!(proche(p[0], c[0], 1e-9) && proche(*p.last().unwrap(), c[2], 1e-9));
}

#[test]
fn bande_segment() {
    let t = bande(&[[0.0, 0.0], [10.0, 0.0]], &[(2.0, 2.0)], false);
    assert_eq!(t.len() % 3, 0);
    assert!((aire(&t) - 20.0).abs() < 1e-9);
    assert!(t.iter().all(|q| q[1] >= -1.0 - 1e-9 && q[1] <= 1.0 + 1e-9));
}

#[test]
fn ilot_reste_vide() {
    let ext = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let trou = vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]];
    let t = trianguler(&[ext, trou]);
    assert_eq!(t.len() % 3, 0);
    assert!((aire(&t) - 64.0).abs() < 1e-9);
    for c in t.chunks(3) {
        let b = [(c[0][0] + c[1][0] + c[2][0]) / 3.0, (c[0][1] + c[1][1] + c[2][1]) / 3.0];
        assert!(!(b[0] > 2.0 && b[0] < 8.0 && b[1] > 2.0 && b[1] < 8.0));
    }
}

#[test]
fn degenere_sans_panique() {
    assert!(arc_points([0.0, 0.0], 0.0, 0.0, 1.0, 0.001).len() <= 2);
    assert!(trianguler(&[]).is_empty());
    assert!(trianguler(&[vec![]]).is_empty());
    assert!(bande(&[], &[], false).is_empty());
    assert!(bande(&[[1.0, 1.0], [1.0, 1.0]], &[(1.0, 1.0)], false).len() % 3 == 0);
    let _ = spline_points(3, &[], &[], None, 0.001);
    let _ = ellipse_points([0.0; 2], [0.0; 2], 0.5, 0.0, 1.0, 0.001);
}

#[test]
fn ilot_proche_reste_vide() {
    let ext = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let trou = vec![[0.01, 0.01], [9.99, 0.01], [9.99, 9.99], [0.01, 9.99]];
    let t = trianguler(&[ext, trou]);
    assert_eq!(t.len() % 3, 0);
    assert!((aire(&t) - (100.0 - 9.98 * 9.98)).abs() < 1e-6);
    for c in t.chunks(3) {
        let b = [(c[0][0] + c[1][0] + c[2][0]) / 3.0, (c[0][1] + c[1][1] + c[2][1]) / 3.0];
        assert!(!(b[0] > 0.01 && b[0] < 9.99 && b[1] > 0.01 && b[1] < 9.99));
    }
}

#[test]
fn spline_non_serree() {
    let c = [[0.0, 0.0], [1.0, 2.0], [2.0, 0.0]];
    let p = spline_points(2, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0], &c, None, 0.001);
    assert!(proche(p[0], [0.5, 1.0], 1e-9));
    assert!(proche(*p.last().unwrap(), [1.5, 1.0], 1e-9));
}
