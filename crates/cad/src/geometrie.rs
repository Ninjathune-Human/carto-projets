//! Géométrie pure : discrétisation des courbes, bandes de polyligne, triangulation.
//! Aucune fonction ne panique sur une entrée dégénérée.

use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};
use std::f64::consts::TAU;

const MAX_SEGMENTS: usize = 4096;

/// Nombre de segments pour balayer `angle` sur un rayon `r` avec une flèche au plus `tol`.
fn nb_segments(angle: f64, r: f64, tol: f64) -> usize {
    if !(angle.is_finite() && r.is_finite()) || r <= 0.0 {
        return 1;
    }
    let pas = if tol.is_finite() && tol > 0.0 {
        2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos()
    } else {
        0.0
    };
    if pas <= 0.0 {
        return MAX_SEGMENTS;
    }
    let n = (angle.abs() / pas).ceil();
    if n.is_nan() {
        1
    } else {
        (n as usize).clamp(1, MAX_SEGMENTS)
    }
}

/// Arc de cercle, extrémités incluses (radians, sens trigonométrique).
pub fn arc_points(centre: [f64; 2], rayon: f64, debut: f64, fin: f64, tol: f64) -> Vec<[f64; 2]> {
    if !(rayon.is_finite() && rayon > 0.0 && debut.is_finite() && fin.is_finite()) {
        return Vec::new();
    }
    let mut fin = fin;
    if fin < debut {
        fin += TAU;
    }
    let n = nb_segments(fin - debut, rayon, tol);
    (0..=n)
        .map(|i| {
            let a = debut + (fin - debut) * i as f64 / n as f64;
            [centre[0] + rayon * a.cos(), centre[1] + rayon * a.sin()]
        })
        .collect()
}

/// Segment à renflement (bulge) de `a` à `b` : points sans `a`, avec `b`.
pub fn renflement_points(a: [f64; 2], b: [f64; 2], renflement: f64, tol: f64) -> Vec<[f64; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let d = dx.hypot(dy);
    if !renflement.is_finite() || renflement == 0.0 || !d.is_finite() || d == 0.0 {
        return vec![b];
    }
    let theta = 4.0 * renflement.atan();
    let r = d * (1.0 + renflement * renflement) / (4.0 * renflement.abs());
    let h = d * (1.0 - renflement * renflement) / (4.0 * renflement);
    let (gx, gy) = (-dy / d, dx / d);
    let c = [(a[0] + b[0]) / 2.0 + gx * h, (a[1] + b[1]) / 2.0 + gy * h];
    let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
    // Deux moitiés (h + 1 puis h segments) : le point milieu de l'arc est le point
    // du milieu de la liste (longueur 2h + 1).
    let h = nb_segments(theta, r, tol).div_ceil(2).min(MAX_SEGMENTS / 2);
    let pt = |t: f64| [c[0] + r * t.cos(), c[1] + r * t.sin()];
    let mut v: Vec<[f64; 2]> = (1..=h)
        .map(|k| pt(a0 + theta / 2.0 * k as f64 / (h + 1) as f64))
        .collect();
    v.push(pt(a0 + theta / 2.0));
    v.extend((1..=h).map(|k| pt(a0 + theta / 2.0 + theta / 2.0 * k as f64 / h as f64)));
    if let Some(dernier) = v.last_mut() {
        *dernier = b;
    }
    v
}

/// Arc d'ellipse paramétrique, extrémités incluses.
pub fn ellipse_points(
    centre: [f64; 2],
    grand_axe: [f64; 2],
    rapport: f64,
    t0: f64,
    t1: f64,
    tol: f64,
) -> Vec<[f64; 2]> {
    let r = grand_axe[0].hypot(grand_axe[1]);
    if !(r.is_finite() && r > 0.0 && rapport.is_finite() && t0.is_finite() && t1.is_finite()) {
        return Vec::new();
    }
    let t1 = if t1 < t0 { t1 + TAU } else { t1 };
    let v = [-grand_axe[1] * rapport, grand_axe[0] * rapport];
    let n = nb_segments(t1 - t0, r, tol);
    (0..=n)
        .map(|i| {
            let t = t0 + (t1 - t0) * i as f64 / n as f64;
            let (s, c) = t.sin_cos();
            [
                centre[0] + grand_axe[0] * c + v[0] * s,
                centre[1] + grand_axe[1] * c + v[1] * s,
            ]
        })
        .collect()
}

