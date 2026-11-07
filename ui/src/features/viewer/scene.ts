import {
  AmbientLight,
  Box3,
  DirectionalLight,
  GridHelper,
  Group,
  Mesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  Scene,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { LoadedPlate } from "./load-mesh";

/** Bambu-style build plate, in millimetres, and the gap between plates laid out side by side. */
const PLATE_MM = 256;
const PLATE_GAP_MM = 48;

export type SceneInfo = { triangles: number; size: { x: number; y: number; z: number } };

/** A three.js scene with orbit controls and one bed per plate. Call `dispose` when done. */
export class ViewerScene {
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  private readonly camera = new PerspectiveCamera(40, 1, 0.1, 100_000);
  private readonly controls: OrbitControls;
  private readonly material = new MeshStandardMaterial({ color: 0xc9ced8, metalness: 0.05, roughness: 0.6 });
  private readonly holder = new Group();
  private readonly beds: GridHelper[] = [];
  private readonly observer: ResizeObserver;
  private showBeds = true;
  /** One plate: frame the model; several plates: frame all beds. */
  private focusOnModels = true;
  private frame = 0;
  private readonly canvas: HTMLCanvasElement;

  /** Creates its own canvas in `container`: a canvas whose context was released cannot be used again. */
  constructor(container: HTMLElement, label: string) {
    const canvas = document.createElement("canvas");
    canvas.setAttribute("aria-label", label);
    container.appendChild(canvas);
    this.canvas = canvas;
    this.renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    const key = new DirectionalLight(0xffffff, 2.2);
    key.position.set(1, 2, 1.5);
    const fill = new DirectionalLight(0xffffff, 0.8);
    fill.position.set(-1.5, 0.5, -1);
    this.scene.add(new AmbientLight(0xffffff, 0.5), key, fill, this.holder);
    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(canvas);
    this.resize();
    this.loop();
  }

  /** Lays the plates out left to right, each on its own bed, models standing on it. */
  show(plates: LoadedPlate[]): SceneInfo {
    this.clear();
    let triangles = 0;
    plates.forEach((plate, index) => {
      const bed = new GridHelper(PLATE_MM, 16, 0x6e7686, 0x2a303b);
      bed.visible = this.showBeds;
      this.beds.push(bed);
      const model = new Group();
      // Printing files are Z-up; three.js is Y-up.
      model.rotation.x = -Math.PI / 2;
      model.add(plate.object);
      plate.object.traverse((child) => {
        if (child instanceof Mesh) {
          child.material = this.material;
          triangles += (child.geometry.index?.count ?? child.geometry.getAttribute("position").count) / 3;
        }
      });
      if (!plate.arranged) standOnBed(model);
      const slot = new Group();
      slot.position.x = index * (PLATE_MM + PLATE_GAP_MM);
      slot.add(bed, model);
      this.holder.add(slot);
    });
    this.focusOnModels = plates.length === 1;
    const size = this.modelBox().getSize(new Vector3());
    this.resetView();
    return { triangles: Math.round(triangles), size: { x: size.x, y: size.z, z: size.y } };
  }

  setWireframe(on: boolean): void {
    this.material.wireframe = on;
  }

  setPlate(on: boolean): void {
    this.showBeds = on;
    this.beds.forEach((bed) => (bed.visible = on));
  }

  resetView(): void {
    const models = this.modelBox();
    const box = this.focusOnModels && !models.isEmpty() ? models : new Box3().setFromObject(this.holder);
    const center = box.getCenter(new Vector3());
    const size = box.getSize(new Vector3());
    const radius = Math.max(size.length() / 2, 1);
    // 1.3: a little room around the model, also when the window is wider than tall.
    const distance = (1.3 * radius) / Math.sin((this.camera.fov * Math.PI) / 360);
    this.controls.target.copy(center);
    this.camera.position.set(center.x + distance * 0.55, center.y + distance * 0.5, center.z + distance * 0.65);
    this.camera.near = distance / 100;
    this.camera.far = distance * 100;
    this.camera.updateProjectionMatrix();
    this.controls.update();
  }

  dispose(): void {
    cancelAnimationFrame(this.frame);
    this.observer.disconnect();
    this.controls.dispose();
    this.clear();
    this.material.dispose();
    this.renderer.dispose();
    // WebKit allows only a few live WebGL contexts; release this one right away.
    this.renderer.forceContextLoss();
    this.canvas.remove();
  }

  private clear(): void {
    this.holder.traverse((child) => child instanceof Mesh && child.geometry.dispose());
    this.beds.forEach((bed) => bed.dispose());
    this.beds.length = 0;
    this.holder.clear();
  }

  /** Bounds of the models only, without the beds. */
  private modelBox(): Box3 {
    // Matrices are only refreshed on render; without this the Z-up rotation is not applied yet.
    this.holder.updateMatrixWorld(true);
    const box = new Box3();
    this.holder.traverse((child) => child instanceof Mesh && box.expandByObject(child));
    return box;
  }

  private resize(): void {
    const { clientWidth, clientHeight } = this.canvas;
    if (clientWidth === 0 || clientHeight === 0) return;
    this.renderer.setSize(clientWidth, clientHeight, false);
    this.camera.aspect = clientWidth / clientHeight;
    this.camera.updateProjectionMatrix();
  }

  private loop = (): void => {
    this.frame = requestAnimationFrame(this.loop);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };
}

/** Centers a loose model on its bed with its lowest point on it. */
function standOnBed(model: Group): void {
  const box = new Box3().setFromObject(model);
  const center = box.getCenter(new Vector3());
  model.position.set(-center.x, -box.min.y, -center.z);
}
