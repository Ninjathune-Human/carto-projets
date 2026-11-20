// CoucheScene : couche WebGL personnalisée (BaseLayerViewGL2D avec tuilage) qui dessine une scène
// SCN1 (traits et surfaces) sur la carte. Même structure que couche-lignes.js ; les tampons d'une
// tuile viennent de preparerTampons (essais/commun/rendu-scene.js), avec l'origine de la tuile pour
// origine : les coordonnées relatives sont calculées en double précision côté JS.
//
// Ordre d'affichage sans tampon de profondeur (la cible de rendu d'ArcGIS n'en promet pas) :
// - dans une tuile, surfaces et traits forment une seule liste de triangles rangée par rang de
//   primitive (entrelacerScene), dessinée en un appel : le mélange suit l'ordre de soumission ;
// - une primitive à cheval sur plusieurs tuiles est dans chacune, et chaque tuile ne peint que son
//   propre carré (fragments hors tuile écartés) : sans cela, la tuile voisine repeindrait la
//   primitive par-dessus celles qui la suivent ;
// - la recherche d'une tuile est élargie de la demi-largeur du trait le plus épais, pour qu'un trait
//   proche du bord soit peint des deux côtés.
// Les modules ArcGIS sont fournis par l'appelant ($arcgis.import), rien n'est importé d'ArcGIS ici.
// Shaders GLSL ES 1.00 : le contexte fourni peut être WebGL 1 ou 2.

import { indexerPrimitives, entrelacerScene, FOULEE_SCENE } from "./outils.js";
import { TAILLE_TUILE } from "./couche-lignes.js";
import { preparerTampons } from "../commun/rendu-scene.js";

const PX_PAR_MM = 96 / 25.4;
// Une tuile peut s'afficher à une résolution plus grossière que la sienne pendant un zoom : marge
// prévue pour deux fois sa résolution.
const ECART_RESOLUTION = 2;

// Sommets en mètres relatifs à l'origine de la tuile (coin haut gauche). Un trait est élargi en
// pixels écran : max(1, mm × 96 / 25.4) px, prolongé d'une demi-largeur au-delà de chaque extrémité
// (comme dans rendu-scene.js). v_tuile : position du sommet élargi dans la tuile, en fraction du côté.
const SOURCE_SOMMETS = `
precision highp float;
attribute vec2 a_position;
attribute vec2 a_autre;
attribute vec2 a_trait;      // côté (±1, 0 pour une surface), épaisseur en mm
attribute vec4 a_couleur;
uniform vec2 u_decalage;     // origine de la tuile - centre de la vue (m)
uniform float u_resolution;  // m par px CSS
uniform mat2 u_rotation;
uniform vec2 u_versNdc;      // 2 / taille en px CSS
uniform float u_cote;        // côté de la tuile (m)
varying vec4 v_couleur;
varying vec2 v_tuile;
const float PX_PAR_MM = ${PX_PAR_MM.toFixed(10)};
void main() {
  vec2 p = u_rotation * ((a_position + u_decalage) / u_resolution);
  vec2 decal = vec2(0.0);
  if (a_trait.x != 0.0) {
    vec2 d = u_rotation * ((a_autre - a_position) / u_resolution);
    float l = length(d);
    vec2 dir = l > 0.0 ? d / l : vec2(1.0, 0.0);
    float demi = 0.5 * max(1.0, a_trait.y * PX_PAR_MM);
    decal = vec2(-dir.y, dir.x) * (a_trait.x * demi) - dir * demi;
  }
  // Décalage écran ramené en mètres : rotation inverse (v * M = transposée de M fois v).
  vec2 m = a_position + (decal * u_rotation) * u_resolution;
  v_tuile = vec2(m.x, -m.y) / u_cote;
  v_couleur = a_couleur;
  gl_Position = vec4((p + decal) * u_versNdc, 0.0, 1.0);
}`;

// Carré de la tuile semi-ouvert : le bord droit et le bord bas appartiennent à la voisine.
const SOURCE_FRAGMENTS = `
precision highp float;
varying vec4 v_couleur;
varying vec2 v_tuile;
void main() {
  if (v_tuile.x < 0.0 || v_tuile.x >= 1.0 || v_tuile.y < 0.0 || v_tuile.y >= 1.0) discard;
  gl_FragColor = vec4(v_couleur.rgb * v_couleur.a, v_couleur.a); // prémultipliée
}`;

const ATTRIBUTS = ["a_position", "a_autre", "a_trait", "a_couleur"];
const UNIFORMES = ["u_decalage", "u_resolution", "u_rotation", "u_versNdc", "u_cote"];

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
  ATTRIBUTS.forEach((nom, i) => gl.bindAttribLocation(programme, i, nom));
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
 * Rend { couche, definirScene(scene) } ; definirScene remplace la scène (lireScene, repère
 * Web Mercator) et rend son index (indexerPrimitives).
 */
