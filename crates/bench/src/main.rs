use std::path::Path;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("usage : bench mesurer <dossier_zips> <dossier_sortie>");
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
        _ => usage(),
    }
}
