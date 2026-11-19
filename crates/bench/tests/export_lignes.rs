use acadrust::{CadDocument, DwgWriter, EntityType, Line};
use carto_bench::exporter_lignes;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[test]
fn exporte_une_ligne_l93_en_mercator() {
    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Line(Line::from_coords(700000., 6600000., 0., 1044000., 6298000., 0.))).unwrap();
    let dwg = DwgWriter::write_to_vec(&doc).unwrap();
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    zw.start_file("plan.dwg", SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).unwrap();
    zw.write_all(&dwg).unwrap();
    let octets = zw.finish().unwrap().into_inner();

    let tmp = tempfile::tempdir().unwrap();
    let zip = tmp.path().join("a.zip");
    let sortie = tmp.path().join("a.lignes");
    std::fs::write(&zip, octets).unwrap();

    assert_eq!(exporter_lignes(&zip, "plan.dwg", 1.0, &sortie).unwrap(), (1, 2));
    let o = std::fs::read(&sortie).unwrap();
    assert_eq!(&o[..4], b"LIG0");
    let f = |i: usize| f64::from_le_bytes(o[i..i + 8].try_into().unwrap());
    assert!((f(12) - 333958.4723798207).abs() < 1e-3);
    assert!((f(20) - 5860839.8299478).abs() < 1e-3);
}
