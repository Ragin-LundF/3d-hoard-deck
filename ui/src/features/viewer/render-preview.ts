import { AmbientLight, Box3, Color, DirectionalLight, Group, Mesh, MeshStandardMaterial, PerspectiveCamera, Scene, Sphere, Vector3, WebGLRenderer, type Object3D } from "three";
import type { MeshFormat } from "./formats";
import { loadMesh } from "./load-mesh";

const SIZE = 512;
/** One renderer for all previews: browsers allow only a few WebGL contexts. */
let renderer: WebGLRenderer | null = null;

/**
 * Renders a model file to a 512 × 512 PNG: neutral grey on light grey, the same in both themes, from a 3/4 view, so a
 * render is never mistaken for a photo.
 */
export async function renderPreview(library: number, path: string, format: MeshFormat, signal: AbortSignal): Promise<Uint8Array> {
  const loaded = await loadMesh(library, path, format, signal);
  const object = loaded.plates[0]?.object;
  if (!object) throw new Error("nothing to render");
  renderer ??= new WebGLRenderer({ antialias: true, preserveDrawingBuffer: true, canvas: document.createElement("canvas") });
  renderer.setSize(SIZE, SIZE, false);
  const scene = new Scene();
  scene.background = new Color(0xe6e8ec);
  const material = new MeshStandardMaterial({ color: 0x9aa1ad, metalness: 0.05, roughness: 0.65 });
  object.traverse((child: Object3D) => {
    if (child instanceof Mesh) child.material = material;
  });
  const holder = new Group();
  // Printing files are Z-up; three.js is Y-up.
  object.rotation.x = -Math.PI / 2;
  holder.add(object);
  holder.updateMatrixWorld(true);
  const sphere = new Box3().setFromObject(holder).getBoundingSphere(new Sphere());
  const camera = new PerspectiveCamera(30, 1, 0.1, Math.max(10, sphere.radius * 20));
  const distance = sphere.radius / Math.sin((camera.fov * Math.PI) / 360) * 1.05;
  camera.position.copy(sphere.center).add(new Vector3(0.9, 0.75, 1.2).normalize().multiplyScalar(distance));
  camera.lookAt(sphere.center);
  const key = new DirectionalLight(0xffffff, 2.2);
  key.position.set(1, 2, 1.5);
  const fill = new DirectionalLight(0xffffff, 0.7);
  fill.position.set(-1.5, 0.5, -1);
  scene.add(new AmbientLight(0xffffff, 0.55), key, fill, holder);
  renderer.render(scene, camera);
  const blob = await new Promise<Blob | null>((resolve) => renderer!.domElement.toBlob(resolve, "image/png"));
  object.traverse((child: Object3D) => {
    if (child instanceof Mesh) child.geometry.dispose();
  });
  material.dispose();
  if (!blob) throw new Error("the preview could not be rendered");
  return new Uint8Array(await blob.arrayBuffer());
}
