/// <reference lib="webworker" />
// Parses STL (binary or ASCII) off the UI thread; only the vertex arrays travel back, without copying.
import { STLLoader } from "three/examples/jsm/loaders/STLLoader.js";

self.onmessage = (event: MessageEvent<ArrayBuffer>) => {
  try {
    const geometry = new STLLoader().parse(event.data);
    // Many exporters write zero normals; recomputing gives every file correct flat shading.
    geometry.computeVertexNormals();
    const positions = geometry.getAttribute("position").array as Float32Array;
    const normals = geometry.getAttribute("normal").array as Float32Array;
    self.postMessage({ positions, normals }, [positions.buffer, normals.buffer]);
  } catch (error) {
    self.postMessage({ error: error instanceof Error ? error.message : String(error) });
  }
};
