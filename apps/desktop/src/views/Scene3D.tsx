// 3D explanations (experimental): labelled shapes and arrows from the `scene3d` tool, in a window
// they can spin. Renders only while something moves (appearing, turning, their drag), so an idle
// window costs nothing.
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
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
};

function geometry(o: Scene3DObject): THREE.BufferGeometry {
  const [w, h, d] = o.size;
  switch (o.shape) {
    case "sphere":
      return new THREE.SphereGeometry(w / 2, 20, 14);
    case "cylinder":
      return new THREE.CylinderGeometry(w / 2, w / 2, h, 24);
    case "cone":
      return new THREE.ConeGeometry(w / 2, h, 24);
    // Rings and tori lie flat, like an orbit or a donut on a table.
    case "torus":
      return new THREE.TorusGeometry(w / 2, Math.max(0.05, w / 8), 12, 32).rotateX(Math.PI / 2);
    case "ring":
      return new THREE.TorusGeometry(w / 2, Math.max(0.015, w / 120), 4, 96).rotateX(Math.PI / 2);
    case "capsule":
      return new THREE.CapsuleGeometry(w / 2, Math.max(0.01, h - w), 6, 16);
    case "plane":
      return new THREE.BoxGeometry(w, 0.04, d);
    default:
      return new THREE.BoxGeometry(w, h, d);
  }
}

type Part = { mesh: THREE.Mesh; edges: THREE.LineSegments; label?: HTMLDivElement; born: number; radius: number };

