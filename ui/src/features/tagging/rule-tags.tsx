import type { ModelDetailDto_Serialize, ModelMetaDto } from "../../ipc";
import { Icon } from "../../shared/icons";
import { t } from "../../shared/i18n";

type Props = { model: ModelDetailDto_Serialize; save: (change: Partial<ModelMetaDto>) => void };

const key = (tag: string) => tag.toLocaleLowerCase();

/**
 * Tags from tag rules, below the model's own tags. They are already searchable; "keep" makes one a normal tag (it then
 * stays when the rules change), "hide" removes it from this model only. Hidden ones can be brought back.
 */
export function RuleTags({ model, save }: Props) {
  const { meta } = model;
  const own = new Set(meta.tags.map(key));
  const hiddenTags = meta.hiddenTags ?? [];
  const hidden = new Set(hiddenTags.map(key));
  const shown = model.autoTags.filter((tag) => !own.has(key(tag)) && !hidden.has(key(tag)));
  const hiddenHere = model.autoTags.filter((tag) => hidden.has(key(tag)));
  if (shown.length === 0 && hiddenHere.length === 0) return null;
  return (
    <div className="ruletags">
      {shown.length > 0 && <span className="rt-label">{t("From tag rules")}</span>}
      {shown.map((tag) => (
        <span className="chip auto" key={tag}>
          {tag}
          <button className="rt-act" onClick={() => save({ tags: [...meta.tags, tag] })} aria-label={t("Keep {tag} as a tag", { tag })} title={t("Keep as a tag")}>
            <Icon name="check" />
          </button>
          <button className="rt-act" onClick={() => save({ hiddenTags: [...hiddenTags, tag] })} aria-label={t("Hide {tag} on this model", { tag })} title={t("Hide on this model")}>
            <Icon name="close" />
          </button>
        </span>
      ))}
      {hiddenHere.length > 0 && (
        <span className="rt-hidden">
          {t("Hidden:")}{" "}
          {hiddenHere.map((tag) => (
            <button
              key={tag}
              className="linkish"
              onClick={() => save({ hiddenTags: hiddenTags.filter((hiddenTag) => key(hiddenTag) !== key(tag)) })}
              aria-label={t("Show {tag} again", { tag })}
            >
              {tag}
            </button>
          ))}
        </span>
      )}
    </div>
  );
}
