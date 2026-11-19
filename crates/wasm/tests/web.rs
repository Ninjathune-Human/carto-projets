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