export function creerCoucheScene({ Layer, BaseLayerViewGL2D, TileInfo }, suivi = {}) {
  // margePx : demi-largeur du trait le plus épais, coin du prolongement compris, plus un pixel.
  const partage = { scene: null, index: null, margePx: 0, version: 0, vues: new Set() };

  const VueScene = BaseLayerViewGL2D.createSubclass({
    constructor: function () {
      this.tampons = new Map(); // id de tuile -> { tampon, nbSommets, ox, oy, cote, version }
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
      this.emplacements = {};
      for (const nom of UNIFORMES) this.emplacements[nom] = gl.getUniformLocation(this.programme, nom);
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
      const { scene, index } = partage;
      const ox = tuile.coords[0];
      const oy = tuile.coords[1];
      const cote = TAILLE_TUILE * tuile.resolution;
      const entree = { tampon: null, nbSommets: 0, ox, oy, cote, version: partage.version };
      const marge = partage.margePx * tuile.resolution * ECART_RESOLUTION;
      const trouves = index.chercher(ox - marge, oy - cote - marge, ox + cote + marge, oy + marge);
      if (trouves.length === 0) return entree;
      const sousScene = { calques: scene.calques, primitives: Array.from(trouves, k => scene.primitives[k]) };
      const { donnees, nbSommets } = entrelacerScene(preparerTampons(sousScene, [ox, oy]));
      if (nbSommets === 0) return entree;
      entree.nbSommets = nbSommets;
      entree.tampon = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, entree.tampon);
      gl.bufferData(gl.ARRAY_BUFFER, donnees, gl.STATIC_DRAW);
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

      if (partage.index && partage.index.nbPrimitives > 0) {
        let construites = 0;
        let dureeConstruction = 0;
        const rot = (etat.rotation * Math.PI) / 180;
        const c = Math.cos(rot);
        const s = Math.sin(rot);
        const u = this.emplacements;

        gl.useProgram(this.programme);
        gl.uniform1f(u.u_resolution, etat.resolution);
        gl.uniformMatrix2fv(u.u_rotation, false, [c, -s, s, c]);
        gl.uniform2f(u.u_versNdc, 2 / etat.size[0], 2 / etat.size[1]);
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
        for (let a = 0; a < ATTRIBUTS.length; a++) gl.enableVertexAttribArray(a);

        for (let i = 0; i < tuiles.length; i++) {
          const tuile = tuiles[i];
          let entree = this.tampons.get(tuile.id);
          if (!entree) {
            const t0 = performance.now();
            try {
              entree = this.construireTuile(gl, tuile);
            } catch (e) {
              // Tuile trop lourde ou données inattendues : signalée, laissée vide jusqu'au prochain projet.
              entree = { tampon: null, nbSommets: 0, version: partage.version };
              suivi.erreur?.(e);
            }
            dureeConstruction += performance.now() - t0;
            this.tampons.set(tuile.id, entree);
            construites++;
          }
          if (!entree.tampon) continue;
          gl.uniform2f(u.u_decalage, entree.ox - etat.center[0], entree.oy - etat.center[1]);
          gl.uniform1f(u.u_cote, entree.cote);
          gl.bindBuffer(gl.ARRAY_BUFFER, entree.tampon);
          gl.vertexAttribPointer(0, 2, gl.FLOAT, false, FOULEE_SCENE, 0);
          gl.vertexAttribPointer(1, 2, gl.FLOAT, false, FOULEE_SCENE, 8);
          gl.vertexAttribPointer(2, 2, gl.FLOAT, false, FOULEE_SCENE, 16);
          gl.vertexAttribPointer(3, 4, gl.UNSIGNED_BYTE, true, FOULEE_SCENE, 24);
          gl.drawArrays(gl.TRIANGLES, 0, entree.nbSommets);
        }
        if (construites > 0) suivi.construction?.(construites, dureeConstruction);

        // État par défaut rendu au SDK.
        for (let a = 0; a < ATTRIBUTS.length; a++) gl.disableVertexAttribArray(a);
        gl.bindBuffer(gl.ARRAY_BUFFER, null);
        gl.blendFunc(gl.ONE, gl.ZERO);
        gl.disable(gl.BLEND);
        gl.useProgram(null);
      }
      suivi.rendu?.(performance.now() - debut, tuiles.length);
    },
  });

  const CoucheScene = Layer.createSubclass({
    tileInfo: TileInfo.create({ size: TAILLE_TUILE, spatialReference: { wkid: 3857 } }),
    createLayerView(view) {
      if (view.type === "2d") {
        return new VueScene({ view, layer: this });
      }
    },
  });

  const couche = new CoucheScene({ title: "Projet (essai WebGL)" });

  /** Remplace la scène de la couche ; rend l'index construit. */
  function definirScene(scene) {
    let epaisseurMax = 0;
    for (const p of scene.primitives) {
      if (p.genre === "trait" && Number.isFinite(p.epaisseurMm) && p.epaisseurMm > epaisseurMax) epaisseurMax = p.epaisseurMm;
    }
    partage.scene = scene;
    partage.index = indexerPrimitives(scene);
    partage.margePx = 0.5 * Math.max(1, epaisseurMax * PX_PAR_MM) * Math.SQRT2 + 1;
    partage.version++;
    for (const vue of partage.vues) vue.requestRender();
    return partage.index;
  }

  return { couche, definirScene };
}
