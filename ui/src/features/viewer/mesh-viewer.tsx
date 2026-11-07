import { useEffect, useRef, useState } from "react";
import { fileName } from "../../shared/format";
import { Icon } from "../../shared/icons";
import type { MeshFormat } from "./formats";
import { loadMesh, type LoadedPlate } from "./load-mesh";
import { ViewerScene, type SceneInfo } from "./scene";

type Props = { library: number; path: string; format: MeshFormat };

/** Interactive preview: drag to rotate, right-drag or two fingers to pan, scroll to zoom. */
export default function MeshViewer({ library, path, format }: Props) {
  const stage = useRef<HTMLDivElement>(null);
  const scene = useRef<ViewerScene | null>(null);
  const [state, setState] = useState<{ loading: boolean; error: string | null; info: SceneInfo | null }>({ loading: true, error: null, info: null });
  const [wireframe, setWireframe] = useState(false);
  const [plate, setPlate] = useState(true);
  const [plates, setPlates] = useState<LoadedPlate[]>([]);
  const [shownPlate, setShownPlate] = useState<number | null>(null);

  useEffect(() => {
    if (!stage.current) return;
    scene.current = new ViewerScene(stage.current, "3D preview");
    return () => {
      scene.current?.dispose();
      scene.current = null;
    };
  }, []);

  useEffect(() => {
    const abort = new AbortController();
    setState({ loading: true, error: null, info: null });
    loadMesh(library, path, format, abort.signal)
      .then((loaded) => {
        setPlates(loaded);
        setShownPlate(null);
        const info = scene.current?.show(loaded) ?? null;
        setState({ loading: false, error: loaded.length === 0 ? "This file has nothing on its plates." : null, info });
      })
      .catch((error: unknown) => {
        if (!abort.signal.aborted) setState({ loading: false, error: error instanceof Error ? error.message : String(error), info: null });
      });
    return () => abort.abort();
  }, [library, path, format]);

  const showPlate = (number: number | null) => {
    setShownPlate(number);
    const info = scene.current?.show(number === null ? plates : plates.filter((p) => p.number === number)) ?? null;
    setState((current) => ({ ...current, info }));
  };

  useEffect(() => scene.current?.setWireframe(wireframe), [wireframe]);
  useEffect(() => scene.current?.setPlate(plate), [plate]);

  const { info } = state;
  return (
    <div className="viewer">
      <div className="viewer-canvas" ref={stage} />
      <div className="vtools">
        <div className="vgroup" role="group" aria-label="Render mode">
          <button className={wireframe ? "" : "on"} aria-pressed={!wireframe} onClick={() => setWireframe(false)}>Solid</button>
          <button className={wireframe ? "on" : ""} aria-pressed={wireframe} onClick={() => setWireframe(true)}>Wireframe</button>
        </div>
        <div className="vgroup" role="group" aria-label="Viewer options">
          <button className={plate ? "on" : ""} aria-pressed={plate} onClick={() => setPlate(!plate)} title="Build plate 256 × 256 mm">Plate</button>
          <button onClick={() => scene.current?.resetView()} aria-label="Reset view" title="Reset view"><Icon name="refresh" /></button>
        </div>
        {plates.length > 1 && (
          <div className="vgroup" role="group" aria-label="Plates">
            <button className={shownPlate === null ? "on" : ""} aria-pressed={shownPlate === null} onClick={() => showPlate(null)}>All plates</button>
            {plates.map((p) => (
              <button key={p.number} className={shownPlate === p.number ? "on" : ""} aria-pressed={shownPlate === p.number} onClick={() => showPlate(p.number)}>
                {p.number}
              </button>
            ))}
          </div>
        )}
      </div>
      {state.loading && <div className="vstatus">Loading {fileName(path)}…</div>}
      {state.error && <div className="vstatus err">{state.error}</div>}
      {info && (
        <div className="vinfo">
          <span><b>{info.size.x.toFixed(1)} × {info.size.y.toFixed(1)} × {info.size.z.toFixed(1)}</b> mm</span>
          <span><b>{info.triangles.toLocaleString()}</b> triangles</span>
          <span>{fileName(path)}</span>
        </div>
      )}
    </div>
  );
}
