import { BufferAttribute, BufferGeometry, Group, Mesh, type Object3D } from "three";
import { OBJLoader } from "three/examples/jsm/loaders/OBJLoader.js";
import { fileUrl, platesUrl } from "../../ipc";
import type { MeshFormat } from "./formats";
import { decodePlates } from "./plates";

/** One build plate; files other than 3MF have exactly one. Objects are Z-up, in millimetres. */
export type LoadedPlate = { number: number; object: Object3D; arranged: boolean };

/** Loads a library file for the viewer. 3MF comes pre-arranged per plate from the shell. */
export async function loadMesh(library: number, path: string, format: MeshFormat, signal: AbortSignal): Promise<LoadedPlate[]> {
  const url = format === "3mf" ? platesUrl(library, path) : fileUrl(library, path);
  const response = await fetch(url, { signal });
  if (response.status === 413) throw new Error("This file is too large for the preview yet.");
  if (response.status === 422) throw new Error("This 3MF file could not be read.");
  if (!response.ok) throw new Error("The file could not be read. Is the library reachable?");
  const buffer = await response.arrayBuffer();
  if (format === "3mf") {
    return decodePlates(buffer).map((plate) => ({ number: plate.number, object: meshOf(plate.positions), arranged: true }));
  }
  const object = format === "obj" ? new OBJLoader().parse(new TextDecoder().decode(buffer)) : await parseStl(buffer, signal);
  return [{ number: 1, object, arranged: false }];
}

function meshOf(positions: Float32Array, normals?: Float32Array): Object3D {
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new BufferAttribute(positions, 3));
  if (normals) geometry.setAttribute("normal", new BufferAttribute(normals, 3));
  else geometry.computeVertexNormals();
  const group = new Group();
  group.add(new Mesh(geometry));
  return group;
}

function parseStl(buffer: ArrayBuffer, signal: AbortSignal): Promise<Object3D> {
  const worker = new Worker(new URL("./stl-worker.ts", import.meta.url), { type: "module" });
  return new Promise<Object3D>((resolve, reject) => {
    signal.addEventListener("abort", () => {
      worker.terminate();
      reject(new DOMException("aborted", "AbortError"));
    });
    worker.onerror = () => {
      worker.terminate();
      reject(new Error("The STL could not be parsed."));
    };
    worker.onmessage = (event: MessageEvent<{ positions?: Float32Array; normals?: Float32Array; error?: string }>) => {
      worker.terminate();
      const { positions, normals, error } = event.data;
      if (!positions || !normals) return reject(new Error(error ?? "Not a valid STL file."));
      resolve(meshOf(positions, normals));
    };
    worker.postMessage(buffer, [buffer]);
  });
}
