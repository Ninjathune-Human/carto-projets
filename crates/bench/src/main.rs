use std::path::Path;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage : bench mesurer <dossier_zips> <dossier_sortie>\n       bench export-lignes <zip> <chemin_dwg> <facteur> <sortie.lignes>\n       bench scene <zip> <sortie.scn> [l93|mercator] [maitre]");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, zips, sortie] if cmd == "mesurer" => {
            match carto_bench::mesurer_dossier(Path::new(zips), Path::new(sortie)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("erreur : {e}");
                    ExitCode::FAILURE
                }
            }
        }
        [cmd, zip, chemin, facteur, sortie] if cmd == "export-lignes" => {
            let facteur: f64 = match facteur.parse() {
                Ok(f) => f,
                Err(_) => {
                    eprintln!("erreur : facteur invalide : {facteur}");
                    return ExitCode::FAILURE;
                }
            };
            match carto_bench::exporter_lignes(Path::new(zip), chemin, facteur, Path::new(sortie)) {
                Ok((n, p)) => {
                    println!("{n} polylignes, {p} points");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("erreur : {e}");
                    ExitCode::FAILURE
                }
            }
        }
        [cmd, zip, sortie, reste @ ..] if cmd == "scene" && reste.len() <= 2 => {
            let repere = match reste.first().map(String::as_str).unwrap_or("l93") {
                "l93" => carto_wasm::Repere::L93,
                "mercator" => carto_wasm::Repere::Mercator,
                autre => {
                    eprintln!("erreur : repère inconnu : {autre}");
                    return ExitCode::FAILURE;
                }
            };
            let maitre = reste.get(1).map(String::as_str);
            match carto_bench::exporter_scene(Path::new(zip), repere, maitre, Path::new(sortie)) {
                Ok(rapport) => {
                    println!("{rapport}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => usage(),
    }
}
