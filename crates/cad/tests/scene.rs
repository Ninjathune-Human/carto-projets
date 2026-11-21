use carto_cad::scene::{ecrire_scene, Primitive, Rapport, Scene};

fn u32_le(o: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(o[p..p + 4].try_into().unwrap())
}

fn scene_deux() -> Scene {
    let mut s = Scene::default();
    let a = s.calque("A", [255, 0, 0]);
    s.primitives.push(Primitive::Trait { calque: a, couleur: [255, 0, 0, 255], epaisseur_mm: 0.25, points: vec![[0.0, 0.0], [1.0, 2.0]] });
    s.primitives.push(Primitive::Surface { calque: a, couleur: [0, 0, 255, 128], triangles: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]] });
    s
}

fn rapport() -> Rapport {
    Rapport { maitre: "plan.dwg".into(), ..Default::default() }
}

#[test]
fn scene_vide_fait_12_octets_plus_rapport() {
    let o = ecrire_scene(&Scene::default(), &Rapport::default());
    assert_eq!(&o[0..4], b"SCN1");
    assert_eq!(u32_le(&o, 4), 0);
    assert_eq!(u32_le(&o, 8), 0);
    let n = u32_le(&o, 12) as usize;
    assert_eq!(o.len(), 16 + n);
    let json: serde_json::Value = serde_json::from_slice(&o[16..]).unwrap();
    assert_eq!(json["maitre"], "");
    assert!(json["emprise"].is_null(), "emprise absente : null");
}

#[test]
fn emprise_du_rapport_en_tableau() {
    let r = Rapport { emprise: Some([1.0, 2.0, 3.5, 4.0]), ..rapport() };
    let o = ecrire_scene(&Scene::default(), &r);
    let json: serde_json::Value = serde_json::from_slice(&o[16..]).unwrap();
    assert_eq!(json["emprise"], serde_json::json!([1.0, 2.0, 3.5, 4.0]));
}

#[test]
fn facteur_du_rapport_nombre_ou_null() {
    let o = ecrire_scene(&Scene::default(), &Rapport::default());
    let json: serde_json::Value = serde_json::from_slice(&o[16..]).unwrap();
    assert_eq!(json.get("facteur"), Some(&serde_json::Value::Null), "facteur inconnu : null");
    let r = Rapport { facteur: Some(0.01), ..rapport() };
    let o = ecrire_scene(&Scene::default(), &r);
    let json: serde_json::Value = serde_json::from_slice(&o[16..]).unwrap();
    assert_eq!(json["facteur"], serde_json::json!(0.01));
}

#[test]
fn trait_et_surface_serialises() {
    let o = ecrire_scene(&scene_deux(), &rapport());
    // 4 signature + 4 calques + (4 + 1 nom + 4 couleur) + 4 primitives
    let debut = 4 + 4 + 9 + 4;
    assert_eq!(u32_le(&o, 4), 1);
    assert_eq!(u32_le(&o, 8), 1);
    assert_eq!(&o[12..13], b"A");
    assert_eq!(&o[13..17], &[255, 0, 0, 0]);
    assert_eq!(u32_le(&o, 17), 2);
    // trait : 17 + 2 * 16 = 49 octets ; surface : 17 + 3 * 16 = 65 octets
    let surface = debut + 49;
    let fin = surface + 65;
    assert_eq!(o[debut], 0);
    assert_eq!(&o[debut + 1..debut + 5], &[255, 0, 0, 255]);
    assert_eq!(u32_le(&o, debut + 5), 0);
    assert_eq!(f32::from_le_bytes(o[debut + 9..debut + 13].try_into().unwrap()), 0.25);
    assert_eq!(u32_le(&o, debut + 13), 2);
    assert_eq!(f64::from_le_bytes(o[debut + 33..debut + 41].try_into().unwrap()), 1.0);
    assert_eq!(o[surface], 1);
    assert_eq!(&o[surface + 1..surface + 5], &[0, 0, 255, 128]);
    assert_eq!(f32::from_le_bytes(o[surface + 9..surface + 13].try_into().unwrap()), 0.0);
    assert_eq!(u32_le(&o, surface + 13), 3);
    let n = u32_le(&o, fin) as usize;
    assert_eq!(o.len(), fin + 4 + n);
}

#[test]
fn calque_renvoie_le_meme_indice() {
    let mut s = Scene::default();
    assert_eq!(s.calque("A", [1, 2, 3]), 0);
    assert_eq!(s.calque("B", [4, 5, 6]), 1);
    assert_eq!(s.calque("A", [9, 9, 9]), 0);
    assert_eq!(s.calques.len(), 2);
}

#[test]
fn points_mut_parcourt_tous_les_points() {
    let mut s = scene_deux();
    for p in s.points_mut() {
        p[0] += 10.0;
    }
    match &s.primitives[0] {
        Primitive::Trait { points, .. } => assert_eq!(points[1][0], 11.0),
        _ => panic!(),
    }
    match &s.primitives[1] {
        Primitive::Surface { triangles, .. } => assert_eq!(triangles[2][0], 10.0),
        _ => panic!(),
    }
}

#[test]
#[ignore]
fn generer_fixture() {
    let o = ecrire_scene(&scene_deux(), &rapport());
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../essais/commun/test/deux_primitives.scn"), o).unwrap();
}
