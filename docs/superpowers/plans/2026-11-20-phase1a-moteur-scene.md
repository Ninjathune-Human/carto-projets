# Phase 1a : moteur de scène (géométrie), conversion, visionneuse et carte

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Importer un ZIP eTransmit tel quel en une scène fidèle à l'espace objet d'AutoCAD (géométrie, couleurs, épaisseurs, hachures pleines, xréfs, visibilité), convertie en Lambert 93 quel que soit le système du plan, puis l'afficher dans une visionneuse et directement sur la carte ArcGIS.

**Architecture:** Le crate `carto-cad` aplatit les DWG du ZIP (blocs, xréfs, visibilité) en une `Scene` de primitives ordonnées (traits et surfaces triangulées) dans les unités du dessin maître. Le crate `carto-georef` reconnaît le système à l'emprise de l'essentiel du dessin et convertit en Lambert 93 puis en Web Mercator. `carto-wasm` sérialise la scène au format binaire SCN1 ; un module JS commun la lit et la dessine en WebGL, dans la visionneuse et dans une couche de la carte.

**Tech Stack:** Rust (acadrust 0.6.8, proj4rs 0.3.1 avec la fonctionnalité `tiff`, lyon 1.0, i_overlay 9.2), wasm-bindgen, JS modules ES, WebGL, ArcGIS Maps SDK 5.2, node:test, pytest, pyproj (référence de test).

**Spec:** document de conception Claude Docs https://claude.ai/artifact/TEPnvhyJCkNw8caHP9px7N (rev 62), copie texte locale `.superpowers/conception-rev62.md`. Sections utiles : Pipeline d'import, Géoréférencement et conversion, Périmètre de la Métropole, Phasage (ligne 1a), Choix faits par Claude.

## Global Constraints

- Paquets : `carto-cad`, `carto-georef`, `carto-bench`, `carto-wasm` (ruling R7) ; commandes `cargo test -p carto-cad`, etc.
- Le corpus client (`corpus/`) n'est jamais versionné ni cité ; une sortie versionnée n'emploie que des identifiants anonymes.
- Interface en français, sobre ; ni emoji ni tiret cadratin ; une commande garde sa place dans tous les modes ; un seul bouton par fonction.
- Phase 1a couvre : LINE, LWPOLYLINE (largeurs, renflements), POLYLINE 2D et 3D, ARC, CIRCLE, ELLIPSE, SPLINE, HATCH et MPOLYGON pleins (dégradé = couleur pleine), SOLID, TRACE, INSERT (MINSERT, imbrication, miroir), xréfs. Tout autre type (TEXT, MTEXT, DIMENSION, MULTILEADER, WIPEOUT, hachures à motif, types de ligne…) est compté dans `Rapport.ignorees` par nom de type, pour la phase 1b.
- Tolérance de corde : 1 mm réel, soit `0.001 / metres_par_unite` en unités du dessin.
- `metres_par_unite` depuis INSUNITS : 1 → 0.0254, 2 → 0.3048, 4 → 0.001, 5 → 0.01, 6 → 1 ; toute autre valeur → 1.
- Couleurs : `Index(7)`, `ByBlock` au premier niveau et `None` → noir `[0,0,0]` ; `ByLayer` → couleur du calque effectif ; `Index(n)` → `acadrust::types::aci_to_rgb(n)` ; `Rgb` tel quel.
- Épaisseur : `Value(n)` → `n / 100` mm ; `Default` → 0 (trait fin) ; `ByLayer` / `ByBlock` résolus ; à l'écran `px = max(1, mm × 96 / 25.4)`.
- Visibilité : calque éteint, gelé, non imprimable ou nommé `Defpoints` (casse ignorée) → masqué ; entité `invisible` → masquée ; dans un bloc, le calque `0` prend le calque de l'insertion ; INSERT sur calque gelé → tout son contenu masqué ; INSERT sur calque éteint → seul le contenu du calque 0 masqué.
- Ordre d'affichage : la table SORTENTS du bloc quand elle existe, sinon l'ordre de la base ; la scène garde cet ordre et les rendus le respectent.
- Systèmes acceptés et définitions (proj4rs) :
  - Lambert 93 : `+proj=lcc +lat_0=46.5 +lon_0=3 +lat_1=49 +lat_2=44 +x_0=700000 +y_0=6600000 +ellps=GRS80 +units=m`
  - CC43 : `+proj=lcc +lat_0=43 +lon_0=3 +lat_1=42.25 +lat_2=43.75 +x_0=1700000 +y_0=2200000 +ellps=GRS80 +units=m`
  - CC44 : `+proj=lcc +lat_0=44 +lon_0=3 +lat_1=43.25 +lat_2=44.75 +x_0=1700000 +y_0=3200000 +ellps=GRS80 +units=m`
  - Lambert II étendu : `+proj=lcc +lat_1=46.8 +lat_0=46.8 +lon_0=0 +k_0=0.99987742 +x_0=600000 +y_0=2200000 +a=6378249.2 +rf=293.4660212936269 +pm=paris +nadgrids=fr_ign_ntf_r93.tif +units=m`
  - Lambert III Sud : idem avec `+lat_1=44.1 +lat_0=44.1 +k_0=0.999877499 +x_0=600000 +y_0=200000`
  - Lambert III carto : idem Lambert III Sud avec `+y_0=3200000`
