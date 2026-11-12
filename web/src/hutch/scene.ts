// The 3D hutch: the plan's cages in two (or more) tiers, see-through wire walls, nest boxes, and the
// rabbits from a typical day hopping about. Under each rabbit a dashed outline shows the floor the
// WA Code of Practice says it needs. Loaded on demand (three.js is big), driven by `Hutch.tsx`.
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { CSS2DObject, CSS2DRenderer } from "three/addons/renderers/CSS2DRenderer.js";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";
import { clone as cloneSkinned } from "three/addons/utils/SkeletonUtils.js";
import rabbitUrl from "../assets/rabbit.glb?url";

export type CageKind = "doe" | "buck" | "grow";
export type Role = "empty" | "doe" | "doe_litter" | "buck" | "replacement" | "growers";

export interface HutchCage {
  kind: CageKind;
  tier: number;
  /** 0 = front row (doors at the front), 1 = back row. */
  row: number;
  x: number;
  w: number;
  area: number;
  role: Role;
  count: number;
  /** Growers 12 weeks and older (need 0.18 m² each); the rest need 0.07 m². */
  older: number;
  label: string;
}

export interface HutchData {
  width: number;
  rows: number;
  rowDepth: number;
  tiers: number;
  /** Cage interior height and the floor + tray + gap under each tier, metres. */
  cageH: number;
  overhead: number;
  /** Nest box floor area in each doe cage (m²); 0 for none. */
  nestArea: number;
  /** The Code's area for a doe and her litter (0.56 or 0.74 m², by weaning age). */
  doeArea: number;
  cages: HutchCage[];
}

export interface Hutch {
  update(d: HutchData): void;
  setLift(on: boolean): void;
  frontOn(): void;
  dispose(): void;
}

/** WA Code of Practice, Section 3, m². */
const CODE = { young: 0.07, older: 0.18, breeder: 0.56 };
const LIFT = 0.32;
const COLOURS = {
  doe: { floor: "#fbe3ec", line: "#c2456f" },
  buck: { floor: "#dcebfb", line: "#2f6fbf" },
  grow: { floor: "#f3eadc", line: "#8a6a43" },
};
/** Body length of each kind of rabbit, metres (an NZ White doe is about 45 cm). */
const LENGTH = { doe: 0.45, buck: 0.48, older: 0.38, young: 0.29, kit: 0.15 };

type BunnyState = "idle" | "turn" | "hop" | "look" | "sit";

interface Bunny {
  obj: THREE.Object3D;
  mixer: THREE.AnimationMixer;
  acts: Record<"Hop" | "Idle" | "Look" | "SitUp", THREE.AnimationAction>;
  current: THREE.AnimationAction;
  len: number;
  kit: boolean;
  /** Where it is now / hopping from / to (cage-floor coordinates). */
  x: number;
  z: number;
  fx: number;
  fz: number;
  tx: number;
  tz: number;
  heading: number;
  fromHeading: number;
  toHeading: number;
  state: BunnyState;
  /** Seconds into the current state, and how long it lasts. */
  t: number;
  dur: number;
  outline: THREE.Object3D | null;
  shadow: THREE.Mesh;
  next: () => [number, number] | null;
}

/** The rabbit's hop clip leaves the ground at about 20% and lands at about 80% of its length. */
const HOP_LIFT = [0.2, 0.8] as const;

function roundedRect(w: number, d: number, r: number): THREE.Vector3[] {
  const pts: THREE.Vector3[] = [];
  const rr = Math.min(r, w / 2, d / 2);
  const corners: [number, number, number][] = [
    [w / 2 - rr, d / 2 - rr, 0],
    [-w / 2 + rr, d / 2 - rr, Math.PI / 2],
    [-w / 2 + rr, -d / 2 + rr, Math.PI],
    [w / 2 - rr, -d / 2 + rr, (3 * Math.PI) / 2],
  ];
  for (const [cx, cz, a0] of corners) for (let i = 0; i <= 6; i++) {
    const a = a0 + (i / 6) * (Math.PI / 2);
    pts.push(new THREE.Vector3(cx + Math.cos(a) * rr, 0, cz + Math.sin(a) * rr));
  }
  return pts;
}

