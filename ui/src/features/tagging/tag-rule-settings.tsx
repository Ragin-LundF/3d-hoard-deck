import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, describeError, IpcFailure, unwrap, type RuleKindDto, type TagRuleDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import { t } from "../../shared/i18n";
import { tagRuleDraft } from "../settings/drafts";
import { SaveBar } from "../settings/save-bar";

const KINDS: { kind: RuleKindDto; label: string; placeholder: string }[] = [
  { kind: "any", label: t("Any of these words"), placeholder: "presupported, supported, ready to slice" },
  { kind: "all", label: t("All of these words"), placeholder: "space, marine" },
  { kind: "regex", label: t("Regular expression"), placeholder: "\\b(28|32|75) ?mm\\b" },
  { kind: "folder", label: t("Name of the folder below"), placeholder: "Miniatures, MyMiniFactory" },
];

const EMPTY: TagRuleDto = { tag: "", kind: "any", pattern: "", unless: "", nameOnly: false, enabled: true };

const same = (a: TagRuleDto, b: TagRuleDto) => a.kind === b.kind && a.pattern.trim() === b.pattern.trim() && a.tag.trim() === b.tag.trim();

/**
 * Tag rules: tags from folder and file names, kept apart from the tags the user typed. Edits stay a draft until saved;
 * matches can be counted and a path tried before that. Saving only regroups from the index, never touches a library.
 */
export function TagRuleSettings({ onError }: { onError: (message: string) => void }) {
  const queryClient = useQueryClient();
  const saved = useQuery({ queryKey: ["tagRules"], queryFn: () => unwrap(commands.tagRules()) });
  const presets = useQuery({ queryKey: ["tagRulePresets"], queryFn: () => unwrap(commands.tagRulePresets()), staleTime: Infinity });
  const draft = tagRuleDraft.use();
  const [counts, setCounts] = useState<number[] | null>(null);
  const rules = draft ?? saved.data ?? [];
  const changed = draft !== null;

  const update = (next: TagRuleDto[]) => {
    tagRuleDraft.set(JSON.stringify(next) === JSON.stringify(saved.data ?? []) ? null : next);
    setCounts(null);
  };
  const change = (index: number, patch: Partial<TagRuleDto>) => update(rules.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)));

  const count = useMutation({
    mutationFn: () => unwrap(commands.countTagRules(rules)),
    onSuccess: setCounts,
    onError: (error) => onError(describeError(error)),
  });
  const save = useMutation({
    mutationFn: () => unwrap(commands.saveTagRules(rules)),
    onSuccess: () => {
      queryClient.setQueryData(["tagRules"], rules);
      tagRuleDraft.set(null);
      // The regroup runs in the background; tags and grids refresh when the "done" event arrives.
    },
    onError: (error) => onError(describeError(error)),
  });
  const missing = (presets.data ?? []).filter((preset) => !rules.some((rule) => same(rule, preset)));
  const [choosing, setChoosing] = useState(false);

  return (
    <>
      <div className="panel">
        {rules.length === 0 && (
          <div className="prow2">
            <div className="t">
              <b>{t("No tag rules yet")}</b>
              <span>{t("Add the suggested rules or your own. Rule tags are searchable like your tags, but never change them.")}</span>
            </div>
          </div>
        )}
        {rules.map((rule, index) => (
          <RuleRow
            key={index}
            rule={rule}
            count={counts?.[index]}
            onChange={(patch) => change(index, patch)}
            onRemove={() => update(rules.filter((_, i) => i !== index))}
          />
        ))}
      </div>
      <div className="panel"><RuleTester rules={rules} /></div>
      <div className="actions">
        <button className="btn" onClick={() => count.mutate()} disabled={rules.length === 0 || count.isPending}>
          <Icon name="search" />{count.isPending ? t("Counting…") : t("Count matches")}
        </button>
        <button className="btn" onClick={() => update([...rules, EMPTY])}><Icon name="plus" />{t("Add rule")}</button>
        {missing.length > 0 && (
          <button className={`btn ghost${choosing ? " on" : ""}`} onClick={() => setChoosing(!choosing)} aria-expanded={choosing}>
            <Icon name="plus" />{t("Suggested rules ({n})", { n: missing.length })}
          </button>
        )}
      </div>
      {choosing && missing.length > 0 && (
        <SuggestedRules
          rules={missing}
          onAdd={(picked) => {
            update([...rules, ...picked]);
            setChoosing(false);
          }}
          onClose={() => setChoosing(false)}
        />
      )}
      {changed && <SaveBar what={t("Tag rules")} saving={save.isPending} onSave={() => save.mutate()} onDiscard={() => update(saved.data ?? [])} />}
    </>
  );
}

type SuggestedProps = { rules: TagRuleDto[]; onAdd: (rules: TagRuleDto[]) => void; onClose: () => void };