- Emprise de la Métropole utilisée pour la reconnaissance : rectangle WGS84 `[6.80, 43.60, 7.50, 44.40]` (lon min, lat min, lon max, lat max), projeté dans chaque système sur une grille de 3 × 3 points.
- Grille NTF → RGF93 : `fr_ign_ntf_r93.tif` (IGN, Licence Ouverte Etalab 2.0, via OSGeo/PROJ-data), versionnée dans `crates/georef/data/`, chargée depuis les octets (pas de fichier en wasm).
- Message de refus exact : `Plan non géoréférencé : aucun système reconnu (Lambert 93, CC43, CC44, Lambert II étendu, Lambert III Sud ou carto).`
- Un débord hors Métropole n'est jamais une erreur : tout est importé.
- Format SCN1 (petit-boutiste) : `SCN1` ; u32 nombre de calques puis, par calque, u32 longueur, nom UTF-8, u8 r, u8 g, u8 b, u8 0 ; u32 nombre de primitives puis, par primitive, u8 genre (0 trait, 1 surface), u8 r, g, b, a, u32 indice de calque, f32 épaisseur en mm (0 pour une surface), u32 n, n × (f64 x, f64 y) ; enfin u32 longueur et rapport JSON UTF-8. Une surface a un n multiple de 3 (triangles). Un trait fermé répète son premier point.
- Commits : messages en français, terminés par une ligne vide puis
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
  `Claude-Session: https://claude.ai/code/session_01AAmNf5tN2efFuHSicXAihy`
- Pas de push sans accord ; tests Node toujours en forme glob entre guillemets (`node --test 'dossier/test/*.test.mjs'`).

## Review Focus

