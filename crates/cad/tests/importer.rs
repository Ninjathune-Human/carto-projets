use std::io::{Cursor, Write};

use acadrust::entities::{Block, Insert};
use acadrust::tables::{BlockRecord, Layer};
use acadrust::{CadDocument, DwgWriter, EntityType, Line, Vector3};
use carto_cad::importer::{charger_dessins, importer_zip, resoudre_chemin, trouver_maitres};
use carto_cad::{Aplatissement, ErreurCad, Primitive};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

// --- Aides ---

fn zip_de(entrees: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (nom, contenu) in entrees {
        zw.start_file(*nom, options).unwrap();
        zw.write_all(contenu).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn ligne(x1: f64, y1: f64, x2: f64, y2: f64, calque: &str) -> EntityType {
    let mut l = Line::from_coords(x1, y1, 0., x2, y2, 0.);
    l.common.layer = calque.into();
    EntityType::Line(l)
}

/// Le DWG désigne les calques par identifiant : un calque absent de la table deviendrait le calque 0.
fn declarer_calque(doc: &mut CadDocument, nom: &str) {
    if !doc.layers.contains(nom) {
        doc.layers.add(Layer::new(nom)).unwrap();
    }
}

fn dessin(insunits: i16, contenu: Vec<EntityType>) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.header.insertion_units = insunits;
    for e in contenu {
        declarer_calque(&mut doc, &e.common().layer);
        doc.add_entity(e).unwrap();
    }
    doc
}

fn ajouter_calque(doc: &mut CadDocument, nom: &str, eteint: bool) {
    declarer_calque(doc, nom);
    if eteint {
        doc.layers.get_mut(nom).unwrap().turn_off();
    }
}

#[derive(Clone, Copy, Default)]
struct Attache {
    superposition: bool,
    dechargee: bool,
}

/// Rattache la xréf `nom` (fichier enregistré `chemin`) et l'insère à l'origine sur `calque`.
fn attacher(doc: &mut CadDocument, nom: &str, chemin: &str, a: Attache, calque: &str) {
    attacher_a_l_echelle(doc, nom, chemin, a, calque, 1.);
}

/// Idem, avec l'échelle d'INSERT qu'AutoCAD enregistre (conversion d'unités comprise).
fn attacher_a_l_echelle(doc: &mut CadDocument, nom: &str, chemin: &str, a: Attache, calque: &str, echelle: f64) {
    let mut record = BlockRecord::new(nom);
    record.handle = doc.allocate_handle();
    record.flags.is_xref = !a.superposition;
    record.flags.is_xref_overlay = a.superposition;
    record.flags.is_xref_unloaded = a.dechargee;
    record.xref_path = chemin.into();
    let proprietaire = record.handle;
    doc.block_records.add(record).unwrap();
    let mut bloc = Block::new(nom, Vector3::ZERO);
    bloc.common.owner_handle = proprietaire;
    let h = doc.add_entity(EntityType::Block(bloc)).unwrap();
    doc.block_records.get_mut(nom).unwrap().block_entity_handle = h;
    declarer_calque(doc, calque);
    let mut i = Insert::new(nom, Vector3::ZERO).with_scale(echelle, echelle, echelle);
    i.common.layer = calque.into();
    doc.add_entity(EntityType::Insert(i)).unwrap();
}

fn dwg(doc: &CadDocument) -> Vec<u8> {
    DwgWriter::write_to_vec(doc).unwrap()
}

/// Traits de la scène : nom du calque et points.
fn traits(a: &Aplatissement) -> Vec<(String, Vec<[f64; 2]>)> {
    a.scene
        .primitives
        .iter()
        .filter_map(|p| match p {
            Primitive::Trait { calque, points, .. } => Some((a.scene.calques[*calque as usize].nom.clone(), points.clone())),
            _ => None,
        })
        .collect()
}

fn calques(a: &Aplatissement) -> Vec<String> {
    traits(a).into_iter().map(|(c, _)| c).collect()
}

fn proche(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

// --- Choix du dessin maître ---

#[test]
fn zip_sans_dwg() {
    let z = zip_de(&[("lisezmoi.txt", b"rien".to_vec())]);
    assert!(matches!(importer_zip(&z, None), Err(ErreurCad::AucunDwg)));
}

#[test]
fn deux_maitres() {
    let a = dwg(&dessin(6, vec![ligne(0., 0., 1., 0., "A")]));
    let b = dwg(&dessin(6, vec![ligne(0., 0., 2., 0., "B")]));
    let z = zip_de(&[("b.dwg", b), ("a.dwg", a)]);
    match importer_zip(&z, None) {
        Err(ErreurCad::PlusieursMaitres(v)) => assert_eq!(v, vec!["a.dwg".to_string(), "b.dwg".to_string()]),
        Err(e) => panic!("erreur inattendue : {e}"),
        Ok(_) => panic!("erreur attendue"),
    }
    let r = importer_zip(&z, Some("b.dwg")).unwrap();
    assert_eq!(r.rapport.maitre, "b.dwg");
    assert_eq!(r.rapport.dessins, vec!["a.dwg".to_string(), "b.dwg".to_string()]);
    assert_eq!(calques(&r), vec!["B".to_string()]);
    match importer_zip(&z, Some("c.dwg")) {
        Err(ErreurCad::MaitreInconnu(m)) => assert_eq!(m, "c.dwg"),
        Err(e) => panic!("erreur inattendue : {e}"),
        Ok(_) => panic!("erreur attendue"),
    }
}

#[test]
fn xref_exclue_des_maitres() {
    let mut plan = dessin(6, vec![]);
    attacher(&mut plan, "TOPO", r"C:\Projet\Fonds\Topo.dwg", Attache::default(), "0");
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("fonds/topo.dwg", dwg(&dessin(6, vec![])))]);
    let d = charger_dessins(&z).unwrap();
    assert_eq!(d.docs.keys().cloned().collect::<Vec<_>>(), vec!["fonds/topo.dwg".to_string(), "plan.dwg".to_string()]);
    assert_eq!(trouver_maitres(&d), vec!["plan.dwg".to_string()]);
    assert_eq!(importer_zip(&z, None).unwrap().rapport.maitre, "plan.dwg");
}

