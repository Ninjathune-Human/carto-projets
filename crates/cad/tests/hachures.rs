use acadrust::entities::hatch::{
    BoundaryEdge, BoundaryPath, CircularArcEdge, LineEdge, PolylineEdge,
};
use acadrust::entities::Hatch;
use acadrust::types::Vector2;
use carto_cad::hachures::anneaux_hachure;
use carto_cad::trianguler;
use std::f64::consts::PI;

const TOL: f64 = 0.001;

fn aire_anneau(a: &[[f64; 2]]) -> f64 {
    let n = a.len();
    (0..n)
        .map(|i| a[i][0] * a[(i + 1) % n][1] - a[(i + 1) % n][0] * a[i][1])
        .sum::<f64>()
        .abs()
        / 2.0
}

fn aire_triangles(t: &[[f64; 2]]) -> f64 {
    t.chunks(3)
        .map(|c| {
            ((c[1][0] - c[0][0]) * (c[2][1] - c[0][1]) - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1])).abs() / 2.0
        })
        .sum()
}

fn carre(x0: f64, y0: f64, c: f64) -> BoundaryPath {
    let mut p = BoundaryPath::new();
    p.edges.push(BoundaryEdge::Polyline(PolylineEdge::new(
        vec![
            Vector2::new(x0, y0),
            Vector2::new(x0 + c, y0),
            Vector2::new(x0 + c, y0 + c),
            Vector2::new(x0, y0 + c),
        ],
        true,
    )));
    p
}

fn hachure(chemins: Vec<BoundaryPath>) -> Hatch {
    let mut h = Hatch::new();
    h.paths = chemins;
    h
}

fn demi(debut: f64, fin: f64, ccw: bool) -> BoundaryEdge {
    BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(0.0, 0.0),
        radius: 5.0,
        start_angle: debut,
        end_angle: fin,
        counter_clockwise: ccw,
    })
}

#[test]
fn chemin_polyligne_carre() {
    let r = anneaux_hachure(&hachure(vec![carre(0.0, 0.0, 10.0)]), TOL);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].len(), 4);
    assert!((aire_anneau(&r[0]) - 100.0).abs() < 1e-9);
}

#[test]
fn chemin_arcs() {
    let mut p = BoundaryPath::new();
    p.edges.push(demi(0.0, PI, true));
    p.edges.push(demi(PI, 2.0 * PI, true));
    let r = anneaux_hachure(&hachure(vec![p]), TOL);
    assert_eq!(r.len(), 1);
    assert!(r[0].len() > 8);
    for q in &r[0] {
        assert!((q[0].hypot(q[1]) - 5.0).abs() < 1e-9);
    }
}

#[test]
fn arc_horaire() {
    // Convention acadrust : angles d'un arc horaire stockés en miroir (point réel à -theta).
    // 0 -> PI horaire = demi-cercle inférieur (y < 0), de (5,0) vers (-5,0).
    let mut p = BoundaryPath::new();
    p.edges.push(demi(0.0, PI, false));
    p.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(-5.0, 0.0),
        end: Vector2::new(5.0, 0.0),
    }));
    let r = anneaux_hachure(&hachure(vec![p]), TOL);
    assert_eq!(r.len(), 1);
    let a = &r[0];
    assert!((a[0][0] - 5.0).abs() < 1e-9 && a[0][1].abs() < 1e-9);
    assert!(a.iter().all(|q| q[1] < 1e-9));
    assert!(a.iter().any(|q| q[1] < -4.9));
    assert!((aire_anneau(a) - PI * 25.0 / 2.0).abs() < 0.05);
}

#[test]
fn ilot() {
    let r = anneaux_hachure(&hachure(vec![carre(0.0, 0.0, 10.0), carre(2.0, 2.0, 6.0)]), TOL);
    assert_eq!(r.len(), 2);
    assert!((aire_triangles(&trianguler(&r)) - 64.0).abs() < 1e-9);
}

#[test]
fn chemin_degenere_ecarte() {
    let mut p = BoundaryPath::new();
    p.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(0.0, 0.0),
        end: Vector2::new(1.0, 0.0),
    }));
    p.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(1.0, 0.0),
        end: Vector2::new(0.0, 0.0),
    }));
    assert!(anneaux_hachure(&hachure(vec![p]), TOL).is_empty());
}
