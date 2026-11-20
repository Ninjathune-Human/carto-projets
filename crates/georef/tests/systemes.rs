use carto_georef::{emprise_essentielle, l93_vers_mercator, reconnaitre, Convertisseur, Reconnaissance, Systeme};

const CSV: &str = include_str!("fixtures/conversions.csv");

fn systeme(nom: &str) -> Systeme {
    Systeme::TOUS.into_iter().find(|s| s.nom() == nom).unwrap_or_else(|| panic!("système inconnu : {nom}"))
}

fn lignes() -> Vec<(Systeme, [f64; 4])> {
    CSV.lines()
        .skip(1)
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let n = |i: usize| c[i].parse::<f64>().unwrap();
            (systeme(c[0]), [n(1), n(2), n(3), n(4)])
        })
        .collect()
}

#[test]
fn conversions_sous_le_millimetre() {
    let mut max = [0.0f64; 6];
    let mut n = 0;
    for (s, [x, y, x93, y93]) in lignes() {
        let c = Convertisseur::nouveau(Reconnaissance { systeme: s, facteur: 1.0 }).unwrap();
        let (rx, ry) = c.vers_l93(x, y).unwrap();
        let e = (rx - x93).abs().max((ry - y93).abs());
        let i = Systeme::TOUS.iter().position(|t| *t == s).unwrap();
        max[i] = max[i].max(e);
        assert!((rx - x93).abs() < 0.001 && (ry - y93).abs() < 0.001, "{} ({x}, {y}) : ({rx}, {ry}) != ({x93}, {y93})", s.nom());
        n += 1;
    }
    assert_eq!(n, 5000);
    eprintln!("erreur maximale par système (m) : {max:?}");
}

#[test]
fn l93_identite() {
    let c = Convertisseur::nouveau(Reconnaissance { systeme: Systeme::Lambert93, facteur: 1.0 }).unwrap();
    let (x, y) = c.vers_l93(1044000.123, 6298000.456).unwrap();
    assert!((x - 1044000.123).abs() < 1e-9 && (y - 6298000.456).abs() < 1e-9);
}

#[test]
fn mercator_coherent_avec_l93() {
    let c = Convertisseur::nouveau(Reconnaissance { systeme: Systeme::Lambert93, facteur: 1.0 }).unwrap();
    let a = c.vers_mercator(1044000.0, 6298000.0).unwrap();
    let b = l93_vers_mercator(1044000.0, 6298000.0).unwrap();
    assert!((a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
    // Un point Lambert II étendu converti en Mercator coïncide avec son équivalent Lambert 93.
    let l2 = Convertisseur::nouveau(Reconnaissance { systeme: Systeme::LambertIIEtendu, facteur: 1.0 }).unwrap();
    let (x93, y93) = l2.vers_l93(1_000_000.0, 1_900_000.0).unwrap();
    let m1 = l2.vers_mercator(1_000_000.0, 1_900_000.0).unwrap();
    let m2 = l93_vers_mercator(x93, y93).unwrap();
    assert!((m1.0 - m2.0).abs() < 1e-3 && (m1.1 - m2.1).abs() < 1e-3);
}

fn centre(x: (f64, f64), y: (f64, f64)) -> [f64; 4] {
    let (cx, cy) = ((x.0 + x.1) / 2.0, (y.0 + y.1) / 2.0);
    [cx - 10.0, cy - 10.0, cx + 10.0, cy + 10.0]
}

#[test]
fn reconnaitre_chaque_systeme() {
    let cas = [
        (Systeme::Lambert93, (1_002_528.0, 1_063_264.0), (6_285_267.0, 6_376_983.0)),
        (Systeme::Cc43, (1_990_000.0, 2_060_000.0), (2_273_593.0, 2_365_152.0)),
        (Systeme::Cc44, (1_990_000.0, 2_060_000.0), (3_162_627.0, 3_254_220.0)),
        (Systeme::LambertIIEtendu, (970_000.0, 1_030_000.0), (1_854_473.0, 1_946_729.0)),
        (Systeme::LambertIIISud, (970_000.0, 1_030_000.0), (154_215.0, 246_222.0)),
        (Systeme::LambertIIICarto, (970_000.0, 1_030_000.0), (3_154_215.0, 3_246_222.0)),
    ];
    for (s, x, y) in cas {
        let r = reconnaitre(centre(x, y)).unwrap_or_else(|| panic!("{} non reconnu", s.nom()));
        assert_eq!((r.systeme, r.facteur), (s, 1.0), "{}", s.nom());
    }
}

#[test]
fn reconnaitre_en_cm() {
    let r = reconnaitre([1_044_000.0 * 100.0, 6_298_000.0 * 100.0, 1_045_000.0 * 100.0, 6_299_000.0 * 100.0]).unwrap();
    assert_eq!((r.systeme, r.facteur), (Systeme::Lambert93, 0.01));
}

#[test]
fn rien_hors_systemes() {
    assert_eq!(reconnaitre([0.0, 0.0, 100.0, 100.0]), None);
}

#[test]
fn objet_egare_ecarte() {
    let mut pts: Vec<[f64; 2]> = (0..1000)
        .map(|i| {
            let t = i as f64 / 999.0;
            [1_043_000.0 + 2000.0 * t, 6_297_000.0 + 2000.0 * (1.0 - t)]
        })
        .collect();
    pts.extend([[0.0, 0.0]; 5]);
    let e = emprise_essentielle(&pts).unwrap();
    let r = reconnaitre(e).unwrap();
    assert_eq!((r.systeme, r.facteur), (Systeme::Lambert93, 1.0));
    assert_eq!(emprise_essentielle(&[]), None);
    assert_eq!(emprise_essentielle(&[[f64::NAN, 1.0]]), None);
}

#[test]
fn noms() {
    let noms: Vec<&str> = Systeme::TOUS.iter().map(|s| s.nom()).collect();
    assert_eq!(noms, ["Lambert 93", "CC43", "CC44", "Lambert II étendu", "Lambert III Sud", "Lambert III carto"]);
}

/// Mesuré en release : `cargo test -p carto-georef --release performance`.
/// Ignoré en debug, où la limite de 2 s n'a pas de sens.
#[test]
#[cfg_attr(debug_assertions, ignore = "à lancer avec --release")]
fn performance() {
    let c = Convertisseur::nouveau(Reconnaissance { systeme: Systeme::LambertIIEtendu, facteur: 1.0 }).unwrap();
    let t = std::time::Instant::now();
    let mut somme = 0.0;
    for i in 0..100_000 {
        let (x, y) = c.vers_l93(1_000_000.0 + (i % 1000) as f64, 1_900_000.0 + (i / 1000) as f64).unwrap();
        somme += x + y;
    }
    let d = t.elapsed();
    assert!(somme.is_finite());
    assert!(d.as_secs_f64() < 2.0, "100 000 conversions en {d:?}");
}
