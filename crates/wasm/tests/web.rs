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
fn mesure_un_zip_avec_un_dwg_valide_et_un_casse() {
    let json = mesurer_zip_json(&zip_mixte()).unwrap();
    assert!(json.contains("\"LINE\":2"));
    assert!(json.contains("\"chemin\":\"b.dwg\",\"duree_ms\":"));
    assert!(json.contains("\"erreur\""));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn zip_illisible_donne_une_erreur() {
    assert!(mesurer_zip_json(b"pas un zip").is_err());
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn mesure_un_zip_avec_un_dwg_valide_et_un_casse() {
        let json = mesurer_zip(&zip_mixte()).unwrap();
        assert!(json.contains("\"LINE\":2"));
        assert!(json.contains("\"chemin\":\"b.dwg\",\"duree_ms\":"));
        assert!(json.contains("\"erreur\""));
    }
}