#[test]
fn resolution_chemins() {
    let chemins = vec!["fonds/topo.dwg".to_string(), "plan.dwg".to_string()];
    assert_eq!(resoudre_chemin(r"C:\Projet\Fonds\Topo.DWG", "plan.dwg", &chemins), Some("fonds/topo.dwg".to_string()));
    // Chemin enregistré tel quel.
    assert_eq!(resoudre_chemin(r"Fonds\TOPO.dwg", "plan.dwg", &chemins), Some("fonds/topo.dwg".to_string()));
    // Chemin relatif au dossier de l'hôte.
    let chemins = vec!["fonds/topo.dwg".to_string(), "plans/plan.dwg".to_string()];
    assert_eq!(resoudre_chemin(r"..\Fonds\Topo.dwg", "plans/plan.dwg", &chemins), Some("fonds/topo.dwg".to_string()));
    // Nom de fichier seul.
    let chemins = vec!["plan.dwg".to_string(), "xrefs/topo.dwg".to_string()];
    assert_eq!(resoudre_chemin(r"C:\Projet\Fonds\Topo.DWG", "plan.dwg", &chemins), Some("xrefs/topo.dwg".to_string()));
    assert_eq!(resoudre_chemin(r"C:\Projet\Fonds\Absent.dwg", "plan.dwg", &chemins), None);
}

// --- Xréfs ---