1. Un objet égaré près de l'origine (0,0) dans un plan en Lambert 93 : le système est quand même reconnu, l'objet est importé (test en tâche 8 et en tâche 9).
2. Un plan maître en cm avec une xréf en m : la xréf est mise à l'échelle ×100 (test en tâche 7).
3. Un bloc en miroir (échelle X négative, ou normale d'extrusion −Z) : géométrie en miroir et arcs dans le bon sens (test en tâche 5).
4. Une hachure pleine avec un îlot : le trou reste vide (tests en tâches 3 et 4).
5. Un ZIP sans DWG, ou avec deux dessins maîtres possibles : erreur structurée, aucune panique, la page propose le choix (tests en tâches 7 et 9).

---

### Task 1 : Scène et format SCN1

**Files:**
- Create: `crates/cad/src/scene.rs`, `crates/cad/tests/scene.rs`, `essais/commun/scene.js`, `essais/commun/test/scene.test.mjs`, `essais/commun/test/deux_primitives.scn`
- Modify: `crates/cad/src/lib.rs`

**Interfaces:**
- Produces (Rust) : `pub struct Scene { pub calques: Vec<Calque>, pub primitives: Vec<Primitive> }`, `pub struct Calque { pub nom: String, pub couleur: [u8; 3] }`, `pub enum Primitive { Trait { calque: u32, couleur: [u8; 4], epaisseur_mm: f32, points: Vec<[f64; 2]> }, Surface { calque: u32, couleur: [u8; 4], triangles: Vec<[f64; 2]> } }`, `#[derive(Serialize, Default)] pub struct Rapport { pub maitre: String, pub dessins: Vec<String>, pub ignorees: BTreeMap<String, u64>, pub avertissements: Vec<String>, pub systeme: Option<String> }`, `pub fn ecrire_scene(scene: &Scene, rapport: &Rapport) -> Vec<u8>`, `impl Scene { pub fn calque(&mut self, nom: &str, couleur: [u8; 3]) -> u32 }` (indice existant ou nouveau), `pub fn points_mut(&mut self) -> impl Iterator<Item = &mut [f64; 2]>`.
- Produces (JS) : `lireScene(arrayBuffer) -> { calques: [{nom, couleur}], primitives: [{genre: "trait" | "surface", couleur: [r,g,b,a], calque, epaisseurMm, points: Float64Array}], rapport }`.

- [ ] **Step 1 : tests Rust** `scene_vide_fait_12_octets_plus_rapport` (en-tête `SCN1`, 0 calque, 0 primitive, puis la longueur du JSON), `trait_et_surface_serialises` (un calque « A » rouge, un trait de 2 points épaisseur 0.25, une surface d'un triangle : longueur totale calculée à la main, octets du genre et de la couleur à leur position), `calque_renvoie_le_meme_indice` (deux appels « A » → 0, « B » → 1).
- [ ] **Step 2 :** `cargo test -p carto-cad --test scene` → échec (module absent).
- [ ] **Step 3 :** implémenter `scene.rs`, exporter dans `lib.rs`.
- [ ] **Step 4 :** test ignoré `generer_fixture` qui écrit `essais/commun/test/deux_primitives.scn` (la scène du test 2) ; le lancer avec `--ignored`.
- [ ] **Step 5 : test Node** `lit_deux_primitives` : `lireScene` sur la fixture → 1 calque « A » `[255,0,0]`, primitive 0 trait de 2 points avec `epaisseurMm` ≈ 0.25, primitive 1 surface de 3 points, `rapport.maitre` présent ; `rejette_signature` : un tampon commençant par `XXXX` lève une erreur dont le message contient `SCN1`.
- [ ] **Step 6 :** `node --test 'essais/commun/test/*.test.mjs'` échoue, implémenter `scene.js`, puis vert.
- [ ] **Step 7 :** commit `feat(cad): scène et format SCN1`.

### Task 2 : Propriétés héritées et visibilité

**Files:**
- Create: `crates/cad/src/proprietes.rs`, `crates/cad/tests/proprietes.rs`
- Modify: `crates/cad/src/lib.rs`

**Interfaces:**
- Consumes : `acadrust::CadDocument`, `EntityCommon`.
- Produces : `#[derive(Clone)] pub struct Herite { pub calque: String, pub couleur: [u8; 3], pub epaisseur_mm: f32, pub alpha: u8 }`, `pub fn resoudre(doc: &CadDocument, e: &EntityCommon, parent: Option<&Herite>) -> Herite`, `pub fn calque_visible(doc: &CadDocument, nom: &str) -> bool`, `pub fn calque_gele(doc: &CadDocument, nom: &str) -> bool`, `pub fn couleur_aci(index: u8) -> [u8; 3]`, `pub fn metres_par_unite(insunits: i16) -> f64`.

- [ ] **Step 1 : tests** (documents construits en mémoire comme dans `crates/cad/tests/lignes.rs`) :
  - `couleur_7_et_bybloc_premier_niveau_en_noir` ;
  - `bycalque_prend_la_couleur_du_calque` (calque index 1 → `couleur_aci(1)`) ;
  - `calque_0_herite_du_parent` (parent calque « Murs » rouge, entité calque 0 couleur ByLayer → calque « Murs », rouge) ;
  - `bybloc_herite_du_parent` (couleur et épaisseur) ;
  - `epaisseur_value_en_mm` (`Value(35)` → 0.35 ; `Default` → 0) ;
  - `transparence_en_alpha` (0 % → 255 ; 50 % → 128 à ±1) ;
  - `calques_masques` : éteint, gelé, non imprimable, `defpoints` → `calque_visible` faux ; calque absent de la table → vrai ;
  - `unites` : `metres_par_unite(5)` = 0.01, `(4)` = 0.001, `(0)` = 1.
- [ ] **Step 2 :** `cargo test -p carto-cad --test proprietes` → échec.
- [ ] **Step 3 :** implémenter selon les règles des Global Constraints (lire le type `Transparency` d'acadrust pour la conversion en alpha).
- [ ] **Step 4 :** vert ; commit `feat(cad): propriétés héritées et visibilité des calques`.

### Task 3 : Géométrie (courbes, bandes, triangulation)

**Files:**
- Create: `crates/cad/src/geometrie.rs`, `crates/cad/tests/geometrie.rs`
- Modify: `crates/cad/Cargo.toml` (`lyon = "1.0"`), `crates/cad/src/lib.rs`

**Interfaces:**
- Produces : `pub fn arc_points(centre: [f64; 2], rayon: f64, debut: f64, fin: f64, tol: f64) -> Vec<[f64; 2]>` (radians, sens trigonométrique de `debut` à `fin`, `fin < debut` → `fin + 2π`, extrémités incluses), `pub fn renflement_points(a: [f64; 2], b: [f64; 2], renflement: f64, tol: f64) -> Vec<[f64; 2]>` (sans `a`, avec `b` ; renflement positif = sens trigonométrique), `pub fn ellipse_points(centre: [f64; 2], grand_axe: [f64; 2], rapport: f64, t0: f64, t1: f64, tol: f64) -> Vec<[f64; 2]>`, `pub fn spline_points(degre: usize, noeuds: &[f64], controles: &[[f64; 2]], poids: Option<&[f64]>, tol: f64) -> Vec<[f64; 2]>` (de Boor, NURBS si poids), `pub fn bande(points: &[[f64; 2]], largeurs: &[(f64, f64)], fermee: bool) -> Vec<[f64; 2]>` (triangles ; `largeurs[i]` = largeur de début et de fin du segment i), `pub fn trianguler(anneaux: &[Vec<[f64; 2]>]) -> Vec<[f64; 2]>` (remplissage pair-impair, lyon).
- Le nombre de segments d'un arc d'angle `θ` et de rayon `r` est `ceil(θ / (2 acos(1 − tol / r)))`, au moins 1, au plus 4096.

- [ ] **Step 1 : tests** :
  - `quart_de_cercle` : rayon 10, tol 0.001 : chaque point à 10 ± 1e-9 du centre, flèche maximale ≤ 0.001, premier point (10,0), dernier (0,10) ;
  - `renflement_1_demi_cercle` : de (0,0) à (2,0), renflement 1 → le point du milieu de la liste est (1,−1) à 1e-9 près, dernier point (2,0) ;
  - `ellipse_complete` : centre 0, grand axe (2,0), rapport 0.5 : chaque point vérifie (x/2)² + y² = 1 à 1e-9 ;
  - `spline_degre_2` : contrôles (0,0) (1,2) (2,0), nœuds [0,0,0,1,1,1] : premier point (0,0), dernier (2,0), un point à (1,1) à 1e-6 près ;
  - `spline_degre_1_rend_les_controles` ;
  - `bande_segment` : (0,0)–(10,0), largeur 2 → aire totale des triangles 20, tous les y dans [−1, 1] ;
  - `ilot_reste_vide` : carré 0..10 avec trou 2..8 → aire 64, aucun barycentre de triangle dans ]2,8[².
- [ ] **Step 2 :** `cargo test -p carto-cad --test geometrie` → échec.
- [ ] **Step 3 :** implémenter (lyon `FillTessellator` avec `FillRule::EvenOdd` pour `trianguler` ; bandes calculées directement, joints en onglet limités à 4 × la demi-largeur).
- [ ] **Step 4 :** vert ; commit `feat(cad): discrétisation, bandes et triangulation`.

### Task 4 : Contours de hachures

**Files:**
- Create: `crates/cad/src/hachures.rs`, `crates/cad/tests/hachures.rs`
- Modify: `crates/cad/src/lib.rs`

**Interfaces:**
- Consumes : `geometrie::{arc_points, ellipse_points, spline_points, renflement_points}`.
- Produces : `pub fn anneaux_hachure(h: &acadrust::entities::Hatch, tol: f64) -> Vec<Vec<[f64; 2]>>` (un anneau fermé par chemin, arêtes ligne, arc circulaire, arc elliptique, spline et polyligne à renflements ; sens horaire des arcs respecté via `counter_clockwise`).

- [ ] **Step 1 : tests** (hachures construites en mémoire) : `chemin_polyligne_carre` (4 sommets → anneau de 4 points, aire 100) ; `chemin_arcs` (deux demi-cercles de rayon 5 → anneau dont tous les points sont à 5 ± 1e-9 du centre) ; `arc_horaire` (même demi-cercle, `counter_clockwise` faux → points de l'autre côté de la corde) ; `ilot` (deux chemins, carré et trou) → `trianguler` des anneaux donne une aire de 64.
- [ ] **Step 2 :** `cargo test -p carto-cad --test hachures` → échec ; implémenter ; vert.
- [ ] **Step 3 :** commit `feat(cad): contours de hachures`.

### Task 5 : Aplatissement d'un dessin

**Files:**
- Create: `crates/cad/src/aplatir.rs`, `crates/cad/tests/aplatir.rs`
- Modify: `crates/cad/src/lib.rs`, `crates/cad/src/erreur.rs` si un variant manque

**Interfaces:**
- Consumes : tâches 1 à 4.
- Produces : `pub struct Dessins { pub docs: BTreeMap<String, CadDocument> }` (clé = chemin normalisé dans le ZIP), `pub struct Aplatissement { pub scene: Scene, pub rapport: Rapport }`, `pub fn aplatir(dessins: &Dessins, maitre: &str) -> Result<Aplatissement, ErreurCad>`. Coordonnées de sortie : unités du dessin maître. Une INSERT de xréf est ignorée avec un avertissement `xréf non résolue : <nom>` (la tâche 7 la remplacera).
- Transformation d'une INSERT (affine 2D) : `T(point d'insertion) · R(rotation) · T(c · espacement colonnes, r · espacement lignes) · S(échelle x, échelle y) · T(−point de base)` ; une normale d'extrusion de z négatif applique un miroir x → −x aux coordonnées OCS (entité et INSERT) ; une autre normale non verticale → entité comptée dans `avertissements`.
- Les courbes sont discrétisées dans le repère du bloc avec `tol / échelle maximale du repère`, puis transformées.
- LWPOLYLINE et POLYLINE 2D : largeur constante ou par sommet non nulle → `Surface` (bande), sinon `Trait` ; renflements discrétisés.

- [ ] **Step 1 : tests** (documents en mémoire, aide `definir_bloc` reprise de `tests/lignes.rs`) :
  - `entites_simples` : une LINE, un ARC, un CIRCLE, une ELLIPSE, une LWPOLYLINE à largeur 0.5, une HATCH pleine, un SOLID → 5 traits et 2 surfaces + 1 surface (SOLID), couleurs résolues ;
  - `bloc_rotation_echelle_base` : reprend `bloc_insere_avec_point_de_base_rotation_et_echelle` de `tests/lignes.rs` sur la scène ;
  - `bloc_miroir_echelle_negative` : bloc contenant l'arc (1,0)→(0,1) de centre 0, inséré en (10,0) échelle (−1, 1) : tous les points ont x ≤ 10 et le premier point est (9,0) ;
  - `normale_negative` : un CIRCLE de centre OCS (5,0) et normale (0,0,−1) → centre réel (−5,0) ;
  - `minsert_deux_colonnes` ; `bloc_imbrique` ;
  - `calque_eteint_masque_le_calque_0_seulement` et `calque_gele_masque_tout` (bloc imbriqué sur calque 0 dans un bloc inséré sur calque éteint → masqué) ;
  - `ignorees_comptees` : un TEXT et une DIMENSION → `rapport.ignorees` vaut {"TEXT": 1, "DIMENSION_LINEAR": 1} (noms tels que `mesure.rs` les produit) ;
  - `ordre_conserve` : trois LINE → primitives dans l'ordre de la base.
- [ ] **Step 2 :** `cargo test -p carto-cad --test aplatir` → échec.
- [ ] **Step 3 :** implémenter le parcours récursif (profondeur maximale 16), `extraire_lignes` reste inchangé.
- [ ] **Step 4 :** vert, `cargo test --workspace` vert ; commit `feat(cad): aplatissement d'un dessin en scène`.

### Task 6 : XCLIP, blocs dynamiques et ordre d'affichage

**Files:**
- Modify: `crates/cad/src/aplatir.rs`, `crates/cad/tests/aplatir.rs`, `crates/cad/Cargo.toml` (`i_overlay = "9.2"`)

**Interfaces:**
- XCLIP : le `SpatialFilter` rattaché à l'INSERT (dictionnaire d'extension `ACAD_FILTER`, entrée `SPATIAL`), `display_enabled` vrai → contour `boundary_points` ramené dans le repère du bloc par `clip_bound_transform` puis `inverse_block_transform`, puis découpe : traits coupés au contour (polylignes résultantes), surfaces intersectées (i_overlay) puis retriangulées. Le sens inversé n'est pas lu par acadrust 0.6.8 : non traité, rien à signaler.
- Bloc dynamique référencé directement (non anonyme) : seules les entités de l'état de visibilité courant (`CadDocument::dynamic_visibility_for_insert`) sont gardées ; une INSERT d'un bloc anonyme `*U` n'est pas filtrée (AutoCAD y a déjà évalué l'état).
- SORTENTS : la `SortEntitiesTable` du bloc, quand elle existe, ordonne ses entités.

- [ ] **Step 1 : tests** : `xclip_coupe_un_trait` (LINE (−5,5)–(15,5) dans un bloc découpé au carré 0..10, transformations identité → un trait (0,5)–(10,5)) ; `xclip_coupe_une_surface` (hachure carré 0..20 découpée au carré 0..10 → aire 100) ; `xclip_desactive` (`display_enabled` faux → rien coupé) ; `visibilite_dynamique` (paramètre de visibilité construit en mémoire : état « B » courant, une entité de l'état A et une de l'état B → seule celle de B) ; `sortents_reordonne` (table qui place la 3e LINE en premier).
- [ ] **Step 2 :** échec ; implémenter ; vert ; `cargo test --workspace` vert.
- [ ] **Step 3 :** commit `feat(cad): XCLIP, blocs dynamiques et ordre d'affichage`.

### Task 7 : Xréfs et import d'un ZIP

**Files:**
- Create: `crates/cad/src/importer.rs`, `crates/cad/tests/importer.rs`
- Modify: `crates/cad/src/aplatir.rs`, `crates/cad/src/erreur.rs`, `crates/cad/src/lib.rs`

**Interfaces:**
- Produces : `pub fn charger_dessins(zip: &[u8]) -> Result<Dessins, ErreurCad>` (tous les `.dwg`, chemins normalisés `/`), `pub fn resoudre_chemin(chemin_enregistre: &str, depuis: &str, chemins: &[String]) -> Option<String>` (dans l'ordre : chemin enregistré normalisé, chemin relatif au dossier de l'hôte, nom de fichier seul ; casse ignorée), `pub fn trouver_maitres(dessins: &Dessins) -> Vec<String>` (dessins qu'aucun autre ne référence, triés), `pub fn importer_zip(zip: &[u8], maitre: Option<&str>) -> Result<Aplatissement, ErreurCad>`.
- Nouveaux variants : `ErreurCad::AucunDwg`, `ErreurCad::PlusieursMaitres(Vec<String>)`, `ErreurCad::MaitreInconnu(String)`.
- Aplatissement d'une xréf : contenu de l'espace objet du dessin cible, transformé par l'INSERT puis mis à l'échelle `metres_par_unite(xréf) / metres_par_unite(maître)` ; calques nommés `NOMXREF|calque` ; visibilité lue d'abord dans le calque `NOMXREF|calque` de l'hôte s'il existe (surcharges VISRETAIN), sinon dans la xréf ; xréf en superposition sous une autre xréf → non chargée ; xréf déchargée → ignorée avec avertissement ; cycle → avertissement `cycle de xréfs : a → b → a`, branche coupée ; fichier introuvable → avertissement `xréf introuvable : <chemin>`.

- [ ] **Step 1 : tests** (ZIP construits en mémoire avec la crate `zip` et des DWG écrits par `DwgWriter`) :
  - `zip_sans_dwg` → `AucunDwg` ;
  - `deux_maitres` → `PlusieursMaitres(["a.dwg", "b.dwg"])`, et `importer_zip(.., Some("b.dwg"))` réussit ;
  - `resolution_chemins` : `C:\Projet\Fonds\Topo.DWG` trouvé comme `fonds/topo.dwg` depuis `plan.dwg`, puis par nom seul ;
  - `xref_en_m_dans_maitre_en_cm` : LINE (0,0)–(1,0) dans la xréf en m → (0,0)–(100,0) dans la scène ;
  - `surcharge_visretain` : calque `TOPO|Arbres` éteint dans l'hôte, allumé dans la xréf → masqué ;
  - `cycle_coupe` : a référence b qui référence a → import réussi avec l'avertissement de cycle ;
  - `superposition_imbriquee_non_chargee`.
- [ ] **Step 2 :** échec ; implémenter ; vert ; `cargo test --workspace` vert.
- [ ] **Step 3 :** commit `feat(cad): xréfs et import d'un ZIP`.

### Task 8 : Reconnaissance du système et conversion

**Files:**
- Create: `crates/georef/src/systemes.rs`, `crates/georef/tests/systemes.rs`, `crates/georef/data/fr_ign_ntf_r93.tif`, `tools/reference_conversions.py`, `crates/georef/tests/fixtures/conversions.csv`
- Modify: `crates/georef/Cargo.toml` (`proj4rs = { version = "0.3.1", features = ["tiff"] }`), `crates/georef/src/lib.rs`, `tools/requirements.txt` (`pyproj`)

**Interfaces:**
- Produces : `#[derive(Clone, Copy, Debug, PartialEq)] pub enum Systeme { Lambert93, Cc43, Cc44, LambertIIEtendu, LambertIIISud, LambertIIICarto }`, `impl Systeme { pub const TOUS: [Systeme; 6]; pub fn nom(&self) -> &'static str }` (« Lambert 93 », « CC43 », « CC44 », « Lambert II étendu », « Lambert III Sud », « Lambert III carto »), `pub struct Reconnaissance { pub systeme: Systeme, pub facteur: f64 }` (unités du dessin → mètres : 1, 0.01 ou 0.001), `pub fn emprise_essentielle(points: &[[f64; 2]]) -> Option<[f64; 4]>` (percentiles 2 et 98 en x et en y ; `None` si vide), `pub fn reconnaitre(emprise: [f64; 4]) -> Option<Reconnaissance>` (premier système, puis premier facteur, dont l'emprise de la Métropole contient `emprise × facteur`), `pub struct Convertisseur` construit une fois par système (projections analysées une seule fois), `impl Convertisseur { pub fn nouveau(r: Reconnaissance) -> Result<Self, ErreurGeoref>; pub fn vers_l93(&self, x: f64, y: f64) -> Result<(f64, f64), ErreurGeoref>; pub fn vers_mercator(&self, x: f64, y: f64) -> Result<(f64, f64), ErreurGeoref> }`. `l93_vers_mercator` reste.
- Grille : téléchargée depuis `https://raw.githubusercontent.com/OSGeo/PROJ-data/master/fr_ign/fr_ign_ntf_r93.tif` (93 581 octets), embarquée par `include_bytes!`, enregistrée une fois dans le catalogue de proj4rs (`nadgrids::formats::read` sur un `Cursor`) sous la clé `fr_ign_ntf_r93.tif`.
- Référence : `tools/reference_conversions.py <grille.tif> <sortie.csv>` tire 1 000 points par système non-L93 dans l'emprise de la Métropole (graine fixe 20261120) et écrit `systeme,x,y,x93,y93` avec pyproj (`pyproj.datadir.append_data_dir` sur le dossier de la grille, `Transformer.from_crs(EPSG, 2154, always_xy=True)` ; EPSG 3943, 3944, 27572, 27563, 27573).

- [ ] **Step 1 :** télécharger la grille ; écrire et lancer le script de référence ; commit des deux fichiers avec le test à venir.
- [ ] **Step 2 : tests** :
  - `conversions_sous_le_millimetre` : pour chaque ligne du CSV, `vers_l93` à moins de 0.001 m en x et en y ;
  - `l93_identite` : `vers_l93` du Lambert 93 rend le point à 1e-9 près ;
  - `reconnaitre_chaque_systeme` : centre de l'emprise de chaque système (valeurs de contrôle : Lambert 93 x 1 002 528–1 063 264, y 6 285 267–6 376 983 ; CC43 y 2 273 593–2 365 152 ; CC44 y 3 162 627–3 254 220 ; Lambert II étendu y 1 854 473–1 946 729 ; Lambert III Sud y 154 215–246 222 ; Lambert III carto y 3 154 215–3 246 222) → bon système, facteur 1 ;
  - `reconnaitre_en_cm` : emprise Lambert 93 × 100 → Lambert 93, facteur 0.01 ;
  - `rien_hors_systemes` : emprise (0,0)–(100,100) → `None` ;
  - `objet_egare_ecarte` : 1 000 points dans un rectangle Lambert 93 de Nice et 5 points à (0,0) → `emprise_essentielle` reconnue Lambert 93 ;
  - `performance` : 100 000 conversions Lambert II étendu en moins de 2 s en `--release`.
- [ ] **Step 3 :** `cargo test -p carto-georef` → échec ; implémenter ; vert.
- [ ] **Step 4 :** commit `feat(georef): reconnaissance du système et conversion vers Lambert 93`.

### Task 9 : Import complet en wasm

**Files:**
- Modify: `crates/wasm/src/lib.rs`, `crates/wasm/tests/web.rs`, `crates/wasm/Cargo.toml` (dépendance `carto-georef`)
- Create: `essais/commun/worker-scene.js`

**Interfaces:**
- Produces : `pub enum Repere { L93, Mercator }`, `pub fn importer_zip_scn(octets: &[u8], repere: Repere, maitre: Option<&str>) -> Result<Vec<u8>, String>` (aplatissement, emprise essentielle de tous les points, reconnaissance, conversion de chaque point, `rapport.systeme` renseigné, SCN1), `#[wasm_bindgen] pub fn importer_zip(octets: &[u8], repere: &str, maitre: Option<String>) -> Result<Vec<u8>, JsError>` (`repere` = `"l93"` ou `"mercator"`).
- Erreurs : chaîne JSON `{"code":"aucun_dwg"}`, `{"code":"plusieurs_maitres","candidats":[…]}`, `{"code":"non_georeference","message":"<message exact des Global Constraints>"}`, `{"code":"lecture","message":"…"}`.
- Worker : message `{zip: ArrayBuffer, repere, maitre}` → réponse `{scn: ArrayBuffer}` (transféré) ou `{erreur: objet JSON}` ; importe `../banc/pkg/carto_wasm.js`.

- [ ] **Step 1 : tests natifs** : `importe_un_plan_l93` (ZIP en mémoire d'un DWG avec deux LINE en coordonnées Lambert 93 de Nice → SCN1 valide, 2 traits, `systeme` = « Lambert 93 ») ; `mercator_proche_de_l93_vers_mercator` (même plan en repère Mercator : premier point égal à `l93_vers_mercator` à 1e-6 près) ; `objet_egare_importe` (une LINE de plus à (0,0) → reconnu, 3 traits) ; `non_georeference` (plan autour de 0,0 → JSON `non_georeference`) ; `plusieurs_maitres_json`.
- [ ] **Step 2 :** échec ; implémenter ; vert ; `cargo test --workspace` vert ; `node --check essais/commun/worker-scene.js`.
- [ ] **Step 3 :** commit `feat(wasm): import d'un ZIP en scène convertie`.

### Task 10 : Rendu commun et visionneuse

**Files:**
- Create: `essais/commun/rendu-scene.js`, `essais/commun/test/rendu-scene.test.mjs`, `essais/visionneuse/index.html`, `essais/visionneuse/visionneuse.js`

**Interfaces:**
- Produces : `preparerTampons(scene, origine) -> { surfaces: {positions: Float32Array, couleurs: Uint8Array, nombre}, traits: {positions: Float32Array, autres: Float32Array, couleurs: Uint8Array, nombre}, ordre }` (fonction pure : coordonnées relatives à `origine`, triangles des surfaces, quadrilatères des traits dont la largeur à l'écran vient de `px = max(1, mm × 96 / 25.4)` calculée par le shader, profondeur = rang de la primitive pour respecter l'ordre), `creerRendu(gl) -> { charger(tampons), dessiner({centre, resolution, rotation, taille}), calques(visibles) }`.
- Visionneuse : un seul bouton « Ouvrir un projet (.zip) » et le dépôt d'un ZIP sur la page ; worker `../commun/worker-scene.js` en repère `l93` ; fond gris clair `#d9d9d9` ; déplacement et zoom à la souris, au doigt et au stylet (pointer events), zoom sur l'emprise à l'ouverture ; panneau repliable avec encoche : calques à cocher, rapport (système reconnu, dessins, types ignorés, avertissements) ; comparaison : « Ouvrir une capture » puis bascule « Côte à côte » / « Superposé » avec un curseur d'opacité ; en cas de `plusieurs_maitres`, une liste pour choisir puis un nouvel import.

- [ ] **Step 1 : tests Node** de `preparerTampons` : une surface d'un triangle et un trait de 2 points → 3 sommets de surface, 4 sommets de trait (un quadrilatère), positions relatives à l'origine, couleurs recopiées, profondeur du trait supérieure à celle de la surface ; un calque masqué via `calques()` n'est pas testé ici (navigateur).
- [ ] **Step 2 :** échec ; implémenter ; vert ; `node --check` sur chaque fichier.
- [ ] **Step 3 :** vérification navigateur différée (ruling R2) : liste de contrôle dans le rapport de tâche.
- [ ] **Step 4 :** commit `feat(essais): visionneuse de scène`.

### Task 11 : Dépôt du ZIP sur la carte

**Files:**
- Create: `essais/carte/couche-scene.js`
- Modify: `essais/carte/carte.js`, `essais/carte/index.html`, `essais/carte/outils.js`, `essais/carte/test/outils.test.mjs`

**Interfaces:**
- Consumes : `preparerTampons` et le principe de `creerCoucheLignes` (tuilage `BaseLayerViewGL2D`, coordonnées relatives à la tuile calculées en double précision côté JS).
- Produces : `indexerPrimitives(scene) -> index` dans `outils.js` (grille par emprise de primitive, même rôle que `indexerSegments`), `creerCoucheScene({ Layer, BaseLayerViewGL2D, TileInfo }, suivi) -> classe de couche` avec `definirScene(scene)`.
- Le bouton existant devient « Ouvrir un projet (.zip ou .lignes) », même place ; un ZIP se dépose aussi sur la carte ; worker `../commun/worker-scene.js` en repère `mercator` ; zoom sur l'emprise ; ce qui sort de la Métropole reste sous le voile (ordre des couches inchangé : projets sous voile et contour) ; erreurs affichées dans le panneau, choix du maître si besoin.

- [ ] **Step 1 : tests Node** `indexerPrimitives` : deux primitives éloignées tombent dans des cases différentes, une surface qui chevauche deux cases est dans les deux.
- [ ] **Step 2 :** échec ; implémenter ; vert ; `node --check` ; vérification navigateur différée (R2), liste de contrôle dans le rapport.
- [ ] **Step 3 :** commit `feat(carte): dépôt d'un ZIP eTransmit sur la carte`.

### Task 12 : Commande `scene`, aperçu et contrôle sur le projet réel

**Files:**
- Create: `tools/apercu_scene.py`, `tools/tests/test_apercu_scene.py`
- Modify: `crates/bench/Cargo.toml` (dépendance `carto-wasm` par chemin, pour `Repere` et `importer_zip_scn`), `crates/bench/src/main.rs`, `crates/bench/src/lib.rs`, `crates/bench/tests/` (nouveau test)

**Interfaces:**
- Consumes : `carto_wasm::{Repere, importer_zip_scn}` (tâche 9).
- Produces : `pub fn exporter_scene(zip: &Path, repere: Repere, maitre: Option<&str>, sortie: &Path) -> Result<String, String>` (rend le rapport JSON), sous-commande `bench scene <zip> <sortie.scn> [l93|mercator] [maitre]` ; `tools/apercu_scene.py <entree.scn> <sortie.png> [--fond #d9d9d9] [--largeur 2000]` dessine la scène dans l'ordre (surfaces en polygones, traits avec leur épaisseur en px).

- [ ] **Step 1 : tests** : `scene_cli` (fixture `crates/cad/tests/fixtures/deux_lignes_un_cercle.dwg` zippée en mémoire et placée à l'origine → erreur `non_georeference` en JSON, code de sortie 1) ; un DWG Lambert 93 en mémoire → fichier SCN1 lisible ; `test_apercu_scene` : la fixture `essais/commun/test/deux_primitives.scn` → PNG non vide des dimensions demandées.
- [ ] **Step 2 :** échec ; implémenter ; vert.
- [ ] **Step 3 :** lancer `bench scene` puis `apercu_scene.py` sur `corpus/zips/*.zip` (hors dépôt) ; noter dans le rapport de tâche les nombres de primitives, le système reconnu et les types ignorés, sans nom de fichier client ; l'image reste hors dépôt.
- [ ] **Step 4 :** commit `feat(bench): export de scène et aperçu PNG`.

### Task 13 : Documentation et publication

**Files:**
- Modify: `README.md`, `.github/workflows/pages.yml`, `index.html`

**Interfaces:**
- `pages.yml` : l'étape « Assembler le site » copie aussi `essais/visionneuse` et `essais/commun` (sans leurs dossiers `test`) ; les tests Node du workflow ajoutent le glob `'essais/commun/test/*.test.mjs'`.
- `index.html` : lien vers la visionneuse à côté des autres essais.
- `README.md` : section « Phase 1a » (ce que fait l'import, systèmes reconnus, limites 1a, attribution de la grille IGN sous Licence Ouverte Etalab 2.0).

- [ ] **Step 1 :** modifier ; YAML valide (`python3 -c "import yaml; yaml.safe_load(open('.github/workflows/pages.yml'))"`) ; simulation locale de l'assemblage `_site` sans le pkg wasm : présence de `essais/visionneuse/index.html` et `essais/commun/scene.js`.
- [ ] **Step 2 :** tous les tests (`cargo test --workspace`, les globs Node, `python3 -m pytest tools/tests`) verts.
- [ ] **Step 3 :** commit `docs: phase 1a, visionneuse et publication`.
