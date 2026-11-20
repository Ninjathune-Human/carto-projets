use carto_wasm::*;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn zip_de_test(entrees: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (nom, contenu) in entrees {
        zw.start_file(*nom, options).unwrap();
        zw.write_all(contenu).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn zip_mixte() -> Vec<u8> {
    zip_de_test(&[
        ("a.dwg", include_bytes!("../../cad/tests/fixtures/deux_lignes_un_cercle.dwg")),
        ("b.dwg", b"abc"),
    ])
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn liste_les_dwg_du_zip() {
    let json = lister_dwg_json(&zip_mixte()).unwrap();
    assert_eq!(json, "[\"a.dwg\",\"b.dwg\"]");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn mesure_un_dwg_valide() {
    let json = mesurer_dwg_json(&zip_mixte(), "a.dwg").unwrap();
    assert!(json.starts_with("{\"chemin\":\"a.dwg\",\"duree_ms\":"));
    assert!(json.contains("\"LINE\":2"));
    assert!(json.contains("\"mesure\""));
    assert!(!json.contains("\"erreur\""));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn un_dwg_casse_donne_un_element_en_erreur_sans_gener_les_autres() {
    let zip = zip_mixte();
    let casse = mesurer_dwg_json(&zip, "b.dwg").unwrap();
    assert!(casse.contains("\"chemin\":\"b.dwg\",\"duree_ms\":"));
    assert!(casse.contains("\"erreur\""));
    assert!(mesurer_dwg_json(&zip, "a.dwg").unwrap().contains("\"LINE\":2"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn un_chemin_absent_du_zip_donne_un_element_en_erreur() {
    let json = mesurer_dwg_json(&zip_mixte(), "absent.dwg").unwrap();
    assert!(json.contains("\"chemin\":\"absent.dwg\""));
    assert!(json.contains("\"erreur\""));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn zip_illisible_donne_une_erreur() {
    assert!(lister_dwg_json(b"pas un zip").is_err());
    let json = mesurer_dwg_json(b"pas un zip", "a.dwg").unwrap();
    assert!(json.contains("\"erreur\""));
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn liste_et_mesure_un_dwg_valide_puis_un_casse() {
        assert_eq!(lister_dwg(&zip_mixte()).unwrap(), "[\"a.dwg\",\"b.dwg\"]");
        assert!(mesurer_dwg(&zip_mixte(), "a.dwg").unwrap().contains("\"LINE\":2"));
        let casse = mesurer_dwg(&zip_mixte(), "b.dwg").unwrap();
        assert!(casse.contains("\"chemin\":\"b.dwg\",\"duree_ms\":"));
        assert!(casse.contains("\"erreur\""));
    }
}

// --- Import complet (ZIP -> SCN1 converti) ---

#[cfg(not(target_arch = "wasm32"))]
mod import {
    use super::*;
    use acadrust::{CadDocument, DwgWriter, EntityType, Line};

    fn dwg(lignes: &[[f64; 4]]) -> Vec<u8> {
        let mut doc = CadDocument::new();
        doc.header.insertion_units = 6;
        for l in lignes {
            doc.add_entity(EntityType::Line(Line::from_coords(l[0], l[1], 0., l[2], l[3], 0.))).unwrap();
        }
        DwgWriter::write_to_vec(&doc).unwrap()
    }

    // Deux lignes à Nice, Lambert 93.
    const NICE: [[f64; 4]; 2] = [[1_042_000., 6_298_000., 1_042_100., 6_298_050.], [1_042_100., 6_298_050., 1_042_150., 6_298_200.]];

    fn zip_nice() -> Vec<u8> {
        zip_de_test(&[("plan.dwg", &dwg(&NICE))])
    }

    fn u32_a(o: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(o[i..i + 4].try_into().unwrap())
    }

    /// Lit le SCN1 : (nombre de traits, premier point du premier trait, rapport JSON).
    fn lire(scn: &[u8]) -> (usize, [f64; 2], serde_json::Value) {
        assert_eq!(&scn[..4], b"SCN1");
        let mut i = 4;
        let nc = u32_a(scn, i) as usize;
        i += 4;
        for _ in 0..nc {
            let n = u32_a(scn, i) as usize;
            i += 4 + n + 4;
        }
        let np = u32_a(scn, i) as usize;
        i += 4;
        let mut premier = [0.; 2];
        for k in 0..np {
            assert_eq!(scn[i], 0, "trait attendu");
            i += 1 + 4 + 4 + 4;
            let n = u32_a(scn, i) as usize;
            i += 4;
            if k == 0 {
                premier = [
                    f64::from_le_bytes(scn[i..i + 8].try_into().unwrap()),
                    f64::from_le_bytes(scn[i + 8..i + 16].try_into().unwrap()),
                ];
            }
            i += 16 * n;
        }
        let nr = u32_a(scn, i) as usize;
        i += 4;
        (np, premier, serde_json::from_slice(&scn[i..i + nr]).unwrap())
    }

    #[test]
    fn importe_un_plan_l93() {
        let scn = importer_zip_scn(&zip_nice(), Repere::L93, None).unwrap();
        let (n, p, rapport) = lire(&scn);
        assert_eq!(n, 2);
        assert!((p[0] - 1_042_000.).abs() < 1e-6 && (p[1] - 6_298_000.).abs() < 1e-6);
        assert_eq!(rapport["systeme"], "Lambert 93");
    }

    #[test]
    fn mercator_proche_de_l93_vers_mercator() {
        let scn = importer_zip_scn(&zip_nice(), Repere::Mercator, None).unwrap();
        let (_, p, _) = lire(&scn);
        let (x, y) = carto_georef::l93_vers_mercator(1_042_000., 6_298_000.).unwrap();
        assert!((p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6);
    }

    #[test]
    fn objet_egare_importe() {
        let mut l = NICE.to_vec();
        l.push([0., 0., 0., 0.]);
        let z = zip_de_test(&[("plan.dwg", &dwg(&l))]);
        let scn = importer_zip_scn(&z, Repere::L93, None).unwrap();
        let (n, _, rapport) = lire(&scn);
        assert_eq!(n, 3);
        assert_eq!(rapport["systeme"], "Lambert 93");
    }

    fn avertissements(rapport: &serde_json::Value) -> Vec<String> {
        rapport["avertissements"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn part_hors_emprise_signalee_sous_la_moitie() {
        // 3 lignes en Lambert 93, 7 près de l'origine : le plan est reconnu (30 % des primitives,
        // au moins 10 %), avec un avertissement pour les 70 % hors de l'emprise.
        let mut l: Vec<[f64; 4]> = (0..3).map(|i| [1_042_000. + 10. * i as f64, 6_298_000., 1_042_005. + 10. * i as f64, 6_298_005.]).collect();
        l.extend((0..7).map(|i| [i as f64, 0., i as f64 + 1., 1.]));
        let z = zip_de_test(&[("plan.dwg", &dwg(&l))]);
        let (n, _, rapport) = lire(&importer_zip_scn(&z, Repere::L93, None).unwrap());
        assert_eq!(n, 10);
        assert_eq!(rapport["systeme"], "Lambert 93");
        assert!(
            avertissements(&rapport).contains(&"Une partie du dessin (70 %) est hors de l'emprise du système reconnu.".to_string()),
            "{rapport}"
        );
        // À la moitié ou plus dans l'emprise, rien n'est signalé.
        let (_, _, rapport) = lire(&importer_zip_scn(&zip_nice(), Repere::L93, None).unwrap());
        assert!(avertissements(&rapport).iter().all(|a| !a.starts_with("Une partie du dessin")), "{rapport}");
        let mut l = NICE.to_vec();
        l.extend([[0., 0., 1., 1.], [2., 2., 3., 3.]]);
        let z = zip_de_test(&[("plan.dwg", &dwg(&l))]);
        let (_, _, rapport) = lire(&importer_zip_scn(&z, Repere::L93, None).unwrap());
        assert!(avertissements(&rapport).iter().all(|a| !a.starts_with("Une partie du dessin")), "{rapport}");
    }

    #[test]
    fn moins_de_dix_pour_cent_refuse() {
        let mut l = vec![NICE[0]];
        l.extend((0..10).map(|i| [i as f64, 0., i as f64 + 1., 1.]));
        let z = zip_de_test(&[("plan.dwg", &dwg(&l))]);
        let v: serde_json::Value = serde_json::from_str(&importer_zip_scn(&z, Repere::L93, None).unwrap_err()).unwrap();
        assert_eq!(v["code"], "non_georeference");
    }

    fn emprise(rapport: &serde_json::Value) -> [f64; 4] {
        let v: Vec<f64> = rapport["emprise"].as_array().expect("emprise attendue").iter().map(|v| v.as_f64().unwrap()).collect();
        v.try_into().unwrap()
    }

    #[test]
    fn emprise_essentielle_sans_objet_egare() {
        // L'objet égaré à l'origine est importé mais reste hors de l'emprise du rapport.
        let mut l = NICE.to_vec();
        l.push([0., 0., 0., 0.]);
        let z = zip_de_test(&[("plan.dwg", &dwg(&l))]);
        let (_, _, rapport) = lire(&importer_zip_scn(&z, Repere::L93, None).unwrap());
        let e = emprise(&rapport);
        assert!(e[0] > 1_000_000. && e[1] > 6_000_000., "l'origine est exclue : {e:?}");
        assert!(e[0] <= 1_042_000. && e[1] <= 6_298_000. && e[2] >= 1_042_150. && e[3] >= 6_298_200., "le plan est couvert : {e:?}");
    }

    #[test]
    fn emprise_dans_le_repere_demande() {
        let (_, _, rapport) = lire(&importer_zip_scn(&zip_nice(), Repere::Mercator, None).unwrap());
        let e = emprise(&rapport);
        for (x, y) in [(1_042_000., 6_298_000.), (1_042_150., 6_298_200.), (1_042_000., 6_298_200.), (1_042_150., 6_298_000.)] {
            let (mx, my) = carto_georef::l93_vers_mercator(x, y).unwrap();
            assert!(mx >= e[0] - 1e-6 && mx <= e[2] + 1e-6 && my >= e[1] - 1e-6 && my <= e[3] + 1e-6, "{mx},{my} hors de {e:?}");
        }
    }

    #[test]
    fn non_georeference() {
        let z = zip_de_test(&[("plan.dwg", &dwg(&[[0., 0., 10., 10.], [10., 10., 20., 5.]]))]);
        let e = importer_zip_scn(&z, Repere::L93, None).unwrap_err();
        let v: serde_json::Value = serde_json::from_str(&e).unwrap();
        assert_eq!(v["code"], "non_georeference");
        assert_eq!(
            v["message"],
            "Plan non géoréférencé : aucun système reconnu (Lambert 93, CC43, CC44, Lambert II étendu, Lambert III Sud ou carto)."
        );
    }

    #[test]
    fn plusieurs_maitres_json() {
        let z = zip_de_test(&[("b.dwg", &dwg(&NICE)), ("a.dwg", &dwg(&NICE))]);
        let e = importer_zip_scn(&z, Repere::L93, None).unwrap_err();
        let v: serde_json::Value = serde_json::from_str(&e).unwrap();
        assert_eq!(v["code"], "plusieurs_maitres");
        assert_eq!(v["candidats"], serde_json::json!(["a.dwg", "b.dwg"]));
    }

    #[test]
    fn aucun_dwg_et_maitre_inconnu() {
        let z = zip_de_test(&[("x.txt", b"rien")]);
        let v: serde_json::Value = serde_json::from_str(&importer_zip_scn(&z, Repere::L93, None).unwrap_err()).unwrap();
        assert_eq!(v["code"], "aucun_dwg");
        let v: serde_json::Value =
            serde_json::from_str(&importer_zip_scn(&zip_nice(), Repere::L93, Some("zz.dwg")).unwrap_err()).unwrap();
        assert_eq!(v["code"], "lecture");
        assert_eq!(v["message"], "dessin maître inconnu : zz.dwg");
    }

    #[test]
    fn repere_depuis_texte() {
        assert!(matches!(Repere::depuis("l93"), Some(Repere::L93)));
        assert!(matches!(Repere::depuis("mercator"), Some(Repere::Mercator)));
        assert!(Repere::depuis("autre").is_none());
    }
}