/** A dashed outline enclosing `area` m², shaped like `aspect` (width / depth). */
function outline(area: number, aspect: number, colour: string): THREE.LineLoop {
  const d = Math.sqrt(area / aspect);
  const w = area / d;
  const g = new THREE.BufferGeometry().setFromPoints(roundedRect(w, d, Math.min(w, d) * 0.18));
  const line = new THREE.LineLoop(g, new THREE.LineDashedMaterial({ color: colour, dashSize: 0.025, gapSize: 0.018, transparent: true, opacity: 0.95 }));
  line.computeLineDistances();
  line.renderOrder = 2;
  return line;
}

/**
 * A dashed outline that follows a cage floor (minus the nest box, which sits in a corner), inset evenly
 * until it encloses exactly `area` m²: the Code minimum shown inside the cage it applies to.
 */
function floorOutline(
  x0: number,
  x1: number,
  z0: number,
  z1: number,
  nest: { x0: number; z0: number; z1: number; atBack: boolean } | null,
  area: number,
  colour: string,
): THREE.LineLoop {
  const shape = (m: number) => {
    const ax0 = x0 + m, ax1 = x1 - m, az0 = z0 + m, az1 = z1 - m;
    if (!nest) return { pts: [[ax0, az0], [ax1, az0], [ax1, az1], [ax0, az1]], a: (ax1 - ax0) * (az1 - az0) };
    const cx = nest.x0 - m;
    if (nest.atBack) {
      // nest box in the (x1, z0) corner
      const cz = nest.z1 + m;
      return { pts: [[ax0, az0], [cx, az0], [cx, cz], [ax1, cz], [ax1, az1], [ax0, az1]], a: (ax1 - ax0) * (az1 - az0) - (ax1 - cx) * (cz - az0) };
    }
    const cz = nest.z0 - m;
    return { pts: [[ax0, az0], [ax1, az0], [ax1, cz], [cx, cz], [cx, az1], [ax0, az1]], a: (ax1 - ax0) * (az1 - az0) - (ax1 - cx) * (az1 - cz) };
  };
  let lo = 0.01, hi = Math.min(x1 - x0, z1 - z0) / 2 - 0.01;
  if (shape(lo).a > area) for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    if (shape(mid).a > area) lo = mid;
    else hi = mid;
  }
  const g = new THREE.BufferGeometry().setFromPoints(shape(lo).pts.map(([x, z]) => new THREE.Vector3(x, 0, z)));
  const line = new THREE.LineLoop(g, new THREE.LineDashedMaterial({ color: colour, dashSize: 0.03, gapSize: 0.02 }));
  line.computeLineDistances();
  line.renderOrder = 2;
  return line;
}

function meshTexture(): THREE.Texture {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d")!;
  g.strokeStyle = "rgba(80,95,115,0.95)";
  g.lineWidth = 1.6;
  g.strokeRect(0, 0, 64, 64);
  const t = new THREE.CanvasTexture(c);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 4;
  return t;
}

const rand = (a: number, b: number) => a + Math.random() * (b - a);
const ease = (t: number) => t * t * (3 - 2 * t);
const angleTo = (from: number, to: number) => from + Math.atan2(Math.sin(to - from), Math.cos(to - from));

/**
 * The coat: thousands of short, soft strands painted on a light ground. Used as the colour map (on top
 * of the model's baked shading) and as a fine bump, so the fur reads up close without noise.
 */
function furTexture(): THREE.CanvasTexture {
  const n = 512;
  const c = document.createElement("canvas");
  c.width = c.height = n;
  const g = c.getContext("2d")!;
  g.fillStyle = "#f4f2ef";
  g.fillRect(0, 0, n, n);
  g.lineCap = "round";
  for (let i = 0; i < 14000; i++) {
    const x = Math.random() * n, y = Math.random() * n;
    const a = Math.random() * Math.PI * 2;
    const len = 5 + Math.random() * 10;
    const v = 200 + Math.floor(Math.random() * 55);
    g.strokeStyle = `rgba(${v},${v - 3},${v - 7},0.45)`;
    g.lineWidth = 0.8 + Math.random() * 0.8;
    g.beginPath();
    g.moveTo(x, y);
    // a slight curl
    g.quadraticCurveTo(x + Math.cos(a + 0.4) * len * 0.5, y + Math.sin(a + 0.4) * len * 0.5, x + Math.cos(a) * len, y + Math.sin(a) * len);
    g.stroke();
  }
  const t = new THREE.CanvasTexture(c);
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.repeat.set(5, 5);
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 8;
  return t;
}