/** The suggested rules to look at first: tick the ones that fit (or take all); they are added as unsaved rules. */
function SuggestedRules({ rules, onAdd, onClose }: SuggestedProps) {
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const kindLabel = (kind: RuleKindDto) => KINDS.find((entry) => entry.kind === kind)?.label ?? kind;
  return (
    <div className="panel" role="region" aria-label={t("Suggested rules")}>
      <div className="prow2">
        <div className="t">
          <b>{t("Suggested rules")}</b>
          <span>{t("Tick the rules that fit your library. They are added below as unsaved changes; Count matches shows what they would tag before you save.")}</span>
        </div>
      </div>
      {rules.map((rule, index) => (
        <label className="prow2 trule" key={`${rule.tag}-${rule.pattern}`}>
          <input
            type="checkbox"
            checked={picked.has(index)}
            onChange={(event) => {
              const next = new Set(picked);
              if (event.target.checked) next.add(index);
              else next.delete(index);
              setPicked(next);
            }}
          />
          <div className="t">
            <b>{rule.tag}</b>
            <span>{kindLabel(rule.kind)}: <span className="mono">{rule.pattern}</span></span>
            {(rule.unless || rule.nameOnly) && (
              <span className="tr-line sub">
                {rule.unless && <span className="pill">{t("unless {words}", { words: rule.unless })}</span>}
                {rule.nameOnly && <span className="pill">{t("own name only")}</span>}
              </span>
            )}
          </div>
        </label>
      ))}
      <div className="prow2">
        <div className="actions">
          <button className="btn primary" disabled={picked.size === 0} onClick={() => onAdd(rules.filter((_, index) => picked.has(index)))}>
            {t("Add selected ({n})", { n: picked.size })}
          </button>
          <button className="btn" onClick={() => onAdd(rules)}>{t("Add all")}</button>
          <button className="btn ghost" onClick={onClose}>{t("Cancel")}</button>
        </div>
      </div>
    </div>
  );
}

type RowProps = { rule: TagRuleDto; count: number | undefined; onChange: (patch: Partial<TagRuleDto>) => void; onRemove: () => void };

function RuleRow({ rule, count, onChange, onRemove }: RowProps) {
  const kind = KINDS.find((k) => k.kind === rule.kind) ?? KINDS[0]!;
  const captures = rule.kind === "regex" || rule.kind === "folder";
  const [options, setOptions] = useState(false);
  return (
    <div className={`prow2 trule${rule.enabled ? "" : " off"}`}>
      <input type="checkbox" checked={rule.enabled} onChange={(event) => onChange({ enabled: event.target.checked })} aria-label={t("Rule switched on")} />
      <div className="tr-fields">
        <div className="tr-line">
          <input
            className="tr-tag"
            value={rule.tag}
            placeholder={captures ? "{1}" : t("Tag")}
            aria-label={t("Tag")}
            onChange={(event) => onChange({ tag: event.target.value })}
          />
          <select className="sel" value={rule.kind} aria-label={t("How to match")} onChange={(event) => onChange({ kind: event.target.value as RuleKindDto })}>
            {KINDS.map((k) => <option key={k.kind} value={k.kind}>{k.label}</option>)}
          </select>
          <input
            className={`tr-pattern${rule.kind === "regex" ? " mono" : ""}`}
            value={rule.pattern}
            placeholder={kind.placeholder}
            aria-label={t("Words, expression or folders")}
            onChange={(event) => onChange({ pattern: event.target.value })}
          />
          {count !== undefined && <span className="pill">{t("{n} models", { n: count })}</span>}
          <button className={`btn ghost${options ? " on" : ""}`} onClick={() => setOptions(!options)} aria-expanded={options} aria-label={t("Options of rule {tag}", { tag: rule.tag })} title={t("Options")}><Icon name="settings" /></button>
          <button className="btn ghost" onClick={onRemove} aria-label={t("Remove rule {tag}", { tag: rule.tag })}><Icon name="close" /></button>
        </div>
        {!options && (rule.unless.trim() || rule.nameOnly) && (
          <div className="tr-line sub">
            {rule.unless.trim() && <span className="pill">{t("unless {words}", { words: rule.unless })}</span>}
            {rule.nameOnly && <span className="pill">{t("own name only")}</span>}
          </div>
        )}
        {options && <div className="tr-line sub">
          <label>
            {t("Unless")}
            <input value={rule.unless} aria-label={t("Unless these words")} onChange={(event) => onChange({ unless: event.target.value })} />
          </label>
          {rule.kind !== "folder" && (
            <label className="tr-check">
              <input type="checkbox" checked={rule.nameOnly} onChange={(event) => onChange({ nameOnly: event.target.checked })} />
              {t("Only the model's own name and files")}
            </label>
          )}
          {captures && <span className="hint">{rule.kind === "regex" ? t("{1} in the tag is the part in ( ).") : t("{1} in the tag is the folder's name.")}</span>}
        </div>}
      </div>
    </div>
  );
}

/** Type a path, see which tags the current (unsaved) rules give it. */
function RuleTester({ rules }: { rules: TagRuleDto[] }) {
  const [path, setPath] = useState("");
  const [result, setResult] = useState<{ tags: string[]; error?: string } | null>(null);
  useEffect(() => {
    if (!path.trim()) {
      setResult(null);
      return;
    }
    const timer = setTimeout(() => {
      commands.testTagRules(rules, path).then((answer) => {
        setResult(answer.status === "ok" ? { tags: answer.data } : { tags: [], error: describeError(new IpcFailure(answer.error)) });
      }, () => setResult(null));
    }, 250);
    return () => clearTimeout(timer);
  }, [path, rules]);
  return (
    <div className="prow2 stack">
      <div className="t"><b>{t("Try a path")}</b><span>{t("For example a model folder inside a library, starting below the library folder.")}</span></div>
      <input className="tr-test" value={path} placeholder="Miniatures/Loot Studios/Fantasy/resin/32mm/OniMage_32mm_UnSupported" aria-label={t("Try a path")} onChange={(event) => setPath(event.target.value)} />
      {result && (
        <div className="tr-result" aria-live="polite">
          {result.error ? <span className="err-text">{result.error}</span>
            : result.tags.length === 0 ? <span className="hint">{t("No tags for this path.")}</span>
            : result.tags.map((tag) => <span className="chip auto" key={tag}>{tag}</span>)}
        </div>
      )}
    </div>
  );
}
