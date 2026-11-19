use carto_georef::l93_vers_mercator;

fn proche(a: (f64, f64), b: (f64, f64)) {
    assert!((a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3, "{a:?} != {b:?}");
}

#[test]
fn origine_lambert_93() {
    proche(l93_vers_mercator(700000.0, 6600000.0).unwrap(), (333958.4723798207, 5860839.8299478));
}

#[test]
fn point_a_nice() {
    proche(l93_vers_mercator(1044000.0, 6298000.0).unwrap(), (809111.0042919414, 5418722.747879568));
}
