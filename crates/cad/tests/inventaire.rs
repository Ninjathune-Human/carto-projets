use carto_cad::{inventorier, lire_entree, ErreurCad};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Écrit les entrées telles quelles (antislashs compris), compressées en deflate.
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
fn normalise_les_chemins_et_classe_par_extension() {
    let z = zip_de_test(&[
        ("Plans\\Façade.dwg", b"x"),
        ("Xref/TOPO.DWG", b"x"),
        ("Fonts/romans.shx", b"x"),
        ("img/photo.JPG", b"x"),
        ("acad.ctb", b"x"),
        ("rapport.txt", b"x"),
    ]);
    let inv = inventorier(&z).unwrap();
    assert_eq!(inv.dwg, vec!["Plans/Façade.dwg", "Xref/TOPO.DWG"]);
    assert_eq!(inv.polices, vec!["Fonts/romans.shx"]);
    assert_eq!(inv.images, vec!["img/photo.JPG"]);
    assert_eq!(inv.styles_trace, vec!["acad.ctb"]);
    assert_eq!(inv.autres, vec!["rapport.txt"]);
}

#[test]
fn lit_une_entree_par_son_chemin_normalise() {
    let z = zip_de_test(&[("Plans\\Façade.dwg", b"abc")]);
    assert_eq!(lire_entree(&z, "Plans/Façade.dwg").unwrap(), b"abc");
}

#[test]
fn zip_corrompu_donne_une_erreur_zip() {
    assert!(matches!(inventorier(b"pas un zip"), Err(ErreurCad::Zip(_))));
}
