// CoucheLignes : couche WebGL personnalisée (BaseLayerViewGL2D avec tuilage) qui trace des
// segments en gl.LINES. Structure reprise de l'exemple Esri « custom-gl-tiles » (SDK 5.2).
// Les modules ArcGIS sont fournis par l'appelant ($arcgis.import), rien n'est importé ici.

import { indexerSegments } from "./outils.js";

export const TAILLE_TUILE = 512;

// Couleur du plan, prémultipliée (rouge brique #b3261e, opaque).
const COULEUR = [0xb3 / 255, 0x26 / 255, 0x1e / 255, 1];

// Sommets en mètres relatifs à l'origine de la tuile (coin haut gauche).
// Position écran = (sommet + origine de la tuile - centre de la vue) / résolution, puis rotation,
// puis passage en coordonnées normalisées. Le décalage est calculé en double précision côté JS.
const SOURCE_SOMMETS = `
precision highp float;
attribute vec2 a_position;
uniform vec2 u_decalage;
uniform float u_resolution;
uniform mat2 u_rotation;
uniform vec2 u_versNdc;
void main() {
  vec2 pixels = u_rotation * ((a_position + u_decalage) / u_resolution);
  gl_Position = vec4(pixels * u_versNdc, 0.0, 1.0);
}`;

const SOURCE_FRAGMENTS = `
precision mediump float;
uniform vec4 u_couleur;
void main() {
  gl_FragColor = u_couleur;
}`;

function compiler(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const journal = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error("shader refusé : " + journal);
  }
  return shader;
}

function creerProgramme(gl) {
  const vs = compiler(gl, gl.VERTEX_SHADER, SOURCE_SOMMETS);
  const fs = compiler(gl, gl.FRAGMENT_SHADER, SOURCE_FRAGMENTS);
  const programme = gl.createProgram();
  gl.attachShader(programme, vs);
  gl.attachShader(programme, fs);
  gl.bindAttribLocation(programme, 0, "a_position");
  gl.linkProgram(programme);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (!gl.getProgramParameter(programme, gl.LINK_STATUS)) {
    const journal = gl.getProgramInfoLog(programme);
    gl.deleteProgram(programme);
    throw new Error("programme refusé : " + journal);
  }
  return programme;
}

/**
 * Crée la couche et sa fonction d'alimentation.
 * modules : { Layer, BaseLayerViewGL2D, TileInfo } chargés par $arcgis.import.
 * suivi : rappels facultatifs { rendu(ms, nbTuiles), construction(nbTuiles, ms), erreur(e) }.
 */
