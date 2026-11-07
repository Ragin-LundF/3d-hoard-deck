import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { commands, describeError, unwrap } from "../../ipc";
import { useSlicers } from "./queries";
import { t } from "../../shared/i18n";

type Props = { model: number; file: string | null; onMessage: (message: string) => void };

/** "Open in Bambu Studio" plus a menu with the other slicers. Sends the file shown in 3D, else the model's default files. */
export function OpenInSlicer({ model, file, onMessage }: Props) {
  const slicers = useSlicers();
  const [menu, setMenu] = useState(false);
  const open = useMutation({
    mutationFn: (slicer: number | null) => unwrap(commands.openInSlicer(model, file ? [file] : [], slicer)),
    onSuccess: (name) => onMessage(t("Opened in {name}", { name })),
    onError: (error) => onMessage(describeError(error)),
  });
  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(false);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [menu]);

  const list = slicers.data ?? [];
  const first = list[0];
  if (!first) {
    return <button className="btn" disabled title={t("Add a slicer in Settings › Slicers")}>{t("Open in slicer")}</button>;
  }
  return (
    <div className="split">
      <button className="btn primary" disabled={open.isPending} onClick={() => open.mutate(null)} title={file ? t("Opens {file}", { file }) : t("Opens the 3MF project, or the main model files")}>
        {t("Open in {name}", { name: first.name })}
      </button>
      {list.length > 1 && (
        <button className="btn primary more" aria-label={t("Open with another slicer")} aria-haspopup="menu" aria-expanded={menu} onClick={(event) => { event.stopPropagation(); setMenu(!menu); }}>
          ▾
        </button>
      )}
      {menu && (
        <div className="menu split-menu" role="menu">
          {list.map((slicer, index) => (
            <button key={slicer.program} role="menuitem" onClick={() => open.mutate(index)}>
              {slicer.name}
              <span className="k">{slicer.extensions.join(", ")}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