/// Spline B-spline (de Boor), NURBS si `poids` est fourni.
pub fn spline_points(
    degre: usize,
    noeuds: &[f64],
    controles: &[[f64; 2]],
    poids: Option<&[f64]>,
    tol: f64,
) -> Vec<[f64; 2]> {
    let m = controles.len();
    if m == 0 {
        return Vec::new();
    }
    if m == 1 {
        return vec![controles[0]];
    }
    let p = degre.clamp(1, m - 1);
    let attendu = m + p + 1;
    let croissants = noeuds.len() == attendu
        && noeuds.iter().all(|x| x.is_finite())
        && noeuds.windows(2).all(|w| w[0] <= w[1]);
    let u: Vec<f64> = if croissants {
        noeuds.to_vec()
    } else {
        let interieur = m - p - 1;
        let mut u = vec![0.0; p + 1];
        u.extend((1..=interieur).map(|j| j as f64 / (interieur + 1) as f64));
        u.extend(std::iter::repeat(1.0).take(p + 1));
        u
    };
    let w: Vec<f64> = match poids {
        Some(w) if w.len() == m && w.iter().all(|x| x.is_finite()) => w.to_vec(),
        _ => vec![1.0; m],
    };
    let (u0, u1) = (u[p], u[m]);
    if !(u1 > u0) {
        return vec![controles[0], controles[m - 1]];
    }
    let longueur: f64 = controles
        .windows(2)
        .map(|s| (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1]))
        .sum();
    let n = if tol.is_finite() && tol > 0.0 && longueur.is_finite() {
        ((longueur / tol).sqrt().ceil() as usize).max(4 * (m - 1))
    } else {
        MAX_SEGMENTS
    }
    .min(MAX_SEGMENTS);
    let n = n + (n & 1); // pair : le milieu du domaine est échantillonné
    let mut ts: Vec<f64> = (0..=n).map(|i| u0 + (u1 - u0) * i as f64 / n as f64).collect();
    ts.extend(u.iter().copied().filter(|&x| x > u0 && x < u1));
    ts.sort_by(|a, b| a.total_cmp(b));
    ts.dedup();
    if let Some(d) = ts.last_mut() {
        *d = u1;
    }
    let mut out: Vec<[f64; 2]> = ts.iter().map(|&t| de_boor(p, &u, controles, &w, t)).collect();
    out[0] = controles[0];
    if let Some(d) = out.last_mut() {
        *d = controles[m - 1];
    }
    out
}

fn de_boor(p: usize, u: &[f64], c: &[[f64; 2]], w: &[f64], t: f64) -> [f64; 2] {
    let m = c.len();
    let mut k = (u.partition_point(|&x| x <= t).saturating_sub(1)).clamp(p, m - 1);
    while k > p && u[k] >= u[k + 1] {
        k -= 1;
    }
    let mut d: Vec<[f64; 3]> = (0..=p)
        .map(|j| {
            let i = k - p + j;
            [c[i][0] * w[i], c[i][1] * w[i], w[i]]
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = k - p + j;
            let den = u[i + p - r + 1] - u[i];
            let a = if den > 0.0 { (t - u[i]) / den } else { 0.0 };
            for q in 0..3 {
                d[j][q] = (1.0 - a) * d[j - 1][q] + a * d[j][q];
            }
        }
    }
    let h = d[p];
    if h[2].abs() > 0.0 {
        [h[0] / h[2], h[1] / h[2]]
    } else {
        c[k]
    }
}

type V2 = [f64; 2];

fn tri(out: &mut Vec<V2>, a: V2, b: V2, c: V2) {
    out.extend([a, b, c]);
}

struct Seg {
    a: V2,
    b: V2,
    d: V2, // direction unitaire
    w0: f64,
    w1: f64,
}

