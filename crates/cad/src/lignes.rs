//! Extraction des lignes de l'espace objet et format binaire `.lignes`.
use acadrust::entities::Insert;
use acadrust::types::Transform;
use acadrust::{CadDocument, EntityType};

/// Garde contre les blocs qui s'insèrent eux-mêmes.
const PROFONDEUR_MAX: usize = 16;

/// Suite de points (x, y) à relier par des segments.
#[derive(Debug, Clone, PartialEq)]
pub struct Polyligne {
    pub points: Vec<[f64; 2]>,
}

/// LINE, sommets des LWPOLYLINE et des POLYLINE de l'espace objet, y compris dans les blocs
/// insérés (point de base, échelle, rotation, réseaux MINSERT, blocs imbriqués).
/// Renflements ignorés. Une polyligne fermée répète son premier sommet ; moins de 2 sommets : ignorée.
/// Calques éteints ou gelés écartés ; un bloc sur un calque gelé est écarté en entier, et dans un
/// bloc les objets du calque 0 prennent le calque de l'insertion.
pub fn extraire_lignes(doc: &CadDocument) -> Vec<Polyligne> {
    let mut sortie = Vec::new();
    for e in doc.model_space_entities() {
        ajouter(doc, e, 0, &mut sortie);
    }
    sortie
}

fn ajouter(doc: &CadDocument, e: &EntityType, profondeur: usize, sortie: &mut Vec<Polyligne>) {
    let calque = doc.layers.get(&e.common().layer);
    if let EntityType::Insert(i) = e {
        if profondeur < PROFONDEUR_MAX && !calque.is_some_and(|c| c.is_frozen()) {
            for partie in exploser(doc, i) {
                ajouter(doc, &partie, profondeur + 1, sortie);
            }
        }
        return;
    }
    if calque.is_some_and(|c| c.is_off() || c.is_frozen()) {
        return;
    }
    let (points, fermee): (Vec<[f64; 2]>, bool) = match e {
        EntityType::Line(l) => (vec![[l.start.x, l.start.y], [l.end.x, l.end.y]], false),
        EntityType::LwPolyline(p) => (p.vertices.iter().map(|v| [v.location.x, v.location.y]).collect(), p.is_closed),
        EntityType::Polyline2D(p) => (p.vertices.iter().map(|v| [v.location.x, v.location.y]).collect(), p.is_closed()),
        EntityType::Polyline(p) => (p.vertices.iter().map(|v| [v.location.x, v.location.y]).collect(), p.is_closed()),
        EntityType::Polyline3D(p) => (p.vertices.iter().map(|v| [v.position.x, v.position.y]).collect(), p.is_closed()),
        _ => return,
    };
    if points.len() < 2 {
        return;
    }
    let mut points = points;
    if fermee {
        points.push(points[0]);
    }
    sortie.push(Polyligne { points });
}

/// Contenu du bloc ramené à son point de base, puis placé par `Insert::explode`
/// (calque 0 hérité, réseaux MINSERT). Les xréfs ne sont pas résolues ici.
fn exploser(doc: &CadDocument, i: &Insert) -> Vec<EntityType> {
    let Some(br) = doc.block_records.get(&i.block_name) else { return Vec::new() };
    if br.flags.is_xref || br.flags.is_xref_overlay || br.flags.is_external || !br.xref_path.is_empty() {
        return Vec::new();
    }
    let base = match doc.get_entity(br.block_entity_handle) {
        Some(EntityType::Block(b)) => b.base_point,
        _ => br.base_point,
    };
    let vers_base = Transform::from_translation(base * -1.0);
    let contenu: Vec<EntityType> = doc
        .entities_in_block(&i.block_name)
        .map(|e| {
            let mut e = e.clone();
            e.as_entity_mut().apply_transform(&vers_base);
            e
        })
        .collect();
    i.explode(&contenu)
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
