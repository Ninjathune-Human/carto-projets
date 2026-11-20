//! Contours de hachures : arêtes de chemins de limite converties en anneaux fermés.
//!
//! Convention acadrust (vérifiée dans ses sources) : angles en radians ; pour une arête
//! horaire (`counter_clockwise == false`) les angles sont stockés en miroir, le point réel
//! d'angle stocké `t` est à `-t`, et l'arc réel va dans le sens horaire de `-début` à `-fin`,
//! soit l'arc trigonométrique de `-fin` à `-début` parcouru à l'envers. Même règle pour les
//! paramètres d'une arête elliptique. Pour une polyligne, `z` porte le renflement du segment
//! sortant ; pour une spline, `z` porte le poids si `rational`.

use crate::geometrie::{arc_points, ellipse_points, renflement_points, spline_points};
use acadrust::entities::hatch::{BoundaryEdge, BoundaryPath};
use acadrust::entities::Hatch;

const EPS: f64 = 1e-9;

fn egaux(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() <= EPS && (a[1] - b[1]).abs() <= EPS
}

fn pousser(anneau: &mut Vec<[f64; 2]>, p: [f64; 2]) {
    if !(p[0].is_finite() && p[1].is_finite()) {
        return;
    }
    if anneau.last().map_or(true, |d| !egaux(*d, p)) {
        anneau.push(p);
    }
}

/// Arc circulaire d'une arête, dans l'ordre début -> fin de l'arête.
fn arc_arete(centre: [f64; 2], rayon: f64, debut: f64, fin: f64, antihoraire: bool, tol: f64) -> Vec<[f64; 2]> {
    if antihoraire {
        arc_points(centre, rayon, debut, fin, tol)
    } else {
        let mut v = arc_points(centre, rayon, -fin, -debut, tol);
        v.reverse();
        v
    }
}

fn points_arete(arete: &BoundaryEdge, tol: f64) -> Vec<[f64; 2]> {
    match arete {
        BoundaryEdge::Line(l) => vec![[l.start.x, l.start.y], [l.end.x, l.end.y]],
        BoundaryEdge::CircularArc(a) => arc_arete(
            [a.center.x, a.center.y],
            a.radius,
            a.start_angle,
            a.end_angle,
            a.counter_clockwise,
            tol,
        ),
        BoundaryEdge::EllipticArc(e) => {
            let c = [e.center.x, e.center.y];
            let axe = [e.major_axis_endpoint.x, e.major_axis_endpoint.y];
            if e.counter_clockwise {
                ellipse_points(c, axe, e.minor_axis_ratio, e.start_angle, e.end_angle, tol)
            } else {
                let mut v = ellipse_points(c, axe, e.minor_axis_ratio, -e.end_angle, -e.start_angle, tol);
                v.reverse();
                v
            }
        }
        BoundaryEdge::Spline(s) => {
            if s.control_points.len() >= 2 {
                let ctrl: Vec<[f64; 2]> = s.control_points.iter().map(|p| [p.x, p.y]).collect();
                let poids: Vec<f64> = s.control_points.iter().map(|p| p.z).collect();
                let degre = usize::try_from(s.degree).unwrap_or(3).max(1);
                let poids = if s.rational && poids.iter().all(|w| w.is_finite() && *w > 0.0) {
                    Some(poids.as_slice())
                } else {
                    None
                };
                spline_points(degre, &s.knots, &ctrl, poids, tol)
            } else {
                s.fit_points.iter().map(|p| [p.x, p.y]).collect()
            }
        }
        BoundaryEdge::Polyline(p) => {
            let n = p.vertices.len();
            let mut v = Vec::new();
            if n == 0 {
                return v;
            }
            v.push([p.vertices[0].x, p.vertices[0].y]);
            let segments = if p.is_closed { n } else { n - 1 };
            for i in 0..segments {
                let a = p.vertices[i];
                let b = p.vertices[(i + 1) % n];
                v.extend(renflement_points([a.x, a.y], [b.x, b.y], a.z, tol));
            }
            v
        }
    }
}

fn anneau_chemin(chemin: &BoundaryPath, tol: f64) -> Option<Vec<[f64; 2]>> {
    let mut anneau: Vec<[f64; 2]> = Vec::new();
    for arete in &chemin.edges {
        for p in points_arete(arete, tol) {
            pousser(&mut anneau, p);
        }
    }
    while anneau.len() > 1 && egaux(anneau[0], anneau[anneau.len() - 1]) {
        anneau.pop();
    }
    if anneau.len() < 3 {
        None
    } else {
        Some(anneau)
    }
}

/// Un anneau fermé (premier point non répété) par chemin de limite ; les chemins
/// de moins de 3 points distincts sont écartés.
pub fn anneaux_hachure(h: &Hatch, tol: f64) -> Vec<Vec<[f64; 2]>> {
    h.paths.iter().filter_map(|c| anneau_chemin(c, tol)).collect()
}