export function creerCoucheLignes({ Layer, BaseLayerViewGL2D, TileInfo }, suivi = {}) {
  const partage = { index: null, version: 0, vues: new Set() };

  const VueLignes = BaseLayerViewGL2D.createSubclass({
    constructor: function () {
      this.tampons = new Map(); // id de tuile -> { tampon, nbSommets, ox, oy, version }
      this.programme = null;
      this.emplacements = null;
      this.enPanne = false;
    },

    attach() {
      const gl = this.context;
      try {
        this.programme = creerProgramme(gl);
      } catch (e) {
        this.enPanne = true;
        suivi.erreur?.(e);
        return;
      }
      this.emplacements = {
        decalage: gl.getUniformLocation(this.programme, "u_decalage"),
        resolution: gl.getUniformLocation(this.programme, "u_resolution"),
        rotation: gl.getUniformLocation(this.programme, "u_rotation"),
        versNdc: gl.getUniformLocation(this.programme, "u_versNdc"),
        couleur: gl.getUniformLocation(this.programme, "u_couleur"),
      };
      partage.vues.add(this);
    },

    detach() {
      partage.vues.delete(this);
      const gl = this.context;
      for (const entree of this.tampons.values()) if (entree.tampon) gl.deleteBuffer(entree.tampon);
      this.tampons.clear();
      if (this.programme) gl.deleteProgram(this.programme);
      this.programme = null;
    },

    // Les tampons des tuiles retirées sont libérés au rendu suivant (appels GL dans render seulement).
    tilesChanged() {
      this.requestRender();
    },

    construireTuile(gl, tuile) {
      const index = partage.index;
      const ox = tuile.coords[0];
      const oy = tuile.coords[1];
      const cote = TAILLE_TUILE * tuile.resolution;
      const trouves = index.chercher(ox, oy - cote, ox + cote, oy);
      const entree = { tampon: null, nbSommets: trouves.length * 2, ox, oy, version: partage.version };
      if (trouves.length > 0) {
        const { x0, y0, x1, y1 } = index;
        const sommets = new Float32Array(trouves.length * 4);
        for (let k = 0, o = 0; k < trouves.length; k++, o += 4) {
          const s = trouves[k];
          sommets[o] = x0[s] - ox;
          sommets[o + 1] = y0[s] - oy;
          sommets[o + 2] = x1[s] - ox;
          sommets[o + 3] = y1[s] - oy;
        }
        entree.tampon = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, entree.tampon);
        gl.bufferData(gl.ARRAY_BUFFER, sommets, gl.STATIC_DRAW);
      }
      return entree;
    },

    render(parametres) {
      if (this.enPanne || !this.programme) return;
      const debut = performance.now();
      const gl = parametres.context;
      const etat = parametres.state;
      const tuiles = this.tiles || [];

      // Libérer les tampons des tuiles qui ne sont plus affichées ou dont les données ont changé.
      const presentes = new Set();
      for (let i = 0; i < tuiles.length; i++) presentes.add(tuiles[i].id);
      for (const [id, entree] of this.tampons) {
        if (!presentes.has(id) || entree.version !== partage.version) {
          if (entree.tampon) gl.deleteBuffer(entree.tampon);
          this.tampons.delete(id);
        }
      }

      const index = partage.index;
      if (index && index.nbSegments > 0) {
        let construites = 0;
        let dureeConstruction = 0;
        const rot = (etat.rotation * Math.PI) / 180;
        const c = Math.cos(rot);
        const s = Math.sin(rot);

        gl.useProgram(this.programme);
        gl.uniform1f(this.emplacements.resolution, etat.resolution);
        gl.uniformMatrix2fv(this.emplacements.rotation, false, [c, -s, s, c]);
        gl.uniform2f(this.emplacements.versNdc, 2 / etat.size[0], 2 / etat.size[1]);
        gl.uniform4fv(this.emplacements.couleur, COULEUR);
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
        gl.enableVertexAttribArray(0);

        for (let i = 0; i < tuiles.length; i++) {
          const tuile = tuiles[i];
          let entree = this.tampons.get(tuile.id);
          if (!entree) {
            const t0 = performance.now();
            entree = this.construireTuile(gl, tuile);
            dureeConstruction += performance.now() - t0;
            this.tampons.set(tuile.id, entree);
            construites++;
          }
          if (!entree.tampon) continue;
          gl.uniform2f(this.emplacements.decalage, entree.ox - etat.center[0], entree.oy - etat.center[1]);
          gl.bindBuffer(gl.ARRAY_BUFFER, entree.tampon);
          gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
          gl.drawArrays(gl.LINES, 0, entree.nbSommets);
        }
        if (construites > 0) suivi.construction?.(construites, dureeConstruction);

        gl.disableVertexAttribArray(0);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);
        gl.disable(gl.BLEND);
        gl.useProgram(null);
      }
      suivi.rendu?.(performance.now() - debut, tuiles.length);
    },
  });

  const CoucheLignes = Layer.createSubclass({
    tileInfo: TileInfo.create({ size: TAILLE_TUILE, spatialReference: { wkid: 3857 } }),
    createLayerView(view) {
      if (view.type === "2d") {
        return new VueLignes({ view, layer: this });
      }
    },
  });

  const couche = new CoucheLignes({ title: "Plan (essai WebGL)" });

  /** Remplace les données de la couche ; renvoie l'index construit. */
  function definir(polylignes) {
    partage.index = indexerSegments(polylignes);
    partage.version++;
    for (const vue of partage.vues) vue.requestRender();
    return partage.index;
  }

  return { couche, definir };
}
