import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";
import { t, plural } from "../../shared/i18n";

type Props = { library: number; onError: (message: string) => void };

/** Proposed collection presets and the folder rules in use, each removable. Changes regroup at once. */
export function LibraryRules({ library, onError }: Props) {
  const queryClient = useQueryClient();
  const proposals = useQuery({ queryKey: ["presets", library], queryFn: () => unwrap(commands.proposePresets(library)) });
  const rules = useQuery({ queryKey: ["rules", library], queryFn: () => unwrap(commands.folderRules(library)) });
  const refresh = () => queryClient.invalidateQueries();
  const accept = useMutation({ mutationFn: (id: string) => unwrap(commands.acceptPreset(library, id)), onSuccess: refresh, onError: (e) => onError(describeError(e)) });
  const remove = useMutation({ mutationFn: (pattern: string) => unwrap(commands.removeFolderRule(library, pattern)), onSuccess: refresh, onError: (e) => onError(describeError(e)) });
  const parentCovers = useQuery({ queryKey: ["parentCovers", library], queryFn: () => unwrap(commands.parentCovers(library)) });
  const setParentCovers = useMutation({
    mutationFn: (on: boolean) => unwrap(commands.setParentCovers(library, on)),
    onSuccess: refresh,
    onError: (e) => onError(describeError(e)),
  });
  const busy = accept.isPending || remove.isPending;

  return (
    <>
      <label className="prow2">
        <div className="t">
          <b>{t("Pictures from the folder above")}</b>
          <span>{t("Models without a picture of their own show one from the folder above (two levels if the first has none, never the library folder itself). They get a “from folder” mark. 3MF files show the preview stored inside them.")}</span>
        </div>
        <input type="checkbox" checked={parentCovers.data ?? true} disabled={setParentCovers.isPending} onChange={(event) => setParentCovers.mutate(event.target.checked)} />
      </label>
      {(proposals.data?.length ?? 0) > 0 && (
        <div className="prow2 stack">
          <div className="t">
            <b>{t("Suggested collections")}</b>
            <span>{t("These folder layouts were found in this library. Using one groups every matching folder as a collection; models inside stay separate.")}</span>
          </div>
          {proposals.data?.map((proposal) => (
            <div className="rule" key={proposal.id}>
              <div className="t">
                <b>{proposal.name} · {plural(proposal.matches, "{count} folder", "{count} folders")}</b>
                <span>{proposal.samples.join(", ")}{proposal.matches > proposal.samples.length ? ", …" : ""}</span>
                <span className="code">{proposal.pattern}</span>
              </div>
              <button className="btn" disabled={busy} onClick={() => accept.mutate(proposal.id)}>{t("Use")}</button>
            </div>
          ))}
        </div>
      )}
      {(rules.data?.length ?? 0) > 0 && (
        <div className="prow2 stack">
          <div className="t"><b>{t("Folder rules")}</b><span>{t("Marks from right-clicking a folder and accepted suggestions.")}</span></div>
          {rules.data?.map((rule) => (
            <div className="rule" key={rule.pattern}>
              <div className="t">
                <b>{rule.role === "model" ? t("One model") : t("Collection")}</b>
                <span className="code">{rule.pattern}</span>
              </div>
              <button className="btn ghost" disabled={busy} onClick={() => remove.mutate(rule.pattern)}>{t("Remove")}</button>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
