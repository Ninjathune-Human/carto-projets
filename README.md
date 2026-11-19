![carto-projets](banniere.svg)

carto-projets verse des projets AutoCAD (ZIP eTransmit) sur une carte ArcGIS de la Métropole Nice Côte d'Azur. Le moteur est écrit en Rust compilé en WebAssembly et les pages sont statiques, publiées sur GitHub Pages.

Page du projet : <https://ninjathune-human.github.io/carto-projets/>

Le dossier `corpus/` contient des fichiers de clients : il n'est jamais versionné.

## Campagne de mesure (T11)

Tout ce qui suit se lance depuis la racine du dépôt. Les ZIP, les DXF et les CSV bruts restent dans `corpus/`, qui n'est jamais versionné : seuls les rapports Markdown anonymisés (`P01`, `P01-1`…) peuvent être publiés.

### Disposition du corpus

- `corpus/zips/` : les ZIP eTransmit (15 à 20), un fichier `.zip` par projet.
- `corpus/ref/<nom du ZIP sans .zip>/<chemin du DWG dans le ZIP>.dxf` : la référence. Convertir chaque DWG avec ODA File Converter en DXF 2018 ASCII, en gardant l'arborescence du ZIP (option « récursif »). Exemple : le DWG `Plans/Façade.dwg` du ZIP `Villa.zip` devient `corpus/ref/Villa/Plans/Façade.dxf`. `reference.py` en déduit le ZIP et le chemin du DWG ; une autre disposition fait échouer la jointure des lecteurs.
- `corpus/resultats/` : tous les CSV de lecteurs et les rapports. `comparer.py` lit tous les `*.csv` de ce dossier qui ont une colonne `lecteur`.
- `data/limite_metropole.geojson` : contour de la Métropole en WGS84 (EPSG:4326), à déposer pour `emprises.py`.

### Commandes

1. Lecteur natif acadrust (écrit `acadrust.csv` et `emprises.csv` dans le dossier de sortie) :
   `cargo run --release -p carto-bench --bin bench -- mesurer corpus/zips corpus/resultats`
2. Lecteur libredwg-web (installer d'abord les dépendances : `npm ci --prefix essais/libredwg`) :
   `node essais/libredwg/mesurer.mjs corpus/zips corpus/resultats/libredwg.csv`
3. Référence ezdxf (installer d'abord les dépendances : `pip install -r tools/requirements.txt`) :
   `python3 tools/reference.py corpus/ref corpus/resultats/reference.csv`
4. Lecteur acadrust dans le navigateur : ouvrir la page `essais/banc/` du site publié (PC, puis iPad), y déposer les ZIP, cliquer sur « Copier en CSV » et enregistrer le contenu du presse-papiers dans `corpus/resultats/acadrust-wasm.csv`. Ce fichier doit rester dans `corpus/resultats/` : le comparateur l'anonymise, alors que le coller dans un rapport publierait les noms de fichiers des clients.
5. Comparaison des lecteurs à la référence (écrit aussi `corpus/resultats/correspondance.csv`, identifiants anonymes et noms réels, non versionné) :
   `python3 tools/comparer.py corpus/resultats corpus/resultats/lecteurs.md`
6. Emprises par rapport au contour de la Métropole (utilise le même `correspondance.csv`, donc les mêmes identifiants) :
   `python3 tools/emprises.py corpus/resultats/emprises.csv data/limite_metropole.geojson corpus/resultats/emprises.md`

Les trois lecteurs comptent les entités de l'espace objet seulement, hors `ATTRIB`, `SEQEND` et `VERTEX`. Les CSV bruts gardent les noms de types propres à chaque lecteur ; `comparer.py` les ramène aux types DXF d'ezdxf par une seule table (les objets personnalisés et proxys sont regroupés sous `PERSONNALISEE`). Les rapports `lecteurs.md` et `emprises.md` se copient ensuite dans `docs/phase0/`.
