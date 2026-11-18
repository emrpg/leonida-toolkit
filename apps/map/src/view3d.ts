import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { WORLD_EXTENT, type Landmark, type Landmark3D } from './types';

export interface View3DOptions {
  container: HTMLElement;
  landmarks3d: readonly Landmark3D[];
  landmarks2d: readonly Landmark[];
  onSelectLandmark: (info: { id: string; name: string; x: number; y: number; height?: number }) => void;
}

export class Leonida3DView {
  private renderer: THREE.WebGLRenderer | null = null;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls | null = null;
  private meshes: THREE.Mesh[] = [];
  private roamMode = false;
  private keys = new Set<string>();
  private yaw = 0;
  private pitch = -0.25;
  private dragging = false;
  private lastMouse = { x: 0, y: 0 };
  private animId = 0;
  private disposed = false;

  constructor(private readonly opts: View3DOptions) {
    const w = opts.container.clientWidth || 1000;
    const h = opts.container.clientHeight || 700;
    this.camera = new THREE.PerspectiveCamera(55, w / h, 5, 80000);
    this.camera.position.set(1200, 1400, 2600);

    try {
      this.renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: 'high-performance' });
      this.renderer.setSize(w, h);
      this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      this.renderer.setClearColor(0x070a12, 1);
      opts.container.appendChild(this.renderer.domElement);
    } catch (err) {
      opts.container.innerHTML = `<div class="glass-card" style="margin:24px">WebGL 3D 视界不可用：${String(err)}</div>`;
      return;
    }

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.set(0, 0, 0);
    this.controls.enableDamping = true;
    this.controls.maxDistance = 35000;

    this.buildScene();
    this.bindEvents();
    this.animate();
  }

  private buildScene() {
    this.scene.fog = new THREE.FogExp2(0x070a12, 0.000035);
    const ambient = new THREE.AmbientLight(0xdce6ff, 0.75);
    const dir = new THREE.DirectionalLight(0xffb347, 1.1);
    dir.position.set(6000, 12000, 4000);
    this.scene.add(ambient, dir);

    // Ground plane (32768 x 32768 m)
    const groundGeo = new THREE.PlaneGeometry(WORLD_EXTENT.size, WORLD_EXTENT.size);
    const groundMat = new THREE.MeshStandardMaterial({
      color: 0x101828,
      roughness: 0.9,
      metalness: 0.1,
    });
    const loader = new THREE.TextureLoader();
    loader.load(
      'tiles/dupzor,51/0/0,0,0.jpg',
      (tex) => {
        tex.colorSpace = THREE.SRGBColorSpace;
        groundMat.map = tex;
        groundMat.color.setHex(0xffffff);
        groundMat.needsUpdate = true;
      },
      undefined,
      () => {
        // Keep dark slate ground if tile texture unavailable
      },
    );
    const ground = new THREE.Mesh(groundGeo, groundMat);
    ground.rotation.x = -Math.PI / 2;
    this.scene.add(ground);

    const grid = new THREE.GridHelper(WORLD_EXTENT.size, 32, 0xff4fa3, 0x1e2638);
    grid.position.y = 1;
    this.scene.add(grid);

    // 107 3D Landmarks
    const edgeMat = new THREE.LineBasicMaterial({ color: 0x00e5ff, transparent: true, opacity: 0.65 });
    for (const l of this.opts.landmarks3d) {
      const w = Math.max(12, l.width || 16);
      const h = Math.max(12, l.height || 40);
      const geo = new THREE.BoxGeometry(w, h, w);
      const hueColor = new THREE.Color().setHSL(Math.max(0.48, 0.56 - h / 1200), 0.9, 0.55);
      const mat = new THREE.MeshStandardMaterial({
        color: hueColor,
        emissive: hueColor,
        emissiveIntensity: 0.28,
        transparent: true,
        opacity: 0.85,
      });
      const mesh = new THREE.Mesh(geo, mat);
      mesh.position.set(l.xyz.x, h / 2, -l.xyz.y);
      mesh.userData = { id: l.id, name: l.name, x: l.xyz.x, y: l.xyz.y, height: h };
      const wire = new THREE.LineSegments(new THREE.EdgesGeometry(geo), edgeMat);
      mesh.add(wire);
      this.scene.add(mesh);
      this.meshes.push(mesh);
    }

    // 2D landmarks as ground beacons
    const dotGeo = new THREE.CylinderGeometry(8, 8, 6, 8);
    const dotMat = new THREE.MeshBasicMaterial({ color: 0xff4fa3 });
    for (const l of this.opts.landmarks2d) {
      if (!l.ig) continue;
      const dot = new THREE.Mesh(dotGeo, dotMat);
      dot.position.set(l.ig.x, 3, -l.ig.y);
      dot.userData = { id: l.id, name: l.name, x: l.ig.x, y: l.ig.y };
      this.scene.add(dot);
      this.meshes.push(dot);
    }
  }

  private bindEvents() {
    if (!this.renderer) return;
    const dom = this.renderer.domElement;
    const raycaster = new THREE.Raycaster();
    const pointer = new THREE.Vector2();

    dom.addEventListener('click', (e) => {
      if (this.dragging) return;
      const rect = dom.getBoundingClientRect();
      pointer.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
      pointer.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
      raycaster.setFromCamera(pointer, this.camera);
      const hits = raycaster.intersectObjects(this.meshes, false);
      if (hits.length) {
        const d = hits[0]!.object.userData as {
          id: string;
          name: string;
          x: number;
          y: number;
          height?: number;
        };
        this.opts.onSelectLandmark(d);
      }
    });

    dom.addEventListener('mousedown', (e) => {
      this.dragging = false;
      this.lastMouse = { x: e.clientX, y: e.clientY };
    });
    dom.addEventListener('mousemove', (e) => {
      if (e.buttons !== 1) return;
      const dx = e.clientX - this.lastMouse.x;
      const dy = e.clientY - this.lastMouse.y;
      if (Math.abs(dx) + Math.abs(dy) > 3) this.dragging = true;
      this.lastMouse = { x: e.clientX, y: e.clientY };
      if (this.roamMode) {
        this.yaw -= dx * 0.004;
        this.pitch = Math.max(-1.4, Math.min(1.4, this.pitch - dy * 0.004));
        this.camera.rotation.order = 'YXZ';
        this.camera.rotation.y = this.yaw;
        this.camera.rotation.x = this.pitch;
      }
    });

    window.addEventListener('keydown', (e) => this.keys.add(e.code));
    window.addEventListener('keyup', (e) => this.keys.delete(e.code));
    window.addEventListener('resize', () => this.resize());
  }

  setRoamMode(enabled: boolean) {
    this.roamMode = enabled;
    if (this.controls) this.controls.enabled = !enabled;
    if (enabled) {
      this.camera.rotation.order = 'YXZ';
      this.yaw = this.camera.rotation.y;
      this.pitch = this.camera.rotation.x;
    }
  }

  focusPoint(x: number, y: number, z = 400) {
    this.camera.position.set(x + 350, z + 260, -y + 350);
    if (this.controls && !this.roamMode) {
      this.controls.target.set(x, 40, -y);
      this.controls.update();
    }
  }

  resize() {
    if (!this.renderer) return;
    const w = this.opts.container.clientWidth;
    const h = this.opts.container.clientHeight;
    if (w <= 0 || h <= 0) return;
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  private animate = () => {
    if (this.disposed) return;
    this.animId = requestAnimationFrame(this.animate);
    if (this.roamMode) {
      const speed = this.keys.has('ShiftLeft') ? 80 : 28;
      const forward = new THREE.Vector3(0, 0, -1).applyEuler(this.camera.rotation);
      const right = new THREE.Vector3(1, 0, 0).applyEuler(this.camera.rotation);
      if (this.keys.has('KeyW') || this.keys.has('ArrowUp')) this.camera.position.addScaledVector(forward, speed);
      if (this.keys.has('KeyS') || this.keys.has('ArrowDown')) this.camera.position.addScaledVector(forward, -speed);
      if (this.keys.has('KeyA') || this.keys.has('ArrowLeft')) this.camera.position.addScaledVector(right, -speed);
      if (this.keys.has('KeyD') || this.keys.has('ArrowRight')) this.camera.position.addScaledVector(right, speed);
      if (this.keys.has('Space')) this.camera.position.y += speed * 0.6;
      if (this.keys.has('KeyC')) this.camera.position.y = Math.max(10, this.camera.position.y - speed * 0.6);
    } else {
      this.controls?.update();
    }
    this.renderer?.render(this.scene, this.camera);
  };

  dispose() {
    this.disposed = true;
    cancelAnimationFrame(this.animId);
    this.renderer?.dispose();
  }
}
