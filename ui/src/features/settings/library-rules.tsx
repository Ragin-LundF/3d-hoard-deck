import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";

type Props = { library: number; onError: (message: string) => void };

/** Proposed collection presets and the folder rules in use, each removable. Changes regroup at once. */
export function LibraryRules({ library, onError }: Props) {
  const queryClient = useQueryClient();
  const proposals = useQuery({ queryKey: ["presets", library], queryFn: () => unwrap(commands.proposePresets(library)) });
  const rules = useQuery({ queryKey: ["rules", library], queryFn: () => unwrap(commands.folderRules(library)) });
  const refresh = () => queryClient.invalidateQueries();
  const accept = useMutation({ mutationFn: (id: string) => unwrap(commands.acceptPreset(library, id)), onSuccess: refresh, onError: (e) => onError(describeError(e)) });
  const remove = useMutation({ mutationFn: (pattern: string) => unwrap(commands.removeFolderRule(library, pattern)), onSuccess: refresh, onError: (e) => onError(describeError(e)) });
  const busy = accept.isPending || remove.isPending;

  return (
    <>
      {(proposals.data?.length ?? 0) > 0 && (
        <div className="prow2 stack">
          <div className="t">
            <b>Suggested collections</b>
            <span>These folder layouts were found in this library. Using one groups every matching folder as a collection; models inside stay separate.</span>
          </div>
          {proposals.data?.map((proposal) => (
            <div className="rule" key={proposal.id}>
              <div className="t">
                <b>{proposal.name} · {proposal.matches.toLocaleString()} {proposal.matches === 1 ? "folder" : "folders"}</b>
                <span>{proposal.samples.join(", ")}{proposal.matches > proposal.samples.length ? ", …" : ""}</span>
                <span className="code">{proposal.pattern}</span>
              </div>
              <button className="btn" disabled={busy} onClick={() => accept.mutate(proposal.id)}>Use</button>
            </div>
          ))}
        </div>
      )}
      {(rules.data?.length ?? 0) > 0 && (
        <div className="prow2 stack">
          <div className="t"><b>Folder rules</b><span>Marks from right-clicking a folder and accepted suggestions.</span></div>
          {rules.data?.map((rule) => (
            <div className="rule" key={rule.pattern}>
              <div className="t">
                <b>{rule.role === "model" ? "One model" : "Collection"}</b>
                <span className="code">{rule.pattern}</span>
              </div>
              <button className="btn ghost" disabled={busy} onClick={() => remove.mutate(rule.pattern)}>Remove</button>
            </div>
          ))}
        </div>
      )}
    </>
  );
}
