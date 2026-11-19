use std::path::Path;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage : bench mesurer <dossier_zips> <dossier_sortie>\n       bench export-lignes <zip> <chemin_dwg> <facteur> <sortie.lignes>");
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
        _ => usage(),
    }
}
