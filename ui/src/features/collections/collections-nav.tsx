import { useState } from "react";
import { Icon } from "../../shared/icons";
import { NameInput } from "./name-input";
import { useCollections, useCreateCollection } from "./queries";

type Props = { current: number | null; onOpen: (id: number) => void; onError: (message: string) => void };

/** Sidebar list of collections with "New collection". */
export function CollectionsNav({ current, onOpen, onError }: Props) {
  const collections = useCollections();
  const [creating, setCreating] = useState(false);
  const create = useCreateCollection(onError, onOpen);
  return (
    <div className="sec">
      <h3 className="with-action">
        Collections
        <button className="tiny" onClick={() => setCreating(true)} aria-label="New collection" title="New collection"><Icon name="plus" /></button>
      </h3>
      {collections.data?.map((collection) => (
        <button key={collection.id} className={`nav${current === collection.id ? " on" : ""}`} onClick={() => onOpen(collection.id)}>
          <Icon name={collection.query ? "search" : "box"} />
          <span className="lbl">{collection.name}</span>
          {collection.count !== null && <span className="n">{collection.count.toLocaleString()}</span>}
        </button>
      ))}
      {creating && (
        <NameInput
          placeholder="Collection name"
          onSubmit={(name) => {
            setCreating(false);
            create.mutate({ name, query: null });
          }}
          onCancel={() => setCreating(false)}
        />
      )}
      {!creating && collections.data?.length === 0 && <p className="hint">Group models you want to print together.</p>}
    </div>
  );
}
