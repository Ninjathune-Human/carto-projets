//! Extraction des lignes de l'espace objet et format binaire `.lignes`.
use acadrust::{CadDocument, EntityType};

/// Suite de points (x, y) à relier par des segments.
#[derive(Debug, Clone, PartialEq)]
pub struct Polyligne {
    pub points: Vec<[f64; 2]>,
}

/// LINE et sommets des LWPOLYLINE de l'espace objet ; renflements ignorés en phase 0.
/// Une LWPOLYLINE fermée répète son premier sommet ; moins de 2 sommets : ignorée.
pub fn extraire_lignes(doc: &CadDocument) -> Vec<Polyligne> {
    let mut sortie = Vec::new();
    for e in doc.model_space_entities() {
        match e {
            EntityType::Line(l) => sortie.push(Polyligne { points: vec![[l.start.x, l.start.y], [l.end.x, l.end.y]] }),
            EntityType::LwPolyline(p) if p.vertices.len() >= 2 => {
                let mut points: Vec<[f64; 2]> = p.vertices.iter().map(|v| [v.location.x, v.location.y]).collect();
                if p.is_closed {
                    points.push(points[0]);
                }
                sortie.push(Polyligne { points });
            }
            _ => {}
        }
    }
    sortie
}

/// Format `.lignes` (petit-boutiste) : `LIG0`, u32 nombre de polylignes,
/// puis pour chacune u32 n et n × (f64 x, f64 y).
pub fn ecrire_lignes(polys: &[Polyligne]) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(b"LIG0");
    o.extend_from_slice(&(polys.len() as u32).to_le_bytes());
    for p in polys {
        o.extend_from_slice(&(p.points.len() as u32).to_le_bytes());
        for [x, y] in &p.points {
            o.extend_from_slice(&x.to_le_bytes());
            o.extend_from_slice(&y.to_le_bytes());
        }
    }
    o
}
