use carto_bench::mesurer_dossier;
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

#[test]
fn ecrit_les_csv_et_ne_s_arrete_pas_sur_un_dwg_casse() {
    let fixture = include_bytes!("../../cad/tests/fixtures/deux_lignes_un_cercle.dwg");
    let tmp = tempfile::tempdir().unwrap();
    let zips = tmp.path().join("zips");
    std::fs::create_dir(&zips).unwrap();
    std::fs::write(
        zips.join("a.zip"),
        zip_de_test(&[("Plans\\bon.dwg", fixture), ("casse.dwg", b"abc")]),
    )
    .unwrap();
    std::fs::write(zips.join("mauvais.zip"), b"pas un zip").unwrap();
    let sortie = tmp.path().join("sortie/sous");
    mesurer_dossier(&zips, &sortie).unwrap();

    let mut lecteur = csv::Reader::from_path(sortie.join("acadrust.csv")).unwrap();
    assert_eq!(
        lecteur.headers().unwrap().iter().collect::<Vec<_>>(),
        ["fichier", "chemin", "lecteur", "version", "duree_ms", "type", "nombre", "erreur"]
    );
    let lignes: Vec<csv::StringRecord> = lecteur.records().map(|r| r.unwrap()).collect();
    let line = lignes.iter().find(|r| &r[5] == "LINE").unwrap();
    assert_eq!((&line[0], &line[1], &line[2], &line[6]), ("a.zip", "Plans/bon.dwg", "acadrust", "2"));
    assert!(line[7].is_empty());
    let casse = lignes.iter().find(|r| &r[1] == "casse.dwg").unwrap();
    assert_eq!((&casse[5], &casse[6]), ("", "0"));
    assert!(!casse[7].is_empty());
    let mauvais = lignes.iter().find(|r| &r[0] == "mauvais.zip").unwrap();
    assert!(mauvais[1].is_empty() && !mauvais[7].is_empty());

    let mut emp = csv::Reader::from_path(sortie.join("emprises.csv")).unwrap();
    assert_eq!(
        emp.headers().unwrap().iter().collect::<Vec<_>>(),
        ["fichier", "chemin", "insunits", "xmin", "ymin", "xmax", "ymax", "geodata"]
    );
    let e: Vec<csv::StringRecord> = emp.records().map(|r| r.unwrap()).collect();
    assert_eq!(e.len(), 1);
    assert_eq!(&e[0][1], "Plans/bon.dwg");
    assert_eq!(&e[0][7], "false");
}