/// Bande d'une polyligne à largeurs variables, en triangles.
/// `largeurs[i]` = (début, fin) du segment i ; joints en onglet limités à 4 x la demi-largeur.
pub fn bande(points: &[[f64; 2]], largeurs: &[(f64, f64)], fermee: bool) -> Vec<[f64; 2]> {
    let n = points.len();
    let mut out = Vec::new();
    if n < 2 {
        return out;
    }
    let nb = if fermee { n } else { n - 1 };
    let mut segs: Vec<Seg> = Vec::new();
    for i in 0..nb {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy);
        if !(l.is_finite() && l > 0.0) {
            continue;
        }
        let (w0, w1) = largeurs
            .get(i)
            .or(largeurs.last())
            .copied()
            .unwrap_or((0.0, 0.0));
        if !(w0.is_finite() && w1.is_finite()) {
            continue;
        }
        segs.push(Seg { a, b, d: [dx / l, dy / l], w0, w1 });
    }
    for s in &segs {
        let nrm = [-s.d[1], s.d[0]];
        let (h0, h1) = (s.w0 / 2.0, s.w1 / 2.0);
        let a1 = [s.a[0] + nrm[0] * h0, s.a[1] + nrm[1] * h0];
        let a2 = [s.a[0] - nrm[0] * h0, s.a[1] - nrm[1] * h0];
        let b1 = [s.b[0] + nrm[0] * h1, s.b[1] + nrm[1] * h1];
        let b2 = [s.b[0] - nrm[0] * h1, s.b[1] - nrm[1] * h1];
        tri(&mut out, a1, a2, b2);
        tri(&mut out, a1, b2, b1);
    }
    let joint = |out: &mut Vec<V2>, s0: &Seg, s1: &Seg| {
        let (h0, h1) = (s0.w1 / 2.0, s1.w0 / 2.0);
        let cross = s0.d[0] * s1.d[1] - s0.d[1] * s1.d[0];
        if cross.abs() < 1e-12 || (h0 <= 0.0 && h1 <= 0.0) {
            return;
        }
        let side = if cross > 0.0 { -1.0 } else { 1.0 };
        let (n0, n1) = ([-s0.d[1] * side, s0.d[0] * side], [-s1.d[1] * side, s1.d[0] * side]);
        let v = s0.b;
        let pa = [v[0] + n0[0] * h0, v[1] + n0[1] * h0];
        let pb = [v[0] + n1[0] * h1, v[1] + n1[1] * h1];
        let m = [n0[0] + n1[0], n0[1] + n1[1]];
        let ml = m[0].hypot(m[1]);
        let h = (h0 + h1) / 2.0;
        if ml > 1e-9 {
            let mh = [m[0] / ml, m[1] / ml];
            let cos = mh[0] * n0[0] + mh[1] * n0[1];
            if cos > 1e-9 && h / cos <= 4.0 * h {
                let l = h / cos;
                let tip = [v[0] + mh[0] * l, v[1] + mh[1] * l];
                tri(out, v, pa, tip);
                tri(out, v, tip, pb);
                return;
            }
        }
        tri(out, v, pa, pb);
    };
    for k in 1..segs.len() {
        joint(&mut out, &segs[k - 1], &segs[k]);
    }
    if fermee && segs.len() > 1 {
        joint(&mut out, &segs[segs.len() - 1], &segs[0]);
    }
    out
}

/// Triangulation pair-impair d'anneaux (trous compris) ; liste plate de sommets de triangles.
pub fn trianguler(anneaux: &[Vec<[f64; 2]>]) -> Vec<[f64; 2]> {
    let mut min = [f64::INFINITY; 2];
    for a in anneaux {
        for p in a.iter().filter(|p| p[0].is_finite() && p[1].is_finite()) {
            min[0] = min[0].min(p[0]);
            min[1] = min[1].min(p[1]);
        }
    }
    if !min[0].is_finite() {
        return Vec::new();
    }
    // lyon travaille en f32 : on se ramène près de l'origine pour garder la précision.
    let mut b = Path::builder();
    let mut vides = true;
    for a in anneaux {
        let pts: Vec<_> = a
            .iter()
            .filter(|p| p[0].is_finite() && p[1].is_finite())
            .collect();
        if pts.len() < 3 {
            continue;
        }
        vides = false;
        b.begin(point((pts[0][0] - min[0]) as f32, (pts[0][1] - min[1]) as f32));
        for p in &pts[1..] {
            b.line_to(point((p[0] - min[0]) as f32, (p[1] - min[1]) as f32));
        }
        b.end(true);
    }
    if vides {
        return Vec::new();
    }
    let chemin = b.build();
    let mut tampon: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let options = FillOptions::default().with_fill_rule(FillRule::EvenOdd);
    let res = FillTessellator::new().tessellate_path(
        &chemin,
        &options,
        &mut BuffersBuilder::new(&mut tampon, |v: FillVertex| v.position().to_array()),
    );
    if res.is_err() {
        return Vec::new();
    }
    tampon
        .indices
        .iter()
        .map(|&i| {
            let v = tampon.vertices[i as usize];
            [v[0] as f64 + min[0], v[1] as f64 + min[1]]
        })
        .collect()
}
