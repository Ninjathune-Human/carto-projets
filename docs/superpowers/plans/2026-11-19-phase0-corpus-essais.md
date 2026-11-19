# Phase 0, corpus et essais : plan d'exécution

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal :** choisir le lecteur DWG sur un corpus réel, valider la couche WebGL sur fond Esri sur iPad, et vérifier le CORS et la couverture des sources open data, avant tout investissement dans le moteur complet.

**Architecture :** un workspace Rust (`cad`, `georef`, `bench`, `wasm`) mesure le corpus en natif et dans le navigateur. Des outils Python établissent la référence (DXF convertis par ODA, comptés avec ezdxf), comparent les lecteurs et classent les emprises. Trois pages d'essai, publiées sur GitHub Pages, testent la carte, la couche WebGL et les sources. Le corpus des clients ne quitte jamais la machine.

**Tech Stack :** Rust stable, wasm-bindgen, acadrust 0.6.8 (MPL-2.0), proj4rs, zip ; Node ≥ 20 avec @mlightcad/libredwg-web 0.9.0 (GPL-3.0, essai seulement) ; Python 3 avec ezdxf, pyproj, shapely ; ArcGIS Maps SDK for JavaScript 5.x par CDN.

**Spec :** [Versement de projets AutoCAD sur carte ArcGIS : conception](https://claude.ai/artifact/TEPnvhyJCkNw8caHP9px7N), exporté en `docs/superpowers/specs/2026-11-19-carto-projets-design.md` à la tâche 1.

## Global Constraints

- Nom de travail du dépôt : `carto-projets` (renommable avant la tâche 1).
- Corpus des clients jamais versionné ni publié : `corpus/` est dans `.gitignore`. Les sorties versionnées n'emploient que des identifiants anonymes `P01`, `P02`…
- acadrust en `default-features = false` pour la cible wasm32 : sa fonction `parallel` (Rayon) est activée par défaut.
- libredwg-web n'est utilisé que dans `essais/libredwg/`, jamais dans le code livré (GPL-3.0).
- Lambert 93 = EPSG:2154 ; vue en Web Mercator = EPSG:3857 ; RGF93 vers WGS84 = transformation nulle.
- Clé API Esri (ArcGIS Location Platform) jamais écrite dans le code : saisie dans la page, gardée en `localStorage` sous la clé `cle-esri`, limitée à l'URL du site côté Esri.
- Contour : jeu « Limite de la Métropole Nice Côte d'Azur » au 1er janvier 2022 (data.gouv.fr), copié dans `data/limite_metropole.geojson`.
- Voile blanc à 50 % d'opacité hors Métropole ; trait de contour de 1,5 px à 6 px au niveau 20 (1,125 pt à 4,5 pt), par variable visuelle de taille sur `$view.scale`.
- Navigateurs : ceux du SDK 5.x, dont Safari 18 au minimum sur iPad et iPhone.
- GitHub Pages : `index.html` à la racine, bannière `banniere.svg`, et une étape du workflow qui déplace le tag `deploye` sur le commit publié après chaque déploiement réussi.
- Interface : une commande garde la même place dans tous les modes ; « couche » = donnée, « calque » = ce qu'on active ou désactive.
- Sous-entités exclues de toutes les comparaisons de comptage : `ATTRIB`, `SEQEND`, `VERTEX`.
- Schéma CSV commun aux lecteurs : `fichier,chemin,lecteur,version,duree_ms,type,nombre,erreur`, une ligne par couple (DWG, type) ; un DWG en erreur = une ligne avec `type` vide, `nombre` à 0 et `erreur` renseigné.

## Review Focus

- Noms d'entrées de ZIP avec antislashs Windows et accents (`Plans\Façade.dwg`) : l'inventaire doit les rendre en `Plans/Façade.dwg`. Test : tâche 2.
- DWG tronqué ou illisible au milieu du corpus : le banc écrit une ligne d'erreur et passe au suivant, sans jamais paniquer. Tests : tâches 3 et 4.
- En-tête EXTMIN/EXTMAX non renseigné (±1e20) : l'emprise est « inconnue », pas « hors Métropole ». Tests : tâches 3 et 6.
- Plans en centimètres ou en millimètres : le classement essaie ×1, ×0,01, ×0,001 et retient le premier facteur qui place le plan dans la Métropole. Test : tâche 6.
- Clé Esri absente, expirée ou refusée : la page affiche un message et garde le contour sur un fond neutre, jamais d'écran blanc. Vérification : tâche 9.

---

## Structure des fichiers

```
carto-projets/
  Cargo.toml                     workspace : crates/cad, crates/georef, crates/bench, crates/wasm
  .gitignore                     corpus/, target/, node_modules/, essais/**/pkg/
  README.md   banniere.svg   index.html (liens vers les essais)
  catalogue.json                 sources open data de la phase 0
  data/limite_metropole.geojson
  crates/cad/src/                lib.rs, erreur.rs, inventaire.rs, mesure.rs, lignes.rs
  crates/cad/tests/              inventaire.rs, mesure.rs, lignes.rs, fixtures/
  crates/georef/src/lib.rs       crates/georef/tests/l93.rs
  crates/bench/src/              lib.rs (mesurer_dossier), main.rs (CLI)
  crates/wasm/src/lib.rs         crates/wasm/tests/web.rs
  essais/banc/                   index.html, banc.js, worker.js
  essais/libredwg/               package.json, mesurer.mjs, test/mesurer.test.mjs
  essais/carte/                  index.html, carte.js, couche-lignes.js, outils.js, test/
  essais/sources/                index.html, sources.js, test/sources.test.mjs
  tools/                         reference.py, comparer.py, emprises.py, requirements.txt, tests/
  corpus/ (ignoré)               zips/, ref/ (DXF ODA), resultats/ (CSV)
  docs/phase0/rapport.md
  .github/workflows/pages.yml
```

---

### Task 1 : Dépôt, squelette et déploiement

**Files :**
- Create : `Cargo.toml`, `.gitignore`, `README.md`, `banniere.svg`, `index.html`, `.github/workflows/pages.yml`, `crates/{cad,georef,bench,wasm}/Cargo.toml` et `src/lib.rs` vides
- Create : `docs/superpowers/specs/2026-11-19-carto-projets-design.md` (export du document de conception), ce plan dans `docs/superpowers/plans/`

**Interfaces :**
- Produces : le workspace et l'URL Pages `https://ninjathune-human.github.io/carto-projets/` utilisés par les tâches 4, 9 et 10.

- [ ] **Step 1 :** l'utilisateur crée le dépôt `Ninjathune-Human/carto-projets` (public, Pages activé sur GitHub Actions) et donne l'accès en écriture.
- [ ] **Step 2 :** écrire le workspace et les crates vides ; `index.html` liste les trois essais (`essais/banc/`, `essais/carte/`, `essais/sources/`) dans la charte sobre du projet.
- [ ] **Step 3 :** lancer `cargo test --workspace`. Attendu : `test result: ok` pour chaque crate, 0 test.
- [ ] **Step 4 :** écrire `pages.yml` : sur push vers `main`, construire `crates/wasm` avec `wasm-pack build --target web --release` vers `essais/banc/pkg/`, publier le dépôt sur Pages, puis `git tag -f deploye "$GITHUB_SHA" && git push -f origin deploye` (permission `contents: write`). Versions des actions : les dernières majeures publiées au moment d'écrire, vérifiées sur le Marketplace.
- [ ] **Step 5 :** pousser, puis vérifier : workflow au vert, `index.html` servi à l'URL Pages, et `git ls-remote --tags origin deploye` renvoie le SHA du commit poussé.
- [ ] **Step 6 :** commit `chore: squelette du dépôt et déploiement Pages`.

### Task 2 : Inventaire du ZIP

**Files :**
- Create : `crates/cad/src/erreur.rs`, `crates/cad/src/inventaire.rs`
- Test : `crates/cad/tests/inventaire.rs`

**Interfaces :**
- Produces :
  - `pub enum ErreurCad { Zip(String), Dwg(String), EntreeAbsente(String) }` (thiserror)
  - `pub struct Inventaire { pub dwg: Vec<String>, pub polices: Vec<String>, pub images: Vec<String>, pub styles_trace: Vec<String>, pub autres: Vec<String> }`
  - `pub fn inventorier(octets: &[u8]) -> Result<Inventaire, ErreurCad>`
  - `pub fn lire_entree(octets: &[u8], chemin: &str) -> Result<Vec<u8>, ErreurCad>`

- [ ] **Step 1 : écrire les tests qui échouent**

```rust
#[test]
fn normalise_les_chemins_et_classe_par_extension() {
    let z = zip_de_test(&[("Plans\\Façade.dwg", b"x"), ("Xref/TOPO.DWG", b"x"),
        ("Fonts/romans.shx", b"x"), ("img/photo.JPG", b"x"), ("acad.ctb", b"x"), ("rapport.txt", b"x")]);
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
```

`zip_de_test` écrit les entrées telles quelles avec `zip::ZipWriter` dans un `Cursor<Vec<u8>>` (dépendance de dev).

- [ ] **Step 2 :** `cargo test -p cad --test inventaire`. Attendu : échec de compilation, `inventorier` introuvable.
- [ ] **Step 3 :** implémenter dans `inventaire.rs`. Antislashs remplacés par `/`. Extensions sans tenir compte de la casse : dwg ; shx, ttf, ttc, otf ; jpg, jpeg, png, tif, tiff, bmp, pdf ; ctb, stb ; le reste dans `autres`. Chaque liste est triée. Crate `zip` avec un deflate en Rust pur (sans zlib C) pour compiler en wasm32 : noms de features à vérifier dans la doc de la version retenue.
- [ ] **Step 4 :** relancer. Attendu : 3 tests passent.
- [ ] **Step 5 :** commit `feat(cad): inventaire du ZIP eTransmit`.

### Task 3 : Mesure d'un DWG avec acadrust et banc natif

**Files :**
- Create : `crates/cad/src/mesure.rs`, `crates/bench/src/lib.rs`, `crates/bench/src/main.rs`
- Test : `crates/cad/tests/mesure.rs`, `crates/bench/tests/dossier.rs`
- Create : `crates/cad/tests/fixtures/deux_lignes_un_cercle.dwg` (synthétique, versionné)

**Interfaces :**
- Consumes : `inventorier`, `lire_entree` (tâche 2).
- Produces :
  - `pub fn lire_document(octets: &[u8]) -> Result<acadrust::CadDocument, ErreurCad>` : `DwgReader::from_stream(Cursor::new(octets))`.
  - `pub struct Mesure { pub version: String, pub par_type: BTreeMap<String, u64>, pub total: u64, pub inconnues: u64, pub insunits: i16, pub emprise: Option<[f64; 4]>, pub geodata: bool, pub erreurs_recuperees: usize }` (serde `Serialize`)
  - `pub fn mesurer(octets: &[u8]) -> Result<Mesure, ErreurCad>`
  - `pub fn emprise_depuis(min: [f64; 2], max: [f64; 2]) -> Option<[f64; 4]>`
  - `pub fn mesurer_dossier(zips: &Path, sortie: &Path) -> std::io::Result<()>` : écrit `sortie/acadrust.csv` (schéma commun, `lecteur` = `acadrust`) et `sortie/emprises.csv` (`fichier,chemin,insunits,xmin,ymin,xmax,ymax,geodata`).
  - CLI : `bench mesurer <dossier_zips> <dossier_sortie>`

Règles de `mesurer` : `read_with_stats()` ; entités de `model_space_entities()` hors `ATTRIB`, `SEQEND`, `VERTEX` ; nom = `as_entity().entity_type()`, sauf `EntityType::Unknown(u)`, compté sous `?{u.dxf_name}` et dans `inconnues` ; `version` = `stats.source_version` ; `erreurs_recuperees` = `stats.recovered_errors` ; `insunits` = `header.insertion_units` ; `emprise` d'après `header.model_space_extents_min/max` ; `geodata` vrai si `objects` contient un `ObjectType::GeoData`.

- [ ] **Step 1 : écrire les tests qui échouent**

```rust
fn dwg_de_test() -> Vec<u8> {
    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Line(Line::from_coords(0., 0., 0., 10., 0., 0.))).unwrap();
    doc.add_entity(EntityType::Line(Line::from_coords(0., 0., 0., 0., 10., 0.))).unwrap();
    doc.add_entity(EntityType::Circle(Circle::from_coords(5., 5., 0., 2.))).unwrap();
    DwgWriter::write_to_vec(&doc).unwrap()
}
#[test]
fn compte_par_type_dans_l_espace_objet() {
    let m = mesurer(&dwg_de_test()).unwrap();
    assert_eq!(m.par_type.get("LINE"), Some(&2));
    assert_eq!(m.par_type.get("CIRCLE"), Some(&1));
    assert_eq!((m.total, m.inconnues), (3, 0));
}
#[test]
fn dwg_tronque_ne_panique_pas() {
    let o = dwg_de_test();
    match mesurer(&o[..o.len() / 2]) {
        Err(_) => {}
        Ok(m) => assert!(m.total < 3 || m.erreurs_recuperees > 0),
    }
}
#[test]
fn emprise_absente_si_en_tete_non_renseigne() {
    assert_eq!(emprise_depuis([1e20, 1e20], [-1e20, -1e20]), None);
    assert_eq!(emprise_depuis([1.0, 2.0], [3.0, 4.0]), Some([1.0, 2.0, 3.0, 4.0]));
}
#[test]
#[ignore]
fn generer_fixture() {
    std::fs::write("tests/fixtures/deux_lignes_un_cercle.dwg", dwg_de_test()).unwrap();
}
```

Dans `crates/bench/tests/dossier.rs` : un dossier temporaire contient un ZIP avec la fixture et `casse.dwg` (`b"abc"`). Après `mesurer_dossier`, `acadrust.csv` a une ligne `LINE` à 2 et une ligne `casse.dwg` dont `erreur` n'est pas vide.

- [ ] **Step 2 :** `cargo test -p cad --test mesure && cargo test -p bench`. Attendu : échec, fonctions introuvables.
- [ ] **Step 3 :** implémenter `mesure.rs`, puis `mesurer_dossier` et la CLI. `emprise_depuis` renvoie `None` si un min dépasse son max ou si une valeur absolue atteint 1e19. Durée mesurée par l'appelant (`Instant` en natif).
- [ ] **Step 4 :** relancer. Attendu : tous les tests passent. Puis `cargo test -p cad -- --ignored generer_fixture` et versionner la fixture.
- [ ] **Step 5 :** commit `feat(cad,bench): mesure des DWG et banc natif`.

### Task 4 : Lecture dans le navigateur (WASM)

**Files :**
- Create : `crates/wasm/src/lib.rs`, `essais/banc/index.html`, `essais/banc/banc.js`, `essais/banc/worker.js`
- Test : `crates/wasm/tests/web.rs`

**Interfaces :**
- Consumes : `inventorier`, `lire_entree`, `mesurer` (tâches 2 et 3).
- Produces : `#[wasm_bindgen] pub fn mesurer_zip(octets: &[u8]) -> Result<String, JsError>`. Elle renvoie un JSON `[{"chemin": "...", "mesure": {...}} | {"chemin": "...", "erreur": "..."}]`, un élément par DWG du ZIP.

- [ ] **Step 1 : écrire le test qui échoue** (`wasm-bindgen-test`, cible Node)

```rust
#[wasm_bindgen_test]
fn mesure_un_zip_avec_un_dwg_valide_et_un_casse() {
    let z = zip_de_test(&[("a.dwg", include_bytes!("../../cad/tests/fixtures/deux_lignes_un_cercle.dwg")), ("b.dwg", b"abc")]);
    let json = mesurer_zip(&z).unwrap();
    assert!(json.contains("\"LINE\":2"));
    assert!(json.contains("\"chemin\":\"b.dwg\",\"erreur\""));
}
```

`zip_de_test` est le même helper que celui de la tâche 2, recopié dans ce test.

- [ ] **Step 2 :** `wasm-pack test --node crates/wasm`. Attendu : échec, `mesurer_zip` introuvable.
- [ ] **Step 3 :** implémenter `mesurer_zip` ; dépendance `acadrust = { version = "0.6.8", default-features = false }`. Dans la page, `worker.js` charge le module, reçoit un ZIP déposé ou choisi et appelle `mesurer_zip`. Il chronomètre chaque ZIP avec `performance.now()` et renvoie le tableau ; `banc.js` l'affiche (fichier, DWG, entités, inconnues, durée) avec un bouton « Copier en CSV » au schéma commun, `lecteur` = `acadrust-wasm`.
- [ ] **Step 4 :** relancer le test. Attendu : PASS. Puis pousser et vérifier sur l'URL Pages, sur PC puis sur iPad : un ZIP du corpus donne les mêmes comptages que le banc natif.
- [ ] **Step 5 :** commit `feat(wasm): mesure des ZIP dans un worker`.

### Task 5 : Référence ODA, banc libredwg-web et comparaison

**Files :**
- Create : `tools/requirements.txt` (ezdxf, pyproj, shapely), `tools/reference.py`, `tools/comparer.py`, `essais/libredwg/package.json`, `essais/libredwg/mesurer.mjs`
- Test : `tools/tests/test_reference.py`, `tools/tests/test_comparer.py`, `essais/libredwg/test/mesurer.test.mjs`

**Interfaces :**
- Consumes : schéma CSV commun ; fixture `deux_lignes_un_cercle.dwg` (tâche 3).
- Produces :
  - `compter_dxf(chemin: Path) -> dict[str, int]` : espace objet, hors sous-entités exclues
  - `taux(ref: dict[str, int], lu: dict[str, int]) -> float | None` : Σ min(lu, ref) / Σ ref ; `None` si la référence est vide
  - CLI `python tools/reference.py corpus/ref corpus/resultats/reference.csv`
  - CLI `python tools/comparer.py corpus/resultats docs/phase0/lecteurs.md` : taux global, taux minimum par fichier, nombre d'erreurs, durée médiane, par lecteur, avec les identifiants anonymes `P01`…
  - CLI `node essais/libredwg/mesurer.mjs corpus/zips corpus/resultats/libredwg.csv` : `LibreDwg.create()`, puis `dwg_read_data(buffer, Dwg_File_Type.DWG)` ; un pointeur `undefined` donne une ligne d'erreur ; sinon `convert(ptr)` et comptage de `db.entities` par `type`, puis `dwg_free(ptr)`. Décompression des ZIP avec `fflate`.

- [ ] **Step 1 : écrire les tests qui échouent**

```python
def test_compter_dxf_ne_compte_que_l_espace_objet(tmp_path):
    doc = ezdxf.new("R2018"); msp = doc.modelspace()
    msp.add_line((0, 0), (1, 0)); msp.add_line((0, 0), (0, 1)); msp.add_circle((0, 0), 1)
    doc.blocks.new("B").add_line((0, 0), (1, 1))
    doc.saveas(tmp_path / "t.dxf")
    assert compter_dxf(tmp_path / "t.dxf") == {"LINE": 2, "CIRCLE": 1}

def test_taux_ignore_les_types_en_plus():
    assert taux({"LINE": 10, "CIRCLE": 5}, {"LINE": 10, "CIRCLE": 4, "HATCH": 3}) == pytest.approx(14 / 15)

def test_taux_exclut_les_sous_entites():
    assert taux({"LINE": 1, "ATTRIB": 5}, {"LINE": 1}) == 1.0

def test_taux_reference_vide():
    assert taux({}, {}) is None
```

```js
test("libredwg compte la fixture", async () => {
  const lignes = await mesurerZip(zipAvec("a.dwg", fixture));
  assert.deepEqual(lignes.filter(l => l.type).map(l => [l.type, l.nombre]).sort(), [["CIRCLE", 1], ["LINE", 2]]);
});
```

- [ ] **Step 2 :** `pytest tools/tests -q && node --test essais/libredwg/test`. Attendu : échecs, fonctions introuvables.
- [ ] **Step 3 :** implémenter les trois scripts.
- [ ] **Step 4 :** relancer. Attendu : tous les tests passent.
- [ ] **Step 5 :** commit `feat(tools): référence ezdxf, banc libredwg-web, comparaison`.

### Task 6 : Emprises du corpus et contour de la Métropole

**Files :**
- Create : `data/limite_metropole.geojson` (copie du jeu data.gouv.fr), `tools/emprises.py`
- Test : `tools/tests/test_emprises.py`

**Interfaces :**
- Consumes : `corpus/resultats/emprises.csv` (tâche 3).
- Produces :
  - `classer(emprise: tuple[float, float, float, float] | None, contour: Polygon) -> tuple[str, float | None]`, classe parmi `dedans`, `a_cheval`, `dehors`, `inconnue`
  - CLI `python tools/emprises.py corpus/resultats/emprises.csv data/limite_metropole.geojson docs/phase0/emprises.md` : contour reprojeté en EPSG:2154 avec pyproj, identifiants anonymes.

Règle : essayer les facteurs 1, puis 0,01, puis 0,001 ; le premier qui donne `dedans` ou `a_cheval` est retenu ; sinon `("dehors", None)`.

- [ ] **Step 1 : écrire les tests qui échouent**

```python
CARRE = Polygon([(0, 0), (100, 0), (100, 100), (0, 100)])
def test_emprise_inconnue():           assert classer(None, CARRE) == ("inconnue", None)
def test_dedans_en_metres():           assert classer((10, 10, 20, 20), CARRE) == ("dedans", 1)
def test_plan_en_centimetres():        assert classer((1000, 1000, 2000, 2000), CARRE) == ("dedans", 0.01)
def test_plan_en_millimetres():        assert classer((10000, 10000, 20000, 20000), CARRE) == ("dedans", 0.001)
def test_a_cheval():                   assert classer((90, 90, 110, 110), CARRE) == ("a_cheval", 1)
def test_dehors():                     assert classer((200, 200, 210, 210), CARRE)[0] == "dehors"
```

- [ ] **Step 2 :** `pytest tools/tests/test_emprises.py -q`. Attendu : échec.
- [ ] **Step 3 :** implémenter `classer` avec shapely (`box`, `within`, `intersects`) et la CLI.
- [ ] **Step 4 :** relancer. Attendu : 6 tests passent.
- [ ] **Step 5 :** commit `feat(tools): classement des emprises par rapport au contour`.

### Task 7 : Lambert 93 vers Web Mercator

**Files :**
- Create : `crates/georef/src/lib.rs`
- Test : `crates/georef/tests/l93.rs`

**Interfaces :**
- Produces : `pub fn l93_vers_mercator(x: f64, y: f64) -> Result<(f64, f64), ErreurGeoref>`

Approche décidée : proj4rs convertit Lambert 93 en longitude et latitude GRS80. La source est `+proj=lcc +lat_0=46.5 +lon_0=3 +lat_1=49 +lat_2=44 +x_0=700000 +y_0=6600000 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs`, la cible `+proj=longlat +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +no_defs`, en radians. Puis la formule sphérique de Web Mercator : x = R·λ, y = R·ln(tan(π/4 + φ/2)), R = 6 378 137 m. On évite ainsi tout changement de datum implicite vers la sphère.

- [ ] **Step 1 : écrire les tests qui échouent** (valeurs de référence calculées avec PROJ)

```rust
fn proche(a: (f64, f64), b: (f64, f64)) { assert!((a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3, "{a:?} != {b:?}"); }
#[test] fn origine_lambert_93() { proche(l93_vers_mercator(700000.0, 6600000.0).unwrap(), (333958.4723798207, 5860839.8299478)); }
#[test] fn point_a_nice()       { proche(l93_vers_mercator(1044000.0, 6298000.0).unwrap(), (809111.0042919414, 5418722.747879568)); }
```

- [ ] **Step 2 :** `cargo test -p georef`. Attendu : échec.
- [ ] **Step 3 :** implémenter (API de proj4rs d'après son README : `Proj::from_proj_string`, `transform::transform(&src, &dst, &mut (x, y, z))`).
- [ ] **Step 4 :** relancer. Attendu : 2 tests passent, écart sous 1 mm.
- [ ] **Step 5 :** commit `feat(georef): Lambert 93 vers Web Mercator`.

### Task 8 : Export de polylignes pour l'essai de rendu

**Files :**
- Create : `crates/cad/src/lignes.rs`, sous-commande dans `crates/bench/src/main.rs`
- Test : `crates/cad/tests/lignes.rs`
- Create : `essais/carte/test/deux_polylignes.lignes` (fixture du contrat avec la tâche 9)

**Interfaces :**
- Consumes : `lire_document` (tâche 3), `l93_vers_mercator` (tâche 7).
- Produces :
  - `pub struct Polyligne { pub points: Vec<[f64; 2]> }`
  - `pub fn extraire_lignes(doc: &CadDocument) -> Vec<Polyligne>` : LINE et sommets des LWPOLYLINE de l'espace objet, renflements ignorés en phase 0
  - `pub fn ecrire_lignes(polys: &[Polyligne]) -> Vec<u8>`. Format `.lignes`, petit-boutiste : `b"LIG0"`, u32 nombre de polylignes, puis pour chacune u32 n et n × (f64 x, f64 y).
  - CLI `bench export-lignes <zip> <chemin_dwg> <facteur> <sortie.lignes>` : coordonnées × facteur, puis `l93_vers_mercator`

- [ ] **Step 1 : écrire les tests qui échouent**

```rust
#[test]
fn extrait_lignes_et_lwpolylignes() {
    let doc = doc_avec_deux_lignes_et_une_lwpolyligne_de_trois_sommets();
    let p = extraire_lignes(&doc);
    assert_eq!(p.iter().map(|l| l.points.len()).collect::<Vec<_>>(), vec![2, 2, 3]);
}
#[test]
fn format_lignes() {
    let o = ecrire_lignes(&[Polyligne { points: vec![[0., 0.], [1., 1.]] }, Polyligne { points: vec![[2., 2.], [3., 3.], [4., 4.]] }]);
    assert_eq!(&o[..4], b"LIG0");
    assert_eq!(o.len(), 96);
}
```

Le document du premier test est construit comme dans la tâche 3 (`CadDocument::new`, deux `Line::from_coords`), plus une `LwPolyline` de trois sommets créée avec le constructeur du crate.

- [ ] **Step 2 :** `cargo test -p cad --test lignes`. Attendu : échec.
- [ ] **Step 3 :** implémenter, puis écrire la fixture `deux_polylignes.lignes` à partir des deux polylignes du test `format_lignes`.
- [ ] **Step 4 :** relancer. Attendu : PASS.
- [ ] **Step 5 :** commit `feat(cad): export de polylignes pour l'essai de rendu`.

### Task 9 : Page d'essai carte : fond Esri, périmètre, couche WebGL

**Files :**
- Create : `essais/carte/index.html`, `essais/carte/carte.js`, `essais/carte/couche-lignes.js`, `essais/carte/outils.js`
- Test : `essais/carte/test/outils.test.mjs`

**Interfaces :**
- Consumes : `data/limite_metropole.geojson` (tâche 6) ; fichiers `.lignes` (tâche 8).
- Produces :
  - `echelleMini(largeurM, hauteurM, largeurPx, hauteurPx) -> number` = max(largeurM / largeurPx, hauteurM / hauteurPx) × 96 / 0,0254 × 1,05
  - `lireLignes(arrayBuffer) -> Float64Array[]` (x, y entrelacés)
  - `CoucheLignes` : sous-classe de `Layer` dont la vue est une sous-classe de `BaseLayerViewGL2D` avec le support de tuilage

- [ ] **Step 1 : écrire les tests qui échouent**

```js
test("échelle minimale pour voir toute la Métropole", () => {
  assert.ok(Math.abs(echelleMini(40000, 60000, 1000, 800) - 297637.795) < 0.01);
});
test("lecture du format .lignes", async () => {
  const p = lireLignes((await readFile(new URL("./deux_polylignes.lignes", import.meta.url))).buffer);
  assert.deepEqual(p.map(a => Array.from(a)), [[0, 0, 1, 1], [2, 2, 3, 3, 4, 4]]);
});
```

- [ ] **Step 2 :** `node --test essais/carte/test`. Attendu : échec.
- [ ] **Step 3 :** implémenter `outils.js`, puis la page :
  - SDK 5.x par CDN, clé lue dans `localStorage` (`cle-esri`) ou saisie ; fonds `arcgis/imagery` et `arcgis/topographic`.
  - Voile : un polygone blanc à 50 %, grand rectangle percé du contour.
  - Contour : polyligne avec une variable visuelle de taille `valueExpression: "$view.scale"`, de 1,125 pt à l'échelle minimale à 4,5 pt à l'échelle 564,248588 (niveau 20).
  - `view.constraints` : `geometry` = contour, `minScale` = `echelleMini(...)` de l'étendue du contour, recalculé à chaque redimensionnement.
  - `CoucheLignes` : chargement d'un `.lignes` choisi par l'utilisateur, ou génération de N segments aléatoires dans l'emprise de Nice (N saisi, 500 000 par défaut) ; découpage par tuile en coordonnées relatives à l'origine de la tuile, tracé en `gl.LINES`.
  - Compteur d'images par seconde affiché.
  - Clé absente ou refusée : message explicite, contour et voile affichés sur fond neutre.
- [ ] **Step 4 :** relancer les tests (PASS), pousser, puis vérifier sur l'URL Pages :
  1. Impossible de dézoomer au-delà de la Métropole, sur PC, iPad et iPhone.
  2. Comportement de `geometry` noté : contrainte sur le centre de la vue ou sur toute la zone visible.
  3. Le trait s'épaissit en zoomant.
  4. Un plan réel exporté tombe exactement sur le cadastre de la Métropole.
  5. Images par seconde sur iPad avec 500 000 segments, notées.
  6. Sans clé : message affiché, pas d'écran blanc.
- [ ] **Step 5 :** commit `feat(essais): carte ArcGIS, périmètre et couche WebGL`.

### Task 10 : Sources open data : CORS et couverture

**Files :**
- Create : `catalogue.json`, `essais/sources/index.html`, `essais/sources/sources.js`
- Test : `essais/sources/test/sources.test.mjs`

**Interfaces :**
- Produces :
  - `classerReponse({ ok, status, json, erreurReseau }) -> string` parmi `OK`, `jeton requis` (code d'erreur ArcGIS 498 ou 499 dans le JSON), `CORS ou réseau` (`erreurReseau` vrai), sinon `HTTP <status>`
  - `classerCouverture(etendue, bboxNice, bboxMetropole) -> string` parmi `Nice`, `Métropole`, `inconnue` ; étendues `{xmin, ymin, xmax, ymax}` en EPSG:2154

`catalogue.json` contient au minimum les services vérifiés le 19 novembre 2026 :
- PLU/PLUm (`heberge2`) ;
- PLU_Annexes (`server`), couches 32, 36, 48, 49, 52, 53, 54 et 200 ;
- Cadastre (`heberge2`) ;
- Servitudes/PLU_Servitudes (`heberge2`), couches 26 et 28 ;
- la limite de la Métropole et le PPRI de la DDTM 06 (data.gouv.fr) ;
- Enedis HTA et BT ;
- le flux WFS de la Géoplateforme.

- [ ] **Step 1 : écrire les tests qui échouent**

```js
test("réponses", () => {
  assert.equal(classerReponse({ ok: true, status: 200, json: { layers: [] } }), "OK");
  assert.equal(classerReponse({ ok: true, status: 200, json: { error: { code: 499 } } }), "jeton requis");
  assert.equal(classerReponse({ erreurReseau: true }), "CORS ou réseau");
  assert.equal(classerReponse({ ok: false, status: 404 }), "HTTP 404");
});
test("couverture", () => {
  const nice = { xmin: 0, ymin: 0, xmax: 10, ymax: 10 }, nca = { xmin: -50, ymin: -50, xmax: 50, ymax: 50 };
  assert.equal(classerCouverture({ xmin: 1, ymin: 1, xmax: 9, ymax: 9 }, nice, nca), "Nice");
  assert.equal(classerCouverture({ xmin: -30, ymin: 1, xmax: 9, ymax: 9 }, nice, nca), "Métropole");
  assert.equal(classerCouverture(null, nice, nca), "inconnue");
});
```

- [ ] **Step 2 :** `node --test essais/sources/test`. Attendu : échec.
- [ ] **Step 3 :** implémenter, puis la page :
  - Interroger chaque source depuis l'origine Pages.
  - Pour chaque couche ArcGIS listée : `query?where=1%3D1&returnExtentOnly=true&outSR=2154&f=json`.
  - Boîte de Nice d'après le contour de la commune 06088 (API Découpage administratif) ; boîte de la Métropole d'après `data/limite_metropole.geojson`.
  - Tableau du résultat, avec un bouton « Copier en Markdown ».
- [ ] **Step 4 :** relancer les tests (PASS), pousser, ouvrir la page sur PC et sur iPad. Attendu : chaque source a un statut, chaque couche une couverture.
- [ ] **Step 5 :** commit `feat(essais): CORS et couverture des sources`.

### Task 11 : Corpus, campagne de mesure et rapport

**Files :**
- Create : `docs/phase0/rapport.md`, `docs/phase0/lecteurs.md`, `docs/phase0/emprises.md`
- Modify : document de conception (section Risques, version d'acadrust ; décisions de phase 0)

**Interfaces :**
- Consumes : toutes les CLI et pages précédentes.

- [ ] **Step 1 (utilisateur) :** déposer 15 à 20 ZIP eTransmit dans `corpus/zips/` : plans en cm, topos Covadis, xréfs imbriquées, XCLIP, blocs dynamiques, au moins un objet Civil 3D. Convertir tous les DWG avec ODA File Converter en DXF 2018 ASCII dans `corpus/ref/`, en gardant l'arborescence.
- [ ] **Step 2 :** lancer `bench mesurer`, `reference.py`, `mesurer.mjs`, `comparer.py`, `emprises.py`. Attendu : `lecteurs.md` et `emprises.md` générés, sans aucun nom de fichier client.
- [ ] **Step 3 :** reporter dans `rapport.md` :
  - les temps WASM du banc sur PC et sur iPad ;
  - les observations de la tâche 9 ;
  - le tableau de la tâche 10.
- [ ] **Step 4 :** proposer dans `rapport.md` :
  - le seuil de lecture, à confirmer avec l'utilisateur : taux global ≥ 99 %, aucun fichier sous 95 %, aucun plantage ;
  - le lecteur retenu ;
  - la cible de temps d'import, déduite des temps WASM ;
  - le traitement des xréfs de contexte qui débordent du contour.
- [ ] **Step 5 (utilisateur) :** valider ou corriger ces quatre points, puis reporter les décisions dans le document de conception.
- [ ] **Step 6 :** commit `docs(phase0): rapport et décisions`.
