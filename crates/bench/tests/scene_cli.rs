use acadrust::{CadDocument, DwgWriter, EntityType, Line};
use std::io::{Cursor, Write};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn zip_de(dwg: &[u8]) -> Vec<u8> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    zw.start_file("plan.dwg", SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).unwrap();
    zw.write_all(dwg).unwrap();
    zw.finish().unwrap().into_inner()
}

fn bench(args: &[&std::path::Path]) -> std::process::Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_bench"));
    c.arg("scene");
    c.args(args);
    c.output().unwrap()
}

#[test]
fn plan_a_l_origine_refuse_en_json_code_1() {
    let tmp = tempfile::tempdir().unwrap();
    let zip = tmp.path().join("a.zip");
    let scn = tmp.path().join("a.scn");
    std::fs::write(&zip, zip_de(include_bytes!("../../cad/tests/fixtures/deux_lignes_un_cercle.dwg"))).unwrap();
    let o = bench(&[&zip, &scn]);
    assert_eq!(o.status.code(), Some(1));
    let e: serde_json::Value = serde_json::from_slice(&o.stderr).unwrap();
    assert_eq!(e["code"], "non_georeference");
    assert!(!scn.exists());
}

#[test]
fn plan_l93_donne_un_scn1_lisible() {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = 6;
    doc.add_entity(EntityType::Line(Line::from_coords(1_042_000., 6_298_000., 0., 1_042_100., 6_298_050., 0.))).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let zip = tmp.path().join("a.zip");
    let scn = tmp.path().join("a.scn");
    std::fs::write(&zip, zip_de(&DwgWriter::write_to_vec(&doc).unwrap())).unwrap();
    let o = bench(&[&zip, &scn]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let rapport: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(rapport["systeme"], "Lambert 93");
    let octets = std::fs::read(&scn).unwrap();
    assert_eq!(&octets[..4], b"SCN1");
    let direct = carto_bench::exporter_scene(&zip, carto_wasm::Repere::L93, None, &scn).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&direct).unwrap(), rapport);
}
