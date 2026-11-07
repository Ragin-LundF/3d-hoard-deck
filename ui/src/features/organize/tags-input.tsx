import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { commands, unwrap } from "../../ipc";
import { Icon } from "../../shared/icons";
import { t } from "../../shared/i18n";

type Props = { library: number; tags: string[]; onChange: (tags: string[]) => void };

/** Tag chips with an input; Enter or comma adds, suggestions come from tags already in use. */
export function TagsInput({ library, tags, onChange }: Props) {
  const [text, setText] = useState("");
  const known = useQuery({ queryKey: ["tags", library], queryFn: () => unwrap(commands.listTags(library)) });
  const has = (tag: string) => tags.some((existing) => existing.toLowerCase() === tag.toLowerCase());
  const add = (value: string) => {
    const tag = value.trim().replace(/\s+/g, " ");
    if (tag && !has(tag)) onChange([...tags, tag]);
    setText("");
  };
  const listId = `tags-${library}`;
  return (
    <div className="tags">
      {tags.map((tag) => (
        <span className="tag" key={tag}>
          {tag}
          <button aria-label={t("Remove tag {tag}", { tag })} onClick={() => onChange(tags.filter((other) => other !== tag))}><Icon name="close" /></button>
        </span>
      ))}
      <input
        className="tagin"
        value={text}
        list={listId}
        placeholder={t("Add tag")}
        aria-label={t("Add tag")}
        onChange={(event) => (event.target.value.endsWith(",") ? add(event.target.value.slice(0, -1)) : setText(event.target.value))}
        onKeyDown={(event) => event.key === "Enter" && add(text)}
        onBlur={() => text && add(text)}
      />
      <datalist id={listId}>
        {known.data?.filter((tag) => !has(tag.name)).map((tag) => <option key={tag.name} value={tag.name} />)}
      </datalist>
    </div>
  );
}
