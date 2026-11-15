// 3D explanations: a window of labelled objects the user can spin, built by the `scene3d` tool.
// Marks in the spoken answer ([[3d heart]]) turn the camera to one object and light it up.
//
// What it draws: plain shapes, tubes through points, profiles spun round, outlines pushed up,
// gears; parts that belong to other parts; one object repeated hundreds of times in a single
// draw call; soft light and shadows; spin, orbit, pulse and bob; dots flowing along arrows; an
// exploded view and a cut through the middle.
//
// What it costs: nothing while nothing changes. A frame is drawn only when the camera, the
// objects or the size changed; ambient motion runs at 30 frames a second, stops when the
// window is hidden, and the picture drops to a lower resolution (then loses its shadows) if
// this computer can't keep up while they drag.
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import { CSS2DObject, CSS2DRenderer } from "three/examples/jsm/renderers/CSS2DRenderer.js";
import { api, onEvent, type Scene3D as SceneData, type Scene3DObject } from "../lib/bridge";

const COLORS: Record<string, number> = {
  white: 0xf2f2f2,
  grey: 0x9a9a9a,
  blue: 0x5b8def,
  red: 0xe5554f,
  green: 0x4fbf7a,
  orange: 0xf09a3e,
  yellow: 0xf2d04b,
  pink: 0xef7fb6,
  purple: 0x9b7bef,
  teal: 0x3fbfb4,
  brown: 0x9a6b45,
  black: 0x2a2a2a,
};

const colour = (c: string) => (c.startsWith("#") ? parseInt(c.slice(1), 16) : (COLORS[c] ?? COLORS.white));

/** Shapes with flat faces keep their white outlines and faceted shading (the pixel look);
 *  round ones are shaded smooth. */
const FACETED = new Set(["box", "plane", "pyramid", "extrude", "gear", "cylinder", "cone", "disc"]);
const OUTLINED = new Set(["box", "plane", "pyramid", "extrude", "gear", "cylinder", "cone", "disc"]);

/** A gear's outline on the ground: `teeth` square teeth around a circle of radius r. */
function gearOutline(r: number, teeth: number): THREE.Vector2[] {
  const pts: THREE.Vector2[] = [];
  const inner = r * 0.8;
  for (let i = 0; i < teeth; i++) {
    const a = (i / teeth) * Math.PI * 2;
    const step = (Math.PI * 2) / teeth;
    for (const [f, rad] of [[0, inner], [0.2, r], [0.5, r], [0.7, inner]] as const) pts.push(new THREE.Vector2(Math.cos(a + step * f) * rad, Math.sin(a + step * f) * rad));
  }
  return pts;
}

/** A flat outline (x, z) pushed up by h, standing on y = 0. */
function extruded(points: THREE.Vector2[], h: number): THREE.BufferGeometry {
  const shape = new THREE.Shape(points.map((p) => new THREE.Vector2(p.x, -p.y)));
  return new THREE.ExtrudeGeometry(shape, { depth: h, bevelEnabled: false, curveSegments: 6 }).rotateX(-Math.PI / 2);
}