/** A soft dark disc, for contact shadows under the rabbits. */
function blobTexture(): THREE.Texture {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d")!;
  const r = g.createRadialGradient(32, 32, 0, 32, 32, 32);
  r.addColorStop(0, "rgba(0,0,0,0.55)");
  r.addColorStop(0.6, "rgba(0,0,0,0.22)");
  r.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = r;
  g.fillRect(0, 0, 64, 64);
  return new THREE.CanvasTexture(c);
}

export async function createHutch(el: HTMLElement, first: HutchData): Promise<Hutch> {
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFShadowMap;
  el.appendChild(renderer.domElement);
  const labels = new CSS2DRenderer();
  labels.domElement.className = "hutch-labels";
  el.appendChild(labels.domElement);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(30, 1, 0.05, 50);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.enablePan = false;
  controls.minPolarAngle = 0.25;
  controls.maxPolarAngle = 1.35;
  controls.rotateSpeed = 0.7;

  // a soft studio: an environment for gentle fill and reflections (the eyes catch it), a warm key that
  // casts shadows, and a cool rim to pick out the fur
  const pmrem = new THREE.PMREMGenerator(renderer);
  const envTex = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environment = envTex;
  scene.environmentIntensity = 0.55;
  scene.add(new THREE.HemisphereLight("#eef4ff", "#cdbfae", 0.5));
  const key = new THREE.DirectionalLight("#fff3e6", 2.6);
  key.position.set(2.2, 4, 3);
  key.castShadow = true;
  key.shadow.mapSize.set(2048, 2048);
  key.shadow.camera.left = key.shadow.camera.bottom = -2.5;
  key.shadow.camera.right = key.shadow.camera.top = 2.5;
  key.shadow.radius = 5;
  key.shadow.bias = -0.0004;
  key.shadow.normalBias = 0.01;
  scene.add(key);
  const rim = new THREE.DirectionalLight("#dfe8ff", 1.1);
  rim.position.set(-3, 2.2, -2.5);
  scene.add(rim);
  // the shed floor the hutch stands on, fading out at the edges
  const fade = document.createElement("canvas");
  fade.width = fade.height = 128;
  const fg = fade.getContext("2d")!;
  const grad = fg.createRadialGradient(64, 64, 10, 64, 64, 64);
  grad.addColorStop(0, "#fff");
  grad.addColorStop(0.55, "#fff");
  grad.addColorStop(1, "#000");
  fg.fillStyle = grad;
  fg.fillRect(0, 0, 128, 128);
  const fadeTex = new THREE.CanvasTexture(fade);
  const ground = new THREE.Mesh(
    new THREE.CircleGeometry(3.2, 64),
    new THREE.MeshStandardMaterial({ color: "#d3c9bb", roughness: 0.95, transparent: true, alphaMap: fadeTex, depthWrite: false }),
  );
  ground.rotation.x = -Math.PI / 2;
  ground.receiveShadow = true;
  scene.add(ground);

  const gltf = await new GLTFLoader().loadAsync(rabbitUrl);
  const model = gltf.scene;
  const furTex = furTexture();
  let bodyMesh: THREE.SkinnedMesh | null = null;
  model.traverse((o) => {
    const m = o as THREE.Mesh;
    if (!m.isMesh) return;
    m.castShadow = true;
    m.frustumCulled = false;
    const mat = m.material as THREE.MeshStandardMaterial;
    if (mat.name === "Fur") {
      // the coat: baked shading (vertex colours) x painted strands, a fine bump, a velvety sheen, and a
      // soft bright rim where the fur turns away from the eye
      const coat = new THREE.MeshPhysicalMaterial({
        vertexColors: true,
        map: furTex,
        bumpMap: furTex,
        bumpScale: 1.2,
        roughness: 0.92,
        sheen: 1,
        sheenRoughness: 0.5,
        sheenColor: new THREE.Color("#fff8f0"),
      });
      coat.onBeforeCompile = (sh) => {
        sh.fragmentShader = sh.fragmentShader.replace(
          "#include <opaque_fragment>",
          "outgoingLight += vec3(0.11, 0.105, 0.1) * pow(1.0 - saturate(dot(normalize(normal), normalize(vViewPosition))), 2.6);\n#include <opaque_fragment>",
        );
      };
      coat.name = "Fur";
      m.material = coat;
      if ((m as THREE.SkinnedMesh).isSkinnedMesh) bodyMesh = m as THREE.SkinnedMesh;
    } else if (mat.name === "Eye") {
      // wet, glassy eyes that catch the light
      m.material = new THREE.MeshPhysicalMaterial({ color: mat.color, roughness: 0.04, clearcoat: 1, clearcoatRoughness: 0.02, envMapIntensity: 1.6 });
      m.material.name = mat.name;
    }
  });
  if (!bodyMesh) throw new Error("rabbit model has no skinned body");
  const clips = Object.fromEntries(gltf.animations.map((a) => [a.name, a])) as Record<string, THREE.AnimationClip>;
  const blobTex = blobTexture();
  const meshTex = meshTexture();
  const disposables: { dispose(): void }[] = [meshTex, furTex, blobTex, envTex, pmrem, fadeTex];
  const shared = new Set<unknown>();
  model.traverse((o) => {
    const m = o as THREE.Mesh;
    if (m.isMesh) {
      shared.add(m.geometry);
      (Array.isArray(m.material) ? m.material : [m.material]).forEach((x) => shared.add(x));
    }
  });
  /** Free everything under `root` that this module made (not the shared rabbit model). */
  function disposeTree(root: THREE.Object3D) {
    root.traverse((o) => {
      const m = o as THREE.Mesh;
      if (m.geometry && !shared.has(m.geometry)) m.geometry.dispose();
      if (m.material) {
        for (const x of Array.isArray(m.material) ? m.material : [m.material]) {
          if (shared.has(x)) continue;
          const map = (x as THREE.MeshBasicMaterial).map;
          if (map && map !== meshTex) map.dispose();
          x.dispose();
        }
      }
      if (o instanceof CSS2DObject) o.element.remove();
    });
  }
  (bodyMesh as THREE.SkinnedMesh).geometry.computeBoundingBox();
  const bb = (bodyMesh as THREE.SkinnedMesh).geometry.boundingBox!;
  const modelLen = Math.max(bb.max.x - bb.min.x, bb.max.z - bb.min.z);
  const blobGeo = new THREE.PlaneGeometry(1, 1);
  blobGeo.rotateX(-Math.PI / 2);
  const blobMat = new THREE.MeshBasicMaterial({ map: blobTex, transparent: true, depthWrite: false });
  shared.add(blobGeo);
  shared.add(blobMat);
  disposables.push(blobGeo, blobMat);

  let hutch = new THREE.Group();
  scene.add(hutch);
  let bunnies: Bunny[] = [];
  let lift = false;
  let data = first;
  let tierGroups: THREE.Group[] = [];

  const tierY = (t: number) => t * (data.cageH + data.overhead) + data.overhead + (lift && t > 0 ? LIFT * t : 0);

  function build(d: HutchData) {
    // clear the last build (the rabbits share the model's geometry and materials: keep those)
    scene.remove(hutch);
    disposeTree(hutch);
    hutch = new THREE.Group();
    scene.add(hutch);
    bunnies = [];
    tierGroups = [];
    data = d;
    const W = d.width;
    const rd = d.rowDepth;
    const z0 = (row: number) => (d.rows === 2 ? (row === 0 ? 0 : -rd) : -rd / 2);
    hutch.position.set(-W / 2, 0, 0);

    const slab = new THREE.MeshStandardMaterial({ color: "#dfe5ec", roughness: 0.8 });
    const post = new THREE.MeshStandardMaterial({ color: "#b98b5e", roughness: 0.7 });
    const wire = new THREE.MeshBasicMaterial({ map: meshTex, color: "#7a8a9e", transparent: true, opacity: 0.32, depthWrite: false, side: THREE.DoubleSide });
    const edge = new THREE.LineBasicMaterial({ color: "#7d8ea4" });
    const wood = [
      new THREE.MeshStandardMaterial({ color: "#c99662", roughness: 0.75 }),
      new THREE.MeshStandardMaterial({ color: "#b07f4f", roughness: 0.75 }),
      new THREE.MeshStandardMaterial({ color: "#e0b788", roughness: 0.75 }),
    ];
    const hole = new THREE.MeshBasicMaterial({ color: "#3b2716" });
    const depthAll = d.rows * rd;

    for (let t = 0; t < d.tiers; t++) {
      const g = new THREE.Group();
      g.position.y = tierY(t);
      hutch.add(g);
      tierGroups.push(g);
      // tray and floor frame under the tier
      const tray = new THREE.Mesh(new THREE.BoxGeometry(W + 0.04, 0.05, depthAll + 0.04), slab);
      tray.position.set(W / 2, -0.03, d.rows === 2 ? 0 : 0);
      tray.receiveShadow = true;
      tray.castShadow = true;
      g.add(tray);
      // corner posts
      for (const [px, pz] of [[0, -depthAll / 2], [W, -depthAll / 2], [0, depthAll / 2], [W, depthAll / 2]]) {
        const ph = d.cageH + d.overhead;
        const p = new THREE.Mesh(new THREE.BoxGeometry(0.04, ph, 0.04), post);
        p.position.set(px, d.cageH - ph / 2, pz);
        p.castShadow = true;
        g.add(p);
      }
    }

    d.cages.forEach((c) => {
      const g = tierGroups[c.tier];
      if (!g) return;
      const zMin = z0(c.row);
      const cx = c.x + c.w / 2;
      const cz = zMin + rd / 2;
      const col = COLOURS[c.kind];
      // floor
      const floor = new THREE.Mesh(new THREE.PlaneGeometry(c.w - 0.01, rd - 0.01), new THREE.MeshStandardMaterial({ color: col.floor, roughness: 0.95 }));
      floor.rotation.x = -Math.PI / 2;
      floor.position.set(cx, 0.002, cz);
      floor.receiveShadow = true;
      g.add(floor);
      // wire walls (no roof, so you can see in)
      const walls: [number, number, number, number, number][] = [
        [c.x, cz, rd, Math.PI / 2, 0],
        [c.x + c.w, cz, rd, Math.PI / 2, 0],
        [cx, zMin, c.w, 0, 0],
        [cx, zMin + rd, c.w, 0, 0],
      ];
      for (const [wx, wz, len, ry] of walls) {
        const geo = new THREE.PlaneGeometry(len, d.cageH);
        const m = wire.clone();
        m.map = meshTex.clone();
        m.map.repeat.set(len / 0.06, d.cageH / 0.06);
        m.map.needsUpdate = true;
        const wall = new THREE.Mesh(geo, m);
        wall.position.set(wx, d.cageH / 2, wz);
        wall.rotation.y = ry;
        wall.renderOrder = 1;
        g.add(wall);
      }
      const frame = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(c.w, d.cageH, rd)), edge);
      frame.position.set(cx, d.cageH / 2, cz);
      g.add(frame);

      // the nest box: far corner from the door
      const back = c.row === 0 ? zMin : zMin + rd; // wall away from the door
      let nest: { x0: number; x1: number; z0: number; z1: number } | null = null;
      if (c.kind === "doe" && d.nestArea > 0) {
        const nw = Math.sqrt((d.nestArea * 4) / 3);
        const nd = d.nestArea / nw;
        const nh = 0.28;
        const nx = c.x + c.w - nw / 2 - 0.02;
        const nz = c.row === 0 ? back + nd / 2 + 0.02 : back - nd / 2 - 0.02;
        const nb = new THREE.Mesh(new THREE.BoxGeometry(nw, nh, nd), [wood[0], wood[0], wood[2], wood[1], wood[1], wood[1]]);
        nb.position.set(nx, nh / 2, nz);
        nb.castShadow = nb.receiveShadow = true;
        g.add(nb);
        // the doorway faces into the cage, towards the door side
        const door = new THREE.Mesh(new THREE.CircleGeometry(0.06, 24), hole);
        door.position.set(nx - nw * 0.15, 0.1, nz + (c.row === 0 ? nd / 2 + 0.001 : -nd / 2 - 0.001));
        if (c.row !== 0) door.rotation.y = Math.PI;
        g.add(door);
        nest = { x0: nx - nw / 2, x1: nx + nw / 2, z0: nz - nd / 2, z1: nz + nd / 2 };
      }

      // label above the cage
      const chip = document.createElement("div");
      chip.className = `cage-chip ${c.kind}`;
      chip.textContent = c.label;
      const lab = new CSS2DObject(chip);
      // at the cage's own floor, on its door side, so it can't be mistaken for the tier above
      lab.position.set(cx, 0.03, c.row === 0 ? zMin + rd : zMin);
      g.add(lab);

      // ---- who lives here ----
      const m = 0.05;
      const bx0 = c.x + m, bx1 = c.x + c.w - m, bz0 = zMin + m, bz1 = zMin + rd - m;
      const clear = (x: number, z: number, r: number) => !nest || x + r < nest.x0 - 0.02 || z + r < nest.z0 - 0.02 || z - r > nest.z1 + 0.02;
      const spotIn = (r: number): [number, number] => {
        for (let i = 0; i < 40; i++) {
          const x = rand(bx0 + r, bx1 - r), z = rand(bz0 + r, bz1 - r);
          if (clear(x, z, r)) return [x, z];
        }
        return [cx, cz];
      };
      const add = (len: number, at: [number, number], next: Bunny["next"], area: number, aspect: number) => {
        const obj = cloneSkinned(model);
        obj.scale.setScalar(len / modelLen);
        const mixer = new THREE.AnimationMixer(obj);
        const acts = {
          Hop: mixer.clipAction(clips.Hop),
          Idle: mixer.clipAction(clips.Idle),
          Look: mixer.clipAction(clips.Look),
          SitUp: mixer.clipAction(clips.SitUp),
        };
        for (const k of ["Hop", "Look", "SitUp"] as const) {
          acts[k].setLoop(THREE.LoopOnce, 1);
          acts[k].clampWhenFinished = true;
        }
        const kit = len < 0.2;
        acts.Idle.timeScale = rand(0.8, 1.2);
        acts.Idle.time = rand(0, clips.Idle.duration);
        acts.Idle.play();
        const shadow = new THREE.Mesh(blobGeo, blobMat);
        shadow.scale.set(len * 1.1, 1, len * 0.7);
        shadow.renderOrder = 1;
        g.add(shadow);
        const heading = rand(0, Math.PI * 2);
        const b: Bunny = {
          obj, mixer, acts, current: acts.Idle, len, kit,
          x: at[0], z: at[1], fx: at[0], fz: at[1], tx: at[0], tz: at[1],
          heading, fromHeading: heading, toHeading: heading,
          state: "idle", t: 0, dur: rand(0.5, 4), outline: null, shadow, next,
        };
        if (area > 0) {
          b.outline = outline(area, aspect, col.line);
          b.outline.position.y = 0.006;
          g.add(b.outline);
        }
        obj.position.set(b.x, 0, b.z);
        obj.rotation.y = b.heading;
        g.add(obj);
        bunnies.push(b);
        return b;
      };

      if (c.role === "doe" || c.role === "doe_litter") {
        // the doe and her litter share the Code area for a doe + litter: one outline, following the floor
        // around the nest box
        const shared = floorOutline(c.x, c.x + c.w, zMin, zMin + rd, nest && { x0: nest.x0, z0: nest.z0, z1: nest.z1, atBack: c.row === 0 }, d.doeArea, col.line);
        shared.position.y = 0.006;
        g.add(shared);
        add(LENGTH.doe, spotIn(0.12), () => spotIn(0.12), 0, 1);
        const kits = c.role === "doe_litter" ? c.count - 1 : 0;
        for (let k = 0; k < kits; k++) {
          // kits huddle by the nest box doorway
          const hx = nest ? nest.x0 - 0.06 : cx;
          const hz = nest ? (nest.z0 + nest.z1) / 2 : cz;
          const near = (): [number, number] => {
            const a = rand(0, Math.PI * 2), r = rand(0.02, 0.18);
            const x = Math.min(bx1, Math.max(bx0, hx - Math.abs(Math.cos(a)) * r)), z = Math.min(bz1, Math.max(bz0, hz + Math.sin(a) * r * 1.4));
            return clear(x, z, 0.03) ? [x, z] : [hx - 0.08, hz];
          };
          add(LENGTH.kit, near(), near, 0, 1);
        }
      } else if (c.role === "buck") {
        const shared = floorOutline(c.x, c.x + c.w, zMin, zMin + rd, null, CODE.breeder, col.line);
        shared.position.y = 0.006;
        g.add(shared);
        add(LENGTH.buck, spotIn(0.12), () => spotIn(0.12), 0, 1);
      } else if (c.role === "replacement" || c.role === "growers") {
        // each grower owns a floor cell at least its Code area. An even grid when everyone fits with
        // room to spare (then they hop between free cells); otherwise cells sized to each rabbit's area,
        // which always fit because the plan meets the Code (then they hop on the spot).
        const n = c.role === "replacement" ? 1 : c.count;
        const olderN = c.role === "replacement" ? 1 : c.older;
        const areas = Array.from({ length: n }, (_, k) => (k < olderN ? CODE.older : CODE.young));
        const fw = c.w, fd = rd;
        let grid: [number, number] | null = null;
        let bestScore = -Infinity;
        for (let cols = 1; cols <= 8; cols++) for (let rows = 1; rows <= 6; rows++) {
          const cells = cols * rows;
          if (cells <= n || (fw / cols) * (fd / rows) + 1e-9 < Math.max(...areas)) continue;
          const score = -Math.abs(Math.log(fw / cols / (fd / rows))) * 4 - Math.abs(cells - n - 2);
          if (score > bestScore) [bestScore, grid] = [score, [cols, rows]];
        }
        type Cell = { x: number; z: number; w: number; d: number; used: boolean };
        const cells: Cell[] = [];
        if (grid) {
          const [cols, rows] = grid;
          const cw = fw / cols, cd = fd / rows;
          for (let i = 0; i < cols; i++) for (let j = 0; j < rows; j++) cells.push({ x: c.x + cw * (i + 0.5), z: zMin + cd * (j + 0.5), w: cw, d: cd, used: false });
          cells.sort(() => Math.random() - 0.5);
        } else {
          // treemap: one or two strips, each split across in proportion to the rabbits' areas
          const total = areas.reduce((x, y) => x + y, 0);
          const strips = n > 3 ? 2 : 1;
          const rowsOf: number[][] = Array.from({ length: strips }, () => []);
          const sum = (r: number[]) => r.reduce((x, k) => x + areas[k], 0);
          areas.map((_, k) => k).forEach((k) => rowsOf.sort((p, q) => sum(p) - sum(q))[0].push(k));
          let z = zMin;
          const order: Cell[] = [];
          for (const r of rowsOf) {
            const rdep = (fd * sum(r)) / total;
            let x = c.x;
            for (const k of r) {
              const w = (fw * areas[k]) / sum(r);
              order[k] = { x: x + w / 2, z: z + rdep / 2, w, d: rdep, used: false };
              x += w;
            }
            z += rdep;
          }
          cells.push(...order);
        }
        for (let k = 0; k < n; k++) {
          const cell = grid ? cells.find((q) => !q.used)! : cells[k];
          cell.used = true;
          const isOld = k < olderN;
          let mine = cell;
          const next = (): [number, number] | null => {
            if (!grid) return null;
            const free = cells.filter((q) => !q.used);
            if (free.length === 0) return null;
            free.sort((p, q) => Math.hypot(p.x - mine.x, p.z - mine.z) - Math.hypot(q.x - mine.x, q.z - mine.z));
            const to = free[Math.floor(Math.random() * Math.min(2, free.length))];
            mine.used = false;
            to.used = true;
            mine = to;
            return [to.x, to.z];
          };
          // the outline takes its cell's shape, a little inside it
          add(isOld ? LENGTH.older : LENGTH.young, [cell.x, cell.z], next, areas[k], cell.w / cell.d);
        }
      }
    });
    for (const b of bunnies) {
      if (b.outline) b.outline.position.set(b.x, 0.006, b.z);
      b.shadow.position.set(b.x, 0.004, b.z);
    }
  }

  function frame(h: number, dist: number) {
    camera.position.set(Math.sin(h) * dist, dist * 0.42 + 0.2, Math.cos(h) * dist);
    controls.target.set(0, (data.tiers * (data.cageH + data.overhead) + (lift ? LIFT : 0)) * 0.48, 0);
    controls.update();
  }
  // further back on narrow screens so the whole hutch fits across
  const distFor = () => (Math.max(data.width, data.rows * data.rowDepth) * 1.25 + 0.75) / Math.min(1, Math.max(camera.aspect, 0.55) / 1.3);

  function resize() {
    const w = el.clientWidth, h = el.clientHeight;
    renderer.setSize(w, h, false);
    labels.setSize(w, h);
    camera.aspect = w / Math.max(h, 1);
    camera.updateProjectionMatrix();
  }
  const ro = new ResizeObserver(resize);
  ro.observe(el);

  build(first);
  resize();
  frame(-0.55, distFor());
  controls.minDistance = distFor() * 0.45;
  controls.maxDistance = distFor() * 1.6;

  // ---- animation ----
  const clock = new THREE.Timer();
  let raf = 0;
  let visible = true;
  const io = new IntersectionObserver((e) => (visible = e[0]?.isIntersecting ?? true));
  io.observe(el);
  function tick() {
    raf = requestAnimationFrame(tick);
    clock.update();
    const dt = Math.min(clock.getDelta(), 0.05);
    if (!visible || document.hidden) return;
    controls.update();
    for (const b of bunnies) {
      b.mixer.update(reduced ? 0 : dt);
      if (reduced) continue;
      b.t += dt;
      const play = (a: THREE.AnimationAction, fade: number) => {
        a.reset().play();
        b.current.crossFadeTo(a, fade, false);
        b.current = a;
      };
      if (b.state === "idle" && b.t >= b.dur) {
        const roll = Math.random();
        const to = roll < (b.kit ? 0.8 : 0.62) ? b.next() : null;
        if (to || roll < 0.5) {
          // hop: to the next spot, or a turn on the spot when there's nowhere free
          b.fx = b.x;
          b.fz = b.z;
          [b.tx, b.tz] = to ?? [b.x, b.z];
          const dx = b.tx - b.x, dz = b.tz - b.z;
          b.fromHeading = b.heading;
          b.toHeading = Math.hypot(dx, dz) > 0.01 ? angleTo(b.heading, Math.atan2(-dz, dx)) : b.heading + rand(-1.6, 1.6);
          b.state = "turn";
          b.t = 0;
          b.dur = 0.22;
        } else if (roll < 0.85 || b.kit) {
          b.acts.Look.timeScale = rand(0.8, 1.1);
          play(b.acts.Look, 0.25);
          b.state = "look";
          b.t = 0;
          b.dur = clips.Look.duration / b.acts.Look.timeScale;
        } else {
          play(b.acts.SitUp, 0.25);
          b.state = "sit";
          b.t = 0;
          b.dur = clips.SitUp.duration + rand(0, 0.6);
        }
      } else if (b.state === "turn") {
        b.heading = b.fromHeading + (b.toHeading - b.fromHeading) * ease(Math.min(1, b.t / b.dur));
        if (b.t >= b.dur) {
          b.acts.Hop.timeScale = b.kit ? 1.5 : rand(1.0, 1.2);
          play(b.acts.Hop, 0.08);
          b.state = "hop";
          b.t = 0;
          b.dur = clips.Hop.duration / b.acts.Hop.timeScale;
        }
      } else if (b.state === "hop") {
        // the body travels while the feet are off the ground
        const ph = Math.min(1, b.t / b.dur);
        const k = ease(Math.min(1, Math.max(0, (ph - HOP_LIFT[0]) / (HOP_LIFT[1] - HOP_LIFT[0]))));
        b.x = b.fx + (b.tx - b.fx) * k;
        b.z = b.fz + (b.tz - b.fz) * k;
        const air = ph > HOP_LIFT[0] && ph < HOP_LIFT[1] ? Math.sin(((ph - HOP_LIFT[0]) / (HOP_LIFT[1] - HOP_LIFT[0])) * Math.PI) : 0;
        b.shadow.scale.set(b.len * 1.1 * (1 - 0.35 * air), 1, b.len * 0.7 * (1 - 0.35 * air));
        if (b.t >= b.dur) {
          play(b.acts.Idle, 0.2);
          b.state = "idle";
          b.t = 0;
          b.dur = b.kit ? rand(0.4, 2.5) : rand(1.2, 5);
        }
      } else if ((b.state === "look" || b.state === "sit") && b.t >= b.dur) {
        play(b.acts.Idle, 0.3);
        b.state = "idle";
        b.t = 0;
        b.dur = rand(0.8, 3.5);
      }
      b.obj.position.set(b.x, 0, b.z);
      b.obj.rotation.y = b.heading;
      b.shadow.position.set(b.x, 0.004, b.z);
      if (b.outline) b.outline.position.set(b.x, 0.006, b.z);
    }
    renderer.render(scene, camera);
    labels.render(scene, camera);
  }
  tick();

  return {
    update(d) {
      build(d);
      controls.minDistance = distFor() * 0.45;
      controls.maxDistance = distFor() * 1.6;
    },
    setLift(on) {
      lift = on;
      tierGroups.forEach((g, t) => (g.position.y = tierY(t)));
      frame(Math.atan2(camera.position.x, camera.position.z), camera.position.distanceTo(controls.target));
    },
    frontOn() {
      frame(0, distFor());
    },
    dispose() {
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
      controls.dispose();
      disposeTree(hutch);
      ground.geometry.dispose();
      (ground.material as THREE.Material).dispose();
      model.traverse((o) => {
        const m = o as THREE.Mesh;
        if (m.isMesh) {
          m.geometry.dispose();
          (Array.isArray(m.material) ? m.material : [m.material]).forEach((x) => x.dispose());
        }
      });
      disposables.forEach((x) => x.dispose());
      renderer.dispose();
      renderer.domElement.remove();
      labels.domElement.remove();
    },
  };
}
