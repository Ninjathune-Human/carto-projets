//! Sonde jetable (ignoree) : sens des aretes horaires de hachures sur le corpus local.
use acadrust::entities::hatch::BoundaryEdge;
use acadrust::entities::EntityType;
use std::path::Path;

fn fin(e: &BoundaryEdge) -> Option<[f64; 2]> {
    match e {
        BoundaryEdge::Line(l) => Some([l.end.x, l.end.y]),
        BoundaryEdge::Polyline(p) if !p.is_closed => p.vertices.last().map(|v| [v.x, v.y]),
        _ => None,
    }
}
fn debut(e: &BoundaryEdge) -> Option<[f64; 2]> {
    match e {
        BoundaryEdge::Line(l) => Some([l.start.x, l.start.y]),
        BoundaryEdge::Polyline(p) if !p.is_closed => p.vertices.first().map(|v| [v.x, v.y]),
        _ => None,
    }
}
fn d(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[test]
#[ignore]
fn sonde() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/zips");
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let (mut cw, mut reel, mut miroir, mut les_deux, mut aucun, mut ell_cw, mut ell_ok, mut ell_ko) = (0, 0, 0, 0, 0, 0, 0, 0);
    for f in rd.flatten() {
        let Ok(octets) = std::fs::read(f.path()) else { continue };
        let Ok(inv) = carto_cad::inventorier(&octets) else { continue };
        for dwg in &inv.dwg {
            let Ok(b) = carto_cad::lire_entree(&octets, dwg) else { continue };
            let Ok(doc) = carto_cad::lire_document(&b) else { continue };
            for e in doc.entities() {
                let EntityType::Hatch(h) = e else { continue };
                for p in &h.paths {
                    let n = p.edges.len();
                    for i in 0..n {
                        let pr = (i + n - 1) % n;
                        let sv = (i + 1) % n;
                        let (prev, next) = (fin(&p.edges[pr]), debut(&p.edges[sv]));
                        if prev.is_none() && next.is_none() { continue; }
                        match &p.edges[i] {
                            BoundaryEdge::CircularArc(a) if !a.counter_clockwise => {
                                cw += 1;
                                let c = [a.center.x, a.center.y];
                                let pt = |t: f64| [c[0] + a.radius * t.cos(), c[1] + a.radius * t.sin()];
                                let tol = 1e-6 * a.radius.max(1.0);
                                let ok = |s: [f64; 2], e: [f64; 2]| {
                                    prev.map_or(true, |q| d(q, s) < tol) && next.map_or(true, |q| d(q, e) < tol)
                                };
                                let r = ok(pt(a.start_angle), pt(a.end_angle));
                                let m = ok(pt(-a.start_angle), pt(-a.end_angle));
                                match (r, m) { (true, true) => les_deux += 1, (true, false) => reel += 1, (false, true) => miroir += 1, _ => aucun += 1 }
                            }
                            BoundaryEdge::EllipticArc(a) if !a.counter_clockwise => {
                                ell_cw += 1;
                                let c = [a.center.x, a.center.y];
                                let m = [a.major_axis_endpoint.x, a.major_axis_endpoint.y];
                                let pt = |t: f64| [c[0] + m[0] * t.cos() - m[1] * a.minor_axis_ratio * t.sin(), c[1] + m[1] * t.cos() + m[0] * a.minor_axis_ratio * t.sin()];
                                let tol = 1e-6 * m[0].hypot(m[1]).max(1.0);
                                let ok = |s: [f64; 2], e: [f64; 2]| prev.map_or(true, |q| d(q, s) < tol) && next.map_or(true, |q| d(q, e) < tol);
                                if ok(pt(-a.start_angle), pt(-a.end_angle)) { ell_ok += 1 } else if ok(pt(a.start_angle), pt(a.end_angle)) { ell_ko += 1 }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    println!("SONDE arcs horaires voisins-testables={cw} reel={reel} miroir={miroir} ambigus={les_deux} aucun={aucun}; ellipses horaires={ell_cw} miroir_ok={ell_ok} reel_ok={ell_ko}");
}