function geometry(o: Scene3DObject): THREE.BufferGeometry {
  const [w, h, d] = o.size;
  // Many small copies don't need as many triangles each.
  const many = (o.copies?.length ?? 0) > 20;
  switch (o.shape) {
    case "sphere":
      return many ? new THREE.SphereGeometry(w / 2, 12, 8) : new THREE.SphereGeometry(w / 2, 28, 18);
    case "dome":
      return new THREE.SphereGeometry(w / 2, 28, 12, 0, Math.PI * 2, 0, Math.PI / 2);
    case "cylinder":
      return new THREE.CylinderGeometry(w / 2, w / 2, h, 28);
    case "disc":
      return new THREE.CylinderGeometry(w / 2, w / 2, Math.max(0.04, w * 0.06), 36);
    case "cone":
      return new THREE.ConeGeometry(w / 2, h, 28);
    case "pyramid":
      return new THREE.ConeGeometry(w / Math.SQRT2, h, 4).rotateY(Math.PI / 4);
    // Rings and tori lie flat, like an orbit or a donut on a table.
    case "torus":
      return new THREE.TorusGeometry(w / 2, Math.max(0.05, w / 8), 14, 40).rotateX(Math.PI / 2);
    case "ring":
      return new THREE.TorusGeometry(w / 2, Math.max(0.015, w / 120), 4, 96).rotateX(Math.PI / 2);
    case "capsule":
      return new THREE.CapsuleGeometry(w / 2, Math.max(0.01, h - w), 8, 18);
    case "plane":
      return new THREE.BoxGeometry(w, 0.04, d);
    case "tube": {
      const pts = (o.path ?? []).map((p) => new THREE.Vector3(p[0], p[1], p[2]));
      const curve = new THREE.CatmullRomCurve3(pts, !!o.closed, "catmullrom", 0.5);
      return new THREE.TubeGeometry(curve, Math.min(220, pts.length * 10), Math.min(1, Math.max(0.02, w / 2)), 10, !!o.closed);
    }
    case "lathe":
      return new THREE.LatheGeometry((o.profile ?? []).map((p) => new THREE.Vector2(Math.max(0.001, p[0]), p[1])), 36);
    case "extrude":
      return extruded((o.outline ?? []).map((p) => new THREE.Vector2(p[0], p[1])), h);
    case "gear":
      return extruded(gearOutline(w / 2, o.teeth ?? 12), Math.min(h, Math.max(0.1, w * 0.25))).translate(0, -Math.min(h, Math.max(0.1, w * 0.25)) / 2, 0);
    default:
      return new THREE.BoxGeometry(w, h, d);
  }
}

function material(o: Scene3DObject): THREE.MeshStandardMaterial {
  const c = colour(o.color);
  const m = new THREE.MeshStandardMaterial({ color: c, roughness: 0.72, metalness: 0.02, flatShading: FACETED.has(o.shape) });
  if (o.material === "metal") Object.assign(m, { roughness: 0.28, metalness: 0.9 });
  if (o.material === "glass") Object.assign(m, { roughness: 0.08, metalness: 0.1, depthWrite: false });
  if (o.material === "glow") {
    m.emissive.setHex(c);
    m.emissiveIntensity = 0.85;
  }
  const opacity = o.opacity ?? 1;
  m.transparent = opacity < 1;
  m.opacity = opacity;
  // Thin and open shapes show both sides.
  if (["dome", "lathe", "extrude", "gear"].includes(o.shape) || opacity < 1) m.side = THREE.DoubleSide;
  return m;
}

type Motion = NonNullable<Scene3DObject["motion"]>;
type Part = {
  name: string;
  /** Moves and turns; holds the mesh, its outline, its label and any parts that belong to it. */
  pivot: THREE.Group;
  mesh: THREE.Mesh;
  mat: THREE.MeshStandardMaterial;
  edges?: THREE.LineSegments;
  label?: HTMLDivElement;
  born: number;
  radius: number;
  /** Where it sits when nothing moves it, and its resting turn. */
  rest: THREE.Vector3;
  turn: THREE.Euler;
  /** Which way it goes in the exploded view. */
  out: THREE.Vector3;
  motion: Motion;
  opacity: number;
  glow: number;
};
type Link = { a: Part; b: Part; line: THREE.Line; head: THREE.Mesh; tag?: CSS2DObject; dots?: THREE.Mesh[] };

