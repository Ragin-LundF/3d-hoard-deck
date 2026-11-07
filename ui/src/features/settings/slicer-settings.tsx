import { useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { commands, describeError, unwrap, type SlicerDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import { useSaveSlicers, useSlicers } from "../slicers/queries";
import { t } from "../../shared/i18n";

const DEFAULT_EXTENSIONS = ["stl", "3mf", "obj"];

/** The slicers "Open in" can use. The first one is the default; others are in the button's menu. */
export function SlicerSettings({ onError }: { onError: (message: string) => void }) {
  const slicers = useSlicers();
  const save = useSaveSlicers((error) => onError(describeError(error)));
  const queryClient = useQueryClient();
  const detect = useMutation({
    mutationFn: () => unwrap(commands.detectSlicers()),
    onSuccess: (list) => queryClient.setQueryData(["slicers"], list),
    onError: (error) => onError(describeError(error)),
  });
  const list = slicers.data ?? [];
  const update = (next: SlicerDto[]) => save.mutate(next);

  const add = async () => {
    const program = await open({ directory: false, title: t("Choose the slicer program") });
    if (typeof program !== "string") return;
    const name = (program.split(/[\\/]/).pop() ?? program).replace(/\.(app|exe)$/i, "");
    update([...list, { name, program, args: ["{files}"], extensions: DEFAULT_EXTENSIONS }]);
  };

  return (
    <>
      <div className="panel">
        {list.length === 0 && (
          <div className="prow2"><div className="t"><b>{t("No slicers yet")}</b><span>{t("Find installed ones or add a program yourself.")}</span></div></div>
        )}
        {list.map((slicer, index) => (
          <div className="prow2" key={slicer.program}>
            <div className="t">
              <b>{slicer.name} {index === 0 && <span className="pill ok-pill">{t("Default")}</span>}</b>
              <span className="mono">{slicer.program}</span>
              <label className="ext">
                {t("File types")}
                <input
                  type="text"
                  defaultValue={slicer.extensions.join(", ")}
                  aria-label={t("File types for {name}", { name: slicer.name })}
                  onBlur={(event) => {
                    const extensions = event.target.value.split(",").map((value) => value.trim()).filter(Boolean);
                    if (extensions.join() !== slicer.extensions.join()) update(list.map((s, i) => (i === index ? { ...s, extensions } : s)));
                  }}
                />
              </label>
            </div>
            {index > 0 && <button className="btn" onClick={() => update([slicer, ...list.filter((_, i) => i !== index)])}>{t("Make default")}</button>}
            <button className="btn ghost" onClick={() => update(list.filter((_, i) => i !== index))} aria-label={t("Remove {name}", { name: slicer.name })}>{t("Remove")}</button>
          </div>
        ))}
      </div>
      <div className="actions">
        <button className="btn" onClick={() => void add()} disabled={save.isPending}><Icon name="plus" />{t("Add slicer…")}</button>
        <button className="btn ghost" onClick={() => detect.mutate()} disabled={detect.isPending}><Icon name="search" />{t("Find installed slicers")}</button>
      </div>
    </>
  );
}
