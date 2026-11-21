use acadrust::entities::hatch::{
    BoundaryEdge, BoundaryPath, CircularArcEdge, EllipticArcEdge, LineEdge, PolylineEdge, SplineEdge,
};
use acadrust::entities::Hatch;
use acadrust::types::{Vector2, Vector3};
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

fn proche(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

#[test]
fn arc_horaire() {
    // Convention acadrust (confirmee sur un DWG reel) : angles horaires stockes en miroir.
    // Stocke 0 -> PI/2, horaire : reel de (5,0) vers (0,-5), par le quart inferieur droit.
    // Lue comme angles reels, la meme arete irait de (5,0) a (0,5) par 270 degres.
    let mut p = BoundaryPath::new();
    p.edges.push(demi(0.0, PI / 2.0, false));
    p.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(0.0, -5.0),
        end: Vector2::new(0.0, 0.0),
    }));
    p.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(0.0, 0.0),
        end: Vector2::new(5.0, 0.0),
    }));
    let r = anneaux_hachure(&hachure(vec![p]), TOL);
    assert_eq!(r.len(), 1);
    let a = &r[0];
    assert!(proche(a[0], [5.0, 0.0]));
    assert!(a.iter().all(|q| q[0] > -1e-9 && q[1] < 1e-9));
    assert!((aire_anneau(a) - PI * 25.0 / 4.0).abs() < 0.05);
    let i = a.iter().position(|q| proche(*q, [0.0, -5.0])).expect("fin de l'arc");
    assert!(a[..=i].iter().all(|q| (q[0].hypot(q[1]) - 5.0).abs() < 1e-9));
}

#[test]
fn ellipse_horaire_axe_tourne() {
    // Grand axe (0,4), rapport 0,5, centre (10,20) ; point(t) = c + (-2 sin t, 4 cos t).
    // Horaire, parametres stockes 0,3 -> 1,2 : reel de point(-0,3) a point(-1,2).
    let mut p = BoundaryPath::new();
    p.edges.push(BoundaryEdge::EllipticArc(EllipticArcEdge {
        center: Vector2::new(10.0, 20.0),
        major_axis_endpoint: Vector2::new(0.0, 4.0),
        minor_axis_ratio: 0.5,
        start_angle: 0.3,
        end_angle: 1.2,
        counter_clockwise: false,
    }));
    let pt = |t: f64| [10.0 - 2.0 * f64::sin(t), 20.0 + 4.0 * f64::cos(t)];
    let r = anneaux_hachure(&hachure(vec![p]), TOL);
    assert_eq!(r.len(), 1);
    let a = &r[0];
    assert!(proche(a[0], pt(-0.3)));
    assert!(proche(*a.last().unwrap(), pt(-1.2)));
    for q in a {
        let (x, y) = ((q[0] - 10.0) / 2.0, (q[1] - 20.0) / 4.0);
        assert!((x * x + y * y - 1.0).abs() < 1e-9);
    }
}

fn spline(poids_milieu: f64, rationnelle: bool) -> Hatch {
    let mut p = BoundaryPath::new();
    p.edges.push(BoundaryEdge::Spline(SplineEdge {
        degree: 2,
        rational: rationnelle,
        periodic: false,
        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        control_points: vec![
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(1.0, 2.0, poids_milieu),
            Vector3::new(2.0, 0.0, 1.0),
        ],
        fit_points: vec![],
        start_tangent: Vector2::new(0.0, 0.0),
        end_tangent: Vector2::new(0.0, 0.0),
    }));
    hachure(vec![p])
}

fn y_max(h: &Hatch) -> f64 {
    anneaux_hachure(h, TOL)[0].iter().map(|q| q[1]).fold(f64::MIN, f64::max)
}

#[test]
fn spline_rationnelle() {
    let sans = y_max(&spline(1.0, false));
    assert!((sans - 1.0).abs() < 1e-6, "Bezier quadratique : sommet a y = 1");
    // z ignore quand la spline n'est pas rationnelle
    assert!((y_max(&spline(5.0, false)) - sans).abs() < 1e-12);
    // poids 5 au milieu : le sommet monte a 2*5/(1+5) = 1,666...
    assert!((y_max(&spline(5.0, true)) - 10.0 / 6.0).abs() < 1e-3);
}

#[test]
fn polyligne_renflee() {
    let mut e = PolylineEdge::new(vec![], false);
    e.add_vertex(Vector2::new(0.0, 0.0), 1.0);
    e.add_vertex(Vector2::new(10.0, 0.0), 0.0);
    let mut p = BoundaryPath::new();
    p.edges.push(BoundaryEdge::Polyline(e));
    let r = anneaux_hachure(&hachure(vec![p]), TOL);
    assert_eq!(r.len(), 1);
    let a = &r[0];
    assert!(proche(a[0], [0.0, 0.0]));
    assert!(proche(*a.last().unwrap(), [10.0, 0.0]));
    assert!(a.len() > 8);
    for q in a {
        assert!(q[1] < 1e-9, "renflement positif : demi-cercle sous la corde");
        assert!(((q[0] - 5.0).hypot(q[1]) - 5.0).abs() < 1e-9);
    }
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