export function Scene3D() {
  const host = useRef<HTMLDivElement>(null);
  const [title, setTitle] = useState("");
  const [empty, setEmpty] = useState(true);
  const [exploded, setExploded] = useState(false);
  const [cut, setCut] = useState(false);
  const [moving, setMoving] = useState(false);
  const [playing, setPlaying] = useState(true);
  const act = useRef({ explode: (_on: boolean) => {}, cut: (_on: boolean) => {}, play: (_on: boolean) => {}, reset: () => {} });

  useEffect(() => {
    const el = host.current!;
    // Retina is sharp enough at 1.5x; half the pixels of 2x. It drops further if frames run late.
    let dpr = Math.min(window.devicePixelRatio, 1.5);
    const renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: "high-performance" });
    renderer.setPixelRatio(dpr);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.05;
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    // Shadows are redrawn only on frames where something moved.
    renderer.shadowMap.autoUpdate = false;
    renderer.localClippingEnabled = true;
    el.appendChild(renderer.domElement);
    const labels = new CSS2DRenderer();
    labels.domElement.className = "scene3d__labels";
    el.appendChild(labels.domElement);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0d0d0d);
    // Soft light from every side (one small image, made once), plus a sun for shadows.
    const pmrem = new THREE.PMREMGenerator(renderer);
    const room = new RoomEnvironment();
    const envMap = pmrem.fromScene(room, 0.04).texture;
    scene.environment = envMap;
    scene.environmentIntensity = 0.55;
    room.dispose();
    pmrem.dispose();
    scene.add(new THREE.HemisphereLight(0xffffff, 0x1a1a1a, 0.7));
    const sun = new THREE.DirectionalLight(0xffffff, 2.1);
    sun.castShadow = true;
    sun.shadow.mapSize.set(1024, 1024);
    sun.shadow.bias = -0.0005;
    sun.shadow.normalBias = 0.02;
    scene.add(sun, sun.target);
    const grid = new THREE.GridHelper(40, 40, 0x2a2a2a, 0x1c1c1c);
    scene.add(grid);
    const floor = new THREE.Mesh(new THREE.PlaneGeometry(80, 80).rotateX(-Math.PI / 2), new THREE.ShadowMaterial({ opacity: 0.38 }));
    floor.receiveShadow = true;
    scene.add(floor);

    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 400);
    camera.position.set(7, 5, 9);
    const controls = new OrbitControls(camera, labels.domElement);
    controls.enableDamping = true;
    controls.autoRotate = true;
    controls.autoRotateSpeed = 0.8;

    const group = new THREE.Group();
    scene.add(group);
    const parts = new Map<string, Part>();
    let links: Link[] = [];
    const slice = new THREE.Plane(new THREE.Vector3(0, 0, -1), 0);
    const centre = new THREE.Vector3();
    let span = 4;
    let home = { target: new THREE.Vector3(), pos: new THREE.Vector3(7, 5, 9) };
    let aim: { target: THREE.Vector3; pos: THREE.Vector3 } | null = null;
    let lit: { name: string; until: number } | null = null;
    let activeUntil = 0;
    let growing = false;
    let frame = 0;
    let lastDraw = 0;
    // The clock motion runs on: it stops while paused or hidden.
    let clock = 0;
    let lastTick = 0;
    let play = true;
    let hasMotion = false;
    // 0…1, eased towards the wanted value.
    let explode = 0, explodeTo = 0;
    let cutAt = 0, cutTo = 0;
    let cutOn = false;
    // Frames that came late while they were dragging: the sign to lower the quality.
    let late = 0, quality = 0;
    let meter = new URLSearchParams(location.search).get("fps") ? { ms: 0, n: 0, since: performance.now() } : null;

    const size = () => {
      const w = el.clientWidth || window.innerWidth;
      const h = el.clientHeight || window.innerHeight;
      renderer.setSize(w, h);
      labels.setSize(w, h);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      wake(300);
    };

    const v1 = new THREE.Vector3(), v2 = new THREE.Vector3(), v3 = new THREE.Vector3();
    const up = new THREE.Vector3(0, 1, 0);

    /** Put every part where its motion and the exploded view say it is at `t` seconds. */
    const pose = (t: number) => {
      for (const p of parts.values()) {
        const m = p.motion;
        p.pivot.position.copy(p.rest);
        if (m.orbit) {
          const c = typeof m.orbit.around === "string" ? (parts.get(m.orbit.around)?.rest ?? v3.set(0, 0, 0)) : v3.set(m.orbit.around[0], m.orbit.around[1], m.orbit.around[2]);
          v1.copy(p.rest).sub(c).applyAxisAngle(up, (t / m.orbit.period) * Math.PI * 2);
          p.pivot.position.copy(c).add(v1);
        }
        if (m.bob) p.pivot.position.y += Math.sin((t / m.bob.period) * Math.PI * 2) * m.bob.height;
        if (explode > 0) p.pivot.position.addScaledVector(p.out, explode);
        if (m.spin) {
          p.pivot.rotation.copy(p.turn);
          p.pivot.rotation[m.axis ?? "y"] += THREE.MathUtils.degToRad(m.spin) * t;
        }
        if (m.pulse) {
          // A beat: a quick swell and a slow settle.
          const phase = (t / m.pulse.period) % 1;
          p.mesh.scale.setScalar(1 + m.pulse.amount * Math.pow(Math.sin(Math.PI * Math.min(1, phase * 2.2)), 2));
        }
      }
    };

    /** Arrows follow the parts they join. */
    const route = (t: number) => {
      for (const l of links) {
        l.a.pivot.getWorldPosition(v1);
        l.b.pivot.getWorldPosition(v2);
        const dir = v3.copy(v2).sub(v1);
        const len = dir.length();
        if (len < 0.01) continue;
        dir.divideScalar(len);
        const from = v1.addScaledVector(dir, l.a.radius);
        const shaft = Math.max(0.05, len - l.a.radius - l.b.radius);
        const head = Math.min(0.34, shaft * 0.3);
        const pos = l.line.geometry.attributes.position as THREE.BufferAttribute;
        pos.setXYZ(0, from.x, from.y, from.z);
        pos.setXYZ(1, from.x + dir.x * (shaft - head), from.y + dir.y * (shaft - head), from.z + dir.z * (shaft - head));
        pos.needsUpdate = true;
        l.line.geometry.computeBoundingSphere();
        l.head.position.copy(from).addScaledVector(dir, shaft - head / 2);
        l.head.quaternion.setFromUnitVectors(up, dir);
        l.head.scale.set(head * 0.55, head, head * 0.55);
        l.tag?.position.copy(from).addScaledVector(dir, shaft / 2);
        l.dots?.forEach((dot, i) => dot.position.copy(from).addScaledVector(dir, shaft * ((t * 0.45 + i / l.dots!.length) % 1)));
      }
    };

    const tick = () => {
      frame = 0;
      const now = performance.now();
      const dt = lastTick ? Math.min(0.1, (now - lastTick) / 1000) : 0;
      const busy = now < activeUntil || !!aim || growing || explode !== explodeTo || cutAt !== cutTo;
      // Ambient motion (nobody touching it) doesn't need more than 30 frames a second.
      if (!busy && now - lastDraw < 31) {
        frame = requestAnimationFrame(tick);
        return;
      }
      lastTick = now;
      const animating = hasMotion && play;
      if (animating) clock += dt;
      let changed = animating || growing;
      if (explode !== explodeTo) {
        explode += Math.sign(explodeTo - explode) * Math.min(Math.abs(explodeTo - explode), dt * 2.2);
        changed = true;
      }
      if (cutAt !== cutTo) {
        cutAt += Math.sign(cutTo - cutAt) * Math.min(Math.abs(cutTo - cutAt), dt * 1.8);
        // The slice comes in from the front and stops at the middle.
        slice.constant = centre.z + span * 0.6 * (1 - cutAt);
        if (cutAt === 0 && !cutOn) for (const p of parts.values()) p.mat.clippingPlanes = [];
        changed = true;
      }
      if (growing) {
        growing = false;
        for (const p of parts.values()) {
          const t = Math.min(1, Math.max(0, (now - p.born) / 380));
          if (t < 1) growing = true;
          p.pivot.scale.setScalar(Math.max(0.001, t < 1 ? 1 - Math.pow(1 - t, 3) : 1));
          if (p.label) p.label.style.opacity = t < 1 ? `${t}` : "";
        }
      }
      if (changed) {
        pose(clock);
        group.updateMatrixWorld();
        route(clock);
        renderer.shadowMap.needsUpdate = quality < 2;
      }
      if (aim) {
        controls.target.lerp(aim.target, 0.12);
        camera.position.lerp(aim.pos, 0.08);
        if (controls.target.distanceTo(aim.target) < 0.01 && camera.position.distanceTo(aim.pos) < 0.02) aim = null;
      }
      if (lit && now > lit.until) light(null);
      controls.update();
      const t0 = meter ? performance.now() : 0;
      renderer.render(scene, camera);
      labels.render(scene, camera);
      if (meter) {
        // ?fps=1 (development): what a frame costs here, every two seconds.
        renderer.getContext().finish();
        meter.ms += performance.now() - t0;
        meter.n += 1;
        if (now - meter.since > 2000) {
          console.log(`scene3d: ${(meter.n / ((now - meter.since) / 1000)).toFixed(0)} fps, ${(meter.ms / meter.n).toFixed(2)} ms a frame, ${renderer.info.render.calls} draw calls, ${renderer.info.render.triangles} triangles, quality ${quality}`);
          meter = { ms: 0, n: 0, since: now };
        }
        activeUntil = now + 1000;
      }
      // Too slow while they drag: fewer pixels first, then no shadows.
      if (now < activeUntil && lastDraw) {
        late = now - lastDraw > 26 ? late + 1 : Math.max(0, late - 1);
        if (late > 24 && quality < 2) {
          late = 0;
          quality += 1;
          if (quality === 1) {
            dpr = 1;
            renderer.setPixelRatio(dpr);
            size();
          } else {
            renderer.shadowMap.enabled = false;
            floor.visible = false;
            for (const p of parts.values()) p.mat.needsUpdate = true;
          }
        }
      }
      lastDraw = now;
      if (!document.hidden && (busy || animating || controls.autoRotate)) frame = requestAnimationFrame(tick);
    };
    const wake = (ms: number) => {
      activeUntil = Math.max(activeUntil, performance.now() + ms);
      if (!frame && !document.hidden) frame = requestAnimationFrame(tick);
    };
    controls.addEventListener("start", () => {
      controls.autoRotate = false; // they took over
      aim = null;
      wake(1500);
    });
    controls.addEventListener("change", () => wake(400));
    // Hidden (another Space, minimised, covered by a full-screen app): draw nothing.
    const visibility = () => {
      lastTick = 0;
      if (!document.hidden) wake(200);
    };
    document.addEventListener("visibilitychange", visibility);

    // Stop spinning on its own after a while.
    let spinTimer = 0;
    const spinFor = (ms: number) => {
      controls.autoRotate = true;
      clearTimeout(spinTimer);
      spinTimer = window.setTimeout(() => {
        controls.autoRotate = false;
        wake(600);
      }, ms);
    };

    const light = (name: string | null) => {
      for (const [n, p] of parts) {
        const on = name === null || n === name;
        p.mat.transparent = p.opacity < 1 || !on;
        p.mat.opacity = on ? p.opacity : Math.min(p.opacity, 0.18);
        if (name !== null && n === name) {
          if (!p.glow) p.mat.emissive.setHex(0x3a3a3a);
          p.mat.emissiveIntensity = Math.max(1, p.glow);
        } else {
          if (!p.glow) p.mat.emissive.setHex(0x000000);
          p.mat.emissiveIntensity = p.glow;
        }
        if (p.edges) (p.edges.material as THREE.LineBasicMaterial).opacity = on ? 0.85 : 0.12;
        p.label?.classList.toggle("is-lit", name !== null && n === name);
        p.label?.classList.toggle("is-dim", !on);
      }
      lit = name === null ? null : { name, until: performance.now() + 7000 };
      wake(200);
    };

    const clear = () => {
      group.traverse((o) => {
        const m = o as THREE.Mesh;
        m.geometry?.dispose();
        const mat = m.material as THREE.Material | THREE.Material[] | undefined;
        (Array.isArray(mat) ? mat : mat ? [mat] : []).forEach((x) => x.dispose());
      });
      group.clear();
      labels.domElement.replaceChildren();
      parts.clear();
      links = [];
      lit = null;
    };

    const build = (data: SceneData) => {
      setTitle(data.title);
      setEmpty(!data.objects.length);
      clear();
      clock = 0;
      explode = explodeTo = 0;
      cutAt = cutTo = 0;
      cutOn = false;
      setExploded(false);
      setCut(false);
      const start = performance.now();
      growing = true;
      hasMotion = false;
      data.objects.forEach((o, i) => {
        const geo = geometry(o);
        const mat = material(o);
        const copies = o.copies ?? [];
        let mesh: THREE.Mesh;
        if (copies.length) {
          // One draw call for all of them.
          const many = new THREE.InstancedMesh(geo, mat, copies.length + 1);
          const m4 = new THREE.Matrix4();
          many.setMatrixAt(0, m4);
          copies.forEach((c, k) => many.setMatrixAt(k + 1, m4.makeTranslation(c[0], c[1], c[2])));
          many.instanceMatrix.needsUpdate = true;
          many.computeBoundingSphere();
          mesh = many;
        } else {
          mesh = new THREE.Mesh(geo, mat);
        }
        mesh.castShadow = (o.opacity ?? 1) > 0.5;
        mesh.receiveShadow = true;
        const pivot = new THREE.Group();
        pivot.position.set(o.at[0], o.at[1], o.at[2]);
        const r = o.rotate ?? [0, 0, 0];
        pivot.rotation.set(THREE.MathUtils.degToRad(r[0]), THREE.MathUtils.degToRad(r[1]), THREE.MathUtils.degToRad(r[2]));
        pivot.add(mesh);
        let edges: THREE.LineSegments | undefined;
        if (OUTLINED.has(o.shape) && !copies.length && (o.opacity ?? 1) > 0.5) {
          edges = new THREE.LineSegments(new THREE.EdgesGeometry(geo, 30), new THREE.LineBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.85 }));
          mesh.add(edges);
        }
        geo.computeBoundingSphere();
        const sphere = geo.boundingSphere ?? new THREE.Sphere(new THREE.Vector3(), 1);
        // A tube's middle is somewhere along its path, not where it's anchored.
        const radius = o.shape === "tube" || o.shape === "lathe" || o.shape === "extrude" ? Math.min(sphere.radius, 1.2) : sphere.radius;
        let label: HTMLDivElement | undefined;
        if (o.label) {
          label = document.createElement("div");
          label.className = "scene3d__label";
          label.textContent = o.label;
          const tag = new CSS2DObject(label);
          // Above it as seen from outside, however the part itself is tilted.
          tag.position.copy(sphere.center).add(new THREE.Vector3(0, sphere.radius + 0.25, 0).applyQuaternion(pivot.quaternion.clone().invert()));
          pivot.add(tag);
        }
        const parent = o.parent ? parts.get(o.parent) : undefined;
        (parent?.pivot ?? group).add(pivot);
        const motion = o.motion ?? {};
        if (motion.spin || motion.orbit || motion.pulse || motion.bob) hasMotion = true;
        parts.set(o.name.toLowerCase(), {
          name: o.name.toLowerCase(),
          pivot,
          mesh,
          mat,
          edges,
          label,
          born: start + Math.min(i, 30) * 110,
          radius,
          rest: pivot.position.clone(),
          turn: pivot.rotation.clone(),
          out: new THREE.Vector3(),
          motion,
          opacity: o.opacity ?? 1,
          glow: o.material === "glow" ? 0.85 : 0,
        });
      });
      for (const l of data.links) {
        const a = parts.get(l.from.trim().toLowerCase());
        const b = parts.get(l.to.trim().toLowerCase());
        if (!a || !b) continue;
        const line = new THREE.Line(new THREE.BufferGeometry().setAttribute("position", new THREE.BufferAttribute(new Float32Array(6), 3)), new THREE.LineBasicMaterial({ color: 0xd0d0d0 }));
        const head = new THREE.Mesh(new THREE.ConeGeometry(0.5, 1, 12), new THREE.MeshBasicMaterial({ color: 0xd0d0d0 }));
        group.add(line, head);
        const link: Link = { a, b, line, head };
        if (l.label) {
          const d = document.createElement("div");
          d.className = "scene3d__label scene3d__label--link";
          d.textContent = l.label;
          link.tag = new CSS2DObject(d);
          group.add(link.tag);
        }
        if (l.flow) {
          hasMotion = true;
          link.dots = [0, 1, 2].map(() => new THREE.Mesh(new THREE.SphereGeometry(0.07, 10, 8), new THREE.MeshBasicMaterial({ color: 0xffffff })));
          group.add(...link.dots);
        }
        links.push(link);
      }
      setMoving(hasMotion);
      setPlaying(true);
      play = true;
      // Frame everything, and fit the light and the floor to it.
      group.updateMatrixWorld(true);
      const box = new THREE.Box3().setFromObject(group);
      box.getCenter(centre);
      span = Math.max(2, box.getSize(new THREE.Vector3()).length());
      // In the exploded view, parts move away from the middle (or from the part they belong to).
      for (const p of parts.values()) {
        const inside = p.pivot.parent !== group;
        p.out.copy(p.rest);
        if (!inside) p.out.sub(centre);
        if (p.out.lengthSq() < 1e-4) p.out.set(0, inside ? 0.6 : 0, 0);
        // Far enough to see between the parts, near enough to stay in the picture.
        if (inside) p.out.multiplyScalar(0.7);
        else p.out.setLength(span * 0.2);
      }
      controls.target.copy(centre);
      camera.position.copy(centre.clone().add(new THREE.Vector3(0.6, 0.45, 0.8).normalize().multiplyScalar(span * 1.25)));
      home = { target: controls.target.clone(), pos: camera.position.clone() };
      grid.position.y = floor.position.y = box.min.y - 0.6;
      sun.position.copy(centre).add(new THREE.Vector3(0.5, 1, 0.6).multiplyScalar(span * 1.4));
      sun.target.position.copy(centre);
      const reach = span * 1.1;
      Object.assign(sun.shadow.camera, { left: -reach, right: reach, top: reach, bottom: -reach, near: 0.1, far: span * 5 });
      sun.shadow.camera.updateProjectionMatrix();
      pose(0);
      group.updateMatrixWorld(true);
      route(0);
      renderer.shadowMap.needsUpdate = true;
      spinFor(9000);
      wake(Math.min(data.objects.length, 30) * 110 + 800);
    };

    const focus = (name: string) => {
      const p = parts.get(name.toLowerCase());
      if (!p) return;
      controls.autoRotate = false;
      const target = p.pivot.getWorldPosition(new THREE.Vector3());
      const away = camera.position.clone().sub(controls.target).normalize();
      const dist = Math.max(3, p.radius * 5);
      aim = { target, pos: target.clone().addScaledVector(away, dist) };
      light(name.toLowerCase());
    };

    act.current = {
      explode: (on) => {
        explodeTo = on ? 1 : 0;
        setExploded(on);
        wake(900);
      },
      cut: (on) => {
        cutOn = on;
        cutTo = on ? 1 : 0;
        // Sliced-open things show their insides.
        if (on) for (const p of parts.values()) Object.assign(p.mat, { clippingPlanes: [slice], side: THREE.DoubleSide, needsUpdate: true });
        setCut(on);
        wake(900);
      },
      play: (on) => {
        play = on;
        lastTick = 0;
        setPlaying(on);
        wake(200);
      },
      reset: () => {
        aim = { target: home.target.clone(), pos: home.pos.clone() };
        light(null);
        wake(900);
      },
    };

    window.addEventListener("resize", size);
    size();
    api.scene3dGet().then((s) => s && build(s));
    const un = onEvent((e) => {
      if (e.kind === "scene3d.show") build(e.payload as SceneData);
      if (e.kind === "scene3d.focus") focus(String(e.payload.name ?? ""));
      if (e.kind === "scene3d.view") {
        if (typeof e.payload.explode === "boolean") act.current.explode(e.payload.explode);
        if (typeof e.payload.cut === "boolean") act.current.cut(e.payload.cut);
      }
    });
    return () => {
      un.then((f) => f());
      window.removeEventListener("resize", size);
      document.removeEventListener("visibilitychange", visibility);
      cancelAnimationFrame(frame);
      clearTimeout(spinTimer);
      clear();
      envMap.dispose();
      controls.dispose();
      renderer.dispose();
      renderer.forceContextLoss();
      renderer.domElement.remove();
      labels.domElement.remove();
    };
  }, []);

  return (
    <div className="scene3d">
      <div className="scene3d__view" ref={host} />
      <div className="scene3d__top" data-tauri-drag-region>
        <span className="scene3d__title">{title || "3D"}</span>
      </div>
      {!empty && (
        <div className="scene3d__tools">
          <button className={exploded ? "is-on" : ""} data-tip="Pull the parts apart to see how they fit" onClick={() => act.current.explode(!exploded)}>
            Explode
          </button>
          <button className={cut ? "is-on" : ""} data-tip="Slice it open to see inside" onClick={() => act.current.cut(!cut)}>
            Cut
          </button>
          {moving && (
            <button data-tip={playing ? "Stop the motion" : "Start the motion"} onClick={() => act.current.play(!playing)}>
              {playing ? "Pause" : "Play"}
            </button>
          )}
          <button data-tip="Back to the whole model" onClick={() => act.current.reset()}>
            Reset
          </button>
        </div>
      )}
      <div className="scene3d__hint">{empty ? "Ask Pixie to show something in 3D." : "Drag to spin · scroll to zoom · right-drag to move"}</div>
    </div>
  );
}