export function Scene3D() {
  const host = useRef<HTMLDivElement>(null);
  const [title, setTitle] = useState("");
  const [empty, setEmpty] = useState(true);

  useEffect(() => {
    const el = host.current!;
    // Retina is sharp enough at 1.5x without antialiasing; half the pixels of 2x.
    const dpr = Math.min(window.devicePixelRatio, 1.5);
    const renderer = new THREE.WebGLRenderer({ antialias: dpr < 1.5, powerPreference: "high-performance" });
    renderer.setPixelRatio(dpr);
    el.appendChild(renderer.domElement);
    const labels = new CSS2DRenderer();
    labels.domElement.className = "scene3d__labels";
    el.appendChild(labels.domElement);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0d0d0d);
    scene.add(new THREE.HemisphereLight(0xffffff, 0x222222, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.4);
    sun.position.set(5, 8, 6);
    scene.add(sun);
    const grid = new THREE.GridHelper(20, 20, 0x2a2a2a, 0x1c1c1c);
    grid.position.y = -3;
    scene.add(grid);

    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 200);
    camera.position.set(7, 5, 9);
    const controls = new OrbitControls(camera, labels.domElement);
    controls.enableDamping = true;
    controls.autoRotate = true;
    controls.autoRotateSpeed = 0.8;

    const group = new THREE.Group();
    scene.add(group);
    const parts = new Map<string, Part>();
    let aim: { target: THREE.Vector3; pos: THREE.Vector3 } | null = null;
    let lit: { name: string; until: number } | null = null;
    let activeUntil = 0;
    let growing = false;
    let frame = 0;

    const size = () => {
      const w = el.clientWidth || window.innerWidth;
      const h = el.clientHeight || window.innerHeight;
      renderer.setSize(w, h);
      labels.setSize(w, h);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      wake(300);
    };

    const tick = () => {
      frame = 0;
      const now = performance.now();
      if (growing) {
        growing = false;
        for (const p of parts.values()) {
          const t = Math.min(1, Math.max(0, (now - p.born) / 380));
          if (t < 1) growing = true;
          const s = t < 1 ? 1 - Math.pow(1 - t, 3) : 1;
          p.mesh.scale.setScalar(Math.max(0.001, s));
          p.edges.scale.setScalar(Math.max(0.001, s));
          if (p.label) p.label.style.opacity = t < 1 ? `${t}` : "";
        }
      }
      if (aim) {
        controls.target.lerp(aim.target, 0.12);
        camera.position.lerp(aim.pos, 0.08);
        if (controls.target.distanceTo(aim.target) < 0.01 && camera.position.distanceTo(aim.pos) < 0.02) aim = null;
      }
      if (lit && now > lit.until) light(null);
      controls.update();
      renderer.render(scene, camera);
      labels.render(scene, camera);
      if (now < activeUntil || aim || controls.autoRotate) frame = requestAnimationFrame(tick);
    };
    const wake = (ms: number) => {
      activeUntil = Math.max(activeUntil, performance.now() + ms);
      if (!frame) frame = requestAnimationFrame(tick);
    };
    controls.addEventListener("start", () => {
      controls.autoRotate = false; // they took over
      aim = null;
      wake(1500);
    });
    controls.addEventListener("change", () => wake(400));

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
        const m = p.mesh.material as THREE.MeshLambertMaterial;
        m.opacity = on ? 0.92 : 0.25;
        m.emissive.setHex(name !== null && n === name ? 0x333333 : 0x000000);
        (p.edges.material as THREE.LineBasicMaterial).opacity = on ? 0.9 : 0.15;
        p.label?.classList.toggle("is-lit", name !== null && n === name);
        p.label?.classList.toggle("is-dim", !on);
      }
      lit = name === null ? null : { name, until: performance.now() + 7000 };
      wake(200);
    };

    const build = (data: SceneData) => {
      setTitle(data.title);
      setEmpty(!data.objects.length);
      for (const p of parts.values()) {
        group.remove(p.mesh, p.edges);
        p.mesh.geometry.dispose();
        (p.mesh.material as THREE.Material).dispose();
        p.edges.geometry.dispose();
      }
      parts.clear();
      group.clear();
      lit = null;
      const start = performance.now();
      growing = true;
      data.objects.forEach((o, i) => {
        const geo = geometry(o);
        const mesh = new THREE.Mesh(
          geo,
          new THREE.MeshLambertMaterial({ color: COLORS[o.color] ?? COLORS.white, flatShading: true, transparent: true, opacity: 0.92 }),
        );
        mesh.position.set(o.at[0], o.at[1], o.at[2]);
        // Outlines give the pixel look; thin rings would just turn into a smear of lines.
        const curved = ["ring", "sphere", "torus", "capsule"].includes(o.shape);
        const edges = new THREE.LineSegments(curved ? new THREE.BufferGeometry() : new THREE.EdgesGeometry(geo, 30), new THREE.LineBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.9 }));
        edges.position.copy(mesh.position);
        group.add(mesh, edges);
        geo.computeBoundingSphere();
        const radius = geo.boundingSphere?.radius ?? 1;
        let label: HTMLDivElement | undefined;
        if (o.label) {
          label = document.createElement("div");
          label.className = "scene3d__label";
          label.textContent = o.label;
          const tag = new CSS2DObject(label);
          tag.position.set(0, radius + 0.25, 0);
          mesh.add(tag);
        }
        parts.set(o.name.toLowerCase(), { mesh, edges, label, born: start + i * 140, radius });
      });
      for (const l of data.links) {
        const a = parts.get(l.from.trim().toLowerCase());
        const b = parts.get(l.to.trim().toLowerCase());
        if (!a || !b) continue;
        const from = a.mesh.position.clone();
        const dir = b.mesh.position.clone().sub(from);
        const len = dir.length();
        if (len < 0.01) continue;
        dir.normalize();
        const s = from.clone().addScaledVector(dir, a.radius);
        const shaft = Math.max(0.1, len - a.radius - b.radius);
        const arrow = new THREE.ArrowHelper(dir, s, shaft, 0xd0d0d0, Math.min(0.4, shaft * 0.3), Math.min(0.22, shaft * 0.18));
        group.add(arrow);
        if (l.label) {
          const d = document.createElement("div");
          d.className = "scene3d__label scene3d__label--link";
          d.textContent = l.label;
          const tag = new CSS2DObject(d);
          tag.position.copy(s.clone().addScaledVector(dir, shaft / 2));
          group.add(tag);
        }
      }
      // Frame everything.
      const box = new THREE.Box3().setFromObject(group);
      const center = box.getCenter(new THREE.Vector3());
      const span = Math.max(2, box.getSize(new THREE.Vector3()).length());
      controls.target.copy(center);
      camera.position.copy(center.clone().add(new THREE.Vector3(0.6, 0.45, 0.8).normalize().multiplyScalar(span * 1.25)));
      grid.position.y = box.min.y - 0.6;
      spinFor(9000);
      wake(data.objects.length * 140 + 800);
    };

    const focus = (name: string) => {
      const p = parts.get(name.toLowerCase());
      if (!p) return;
      controls.autoRotate = false;
      const target = p.mesh.position.clone();
      const away = camera.position.clone().sub(controls.target).normalize();
      const dist = Math.max(3, p.radius * 5);
      aim = { target, pos: target.clone().addScaledVector(away, dist) };
      light(name.toLowerCase());
    };

    window.addEventListener("resize", size);
    size();
    api.scene3dGet().then((s) => s && build(s));
    const un = onEvent((e) => {
      if (e.kind === "scene3d.show") build(e.payload as SceneData);
      if (e.kind === "scene3d.focus") focus(String(e.payload.name ?? ""));
    });
    return () => {
      un.then((f) => f());
      window.removeEventListener("resize", size);
      cancelAnimationFrame(frame);
      clearTimeout(spinTimer);
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
      <div className="scene3d__hint">{empty ? "Ask Pixie to show something in 3D." : "Drag to spin · scroll to zoom · right-drag to move"}</div>
    </div>
  );
}