#[test]
fn xref_en_m_dans_maitre_en_cm() {
    // AutoCAD enregistre la conversion d'unités dans l'échelle de l'INSERT : 1 m = 100 cm.
    let mut plan = dessin(5, vec![]);
    attacher_a_l_echelle(&mut plan, "TOPO", "topo.dwg", Attache::default(), "FOND", 100.);
    let topo = dessin(6, vec![ligne(0., 0., 1., 0., "Bord"), ligne(0., 0., 0., 1., "0")]);
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("topo.dwg", dwg(&topo))]);
    let r = importer_zip(&z, None).unwrap();
    assert_eq!(r.rapport.maitre, "plan.dwg");
    assert_eq!(r.rapport.dessins, vec!["plan.dwg".to_string(), "topo.dwg".to_string()]);
    assert!(r.rapport.avertissements.is_empty(), "{:?}", r.rapport.avertissements);
    let t = traits(&r);
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].0, "TOPO|Bord");
    assert!(proche(t[0].1[0], [0., 0.]) && proche(t[0].1[1], [100., 0.]), "{:?}", t[0].1);
    // Le calque 0 de la xréf prend le calque de l'insertion, comme dans un bloc.
    assert_eq!(t[1].0, "FOND");
    assert!(proche(t[1].1[1], [0., 100.]), "{:?}", t[1].1);
}

#[test]
fn xref_echelle_un_sans_facteur() {
    // Aucun second facteur d'unités : une xréf en m insérée à l'échelle 1 dans un maître en cm garde ses valeurs.
    let mut plan = dessin(5, vec![]);
    attacher(&mut plan, "TOPO", "topo.dwg", Attache::default(), "0");
    let topo = dessin(6, vec![ligne(0., 0., 1., 0., "Bord")]);
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("topo.dwg", dwg(&topo))]);
    let t = traits(&importer_zip(&z, None).unwrap());
    assert_eq!(t.len(), 1);
    assert!(proche(t[0].1[0], [0., 0.]) && proche(t[0].1[1], [1., 0.]), "{:?}", t[0].1);
}

#[test]
fn xref_point_de_base_insbase() {
    let mut plan = dessin(6, vec![]);
    attacher(&mut plan, "TOPO", "topo.dwg", Attache::default(), "0");
    let mut topo = dessin(6, vec![ligne(10., 5., 11., 5., "L")]);
    topo.header.model_space_insertion_base = Vector3::new(10., 5., 0.);
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("topo.dwg", dwg(&topo))]);
    let t = traits(&importer_zip(&z, None).unwrap());
    assert!(proche(t[0].1[0], [0., 0.]) && proche(t[0].1[1], [1., 0.]), "{:?}", t[0].1);
}

#[test]
fn surcharge_visretain() {
    let mut plan = dessin(6, vec![]);
    ajouter_calque(&mut plan, "TOPO|Arbres", true);
    attacher(&mut plan, "TOPO", "topo.dwg", Attache::default(), "0");
    let mut topo = dessin(6, vec![ligne(0., 0., 1., 0., "Arbres"), ligne(0., 0., 2., 0., "Bati"), ligne(0., 0., 3., 0., "Haies")]);
    ajouter_calque(&mut topo, "Arbres", false);
    ajouter_calque(&mut topo, "Bati", false);
    ajouter_calque(&mut topo, "Haies", true);
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("topo.dwg", dwg(&topo))]);
    let r = importer_zip(&z, None).unwrap();
    // Arbres : éteint dans l'hôte, allumé dans la xréf ; Haies : absent de l'hôte, éteint dans la xréf.
    assert_eq!(calques(&r), vec!["TOPO|Bati".to_string()]);
}

#[test]
fn cycle_coupe() {
    let mut a = dessin(6, vec![ligne(0., 0., 1., 0., "A")]);
    attacher(&mut a, "B", "b.dwg", Attache::default(), "0");
    let mut b = dessin(6, vec![ligne(0., 0., 2., 0., "L")]);
    attacher(&mut b, "A", "a.dwg", Attache::default(), "0");
    let z = zip_de(&[("a.dwg", dwg(&a)), ("b.dwg", dwg(&b))]);
    let r = importer_zip(&z, Some("a.dwg")).unwrap();
    assert_eq!(calques(&r), vec!["A".to_string(), "B|L".to_string()]);
    assert_eq!(r.rapport.avertissements, vec!["cycle de xréfs : a.dwg → b.dwg → a.dwg".to_string()]);
}

#[test]
fn superposition_imbriquee_non_chargee() {
    let mut plan = dessin(6, vec![]);
    attacher(&mut plan, "P", "p.dwg", Attache { superposition: true, ..Attache::default() }, "0");
    attacher(&mut plan, "A", "a.dwg", Attache::default(), "0");
    let p = dessin(6, vec![ligne(0., 0., 1., 0., "L")]);
    let mut a = dessin(6, vec![ligne(0., 0., 2., 0., "L")]);
    attacher(&mut a, "O", "o.dwg", Attache { superposition: true, ..Attache::default() }, "0");
    let o = dessin(6, vec![ligne(0., 0., 3., 0., "L")]);
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("p.dwg", dwg(&p)), ("a.dwg", dwg(&a)), ("o.dwg", dwg(&o))]);
    let r = importer_zip(&z, Some("plan.dwg")).unwrap();
    // Superposition rattachée au maître : chargée ; sous une autre xréf : non chargée.
    assert_eq!(calques(&r), vec!["P|L".to_string(), "A|L".to_string()]);
    assert!(r.rapport.avertissements.is_empty(), "{:?}", r.rapport.avertissements);
}

#[test]
fn xref_imbriquee_nommee_par_la_plus_interne() {
    let mut plan = dessin(6, vec![]);
    attacher(&mut plan, "A", "xrefs/a.dwg", Attache::default(), "0");
    let mut a = dessin(6, vec![]);
    // Chemin relatif à l'hôte intermédiaire.
    attacher(&mut a, "C", r".\c.dwg", Attache::default(), "0");
    let c = dessin(6, vec![ligne(0., 0., 1., 0., "L")]);
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("xrefs/a.dwg", dwg(&a)), ("xrefs/c.dwg", dwg(&c))]);
    let r = importer_zip(&z, None).unwrap();
    assert_eq!(calques(&r), vec!["C|L".to_string()]);
}

#[test]
fn xref_dechargee_ou_introuvable_signalee() {
    let mut plan = dessin(6, vec![]);
    attacher(&mut plan, "TOPO", "topo.dwg", Attache { dechargee: true, ..Attache::default() }, "0");
    attacher(&mut plan, "CAD", r"C:\Ailleurs\Cadastre.dwg", Attache::default(), "0");
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("topo.dwg", dwg(&dessin(6, vec![ligne(0., 0., 1., 0., "L")])))]);
    let r = importer_zip(&z, None).unwrap();
    assert!(r.scene.primitives.is_empty());
    assert_eq!(
        r.rapport.avertissements,
        vec!["xréf déchargée : TOPO".to_string(), r"xréf introuvable : C:\Ailleurs\Cadastre.dwg".to_string()]
    );
}

#[test]
fn dessin_illisible_ignore_sauf_le_maitre() {
    let mut plan = dessin(6, vec![ligne(0., 0., 1., 0., "L")]);
    attacher(&mut plan, "CASSE", "casse.dwg", Attache::default(), "0");
    let z = zip_de(&[("plan.dwg", dwg(&plan)), ("casse.dwg", b"pas un dwg".to_vec())]);
    let r = importer_zip(&z, None).unwrap();
    assert_eq!(r.rapport.dessins, vec!["casse.dwg".to_string(), "plan.dwg".to_string()]);
    assert_eq!(calques(&r), vec!["L".to_string()]);
    assert_eq!(r.rapport.avertissements.len(), 1, "{:?}", r.rapport.avertissements);
    assert!(r.rapport.avertissements[0].starts_with("dessin illisible : casse.dwg ("), "{:?}", r.rapport.avertissements);
    assert!(matches!(importer_zip(&z, Some("casse.dwg")), Err(ErreurCad::Dwg(_))));
}
