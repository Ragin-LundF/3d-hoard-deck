import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { commands, describeError, unwrap } from "../../ipc";
import { Icon } from "../../shared/icons";
import { NameInput } from "./name-input";
import { useCollections, useCreateCollection } from "./queries";

/** Menu button on the model page: add the model to a hand-made collection, or to a new one. */
export function AddToCollection({ model, onMessage }: { model: number; onMessage: (message: string) => void }) {
  const queryClient = useQueryClient();
  const collections = useCollections();
  const [open, setOpen] = useState(false);
  const [creating, setCreating] = useState(false);
  const add = useMutation({
    mutationFn: (id: number) => unwrap(commands.addToCollection(id, [model])),
    onSuccess: (_, id) => {
      void queryClient.invalidateQueries({ predicate: (query) => ["collection", "collections"].includes(String(query.queryKey[0])) });
      onMessage(`Added to ${collections.data?.find((c) => c.id === id)?.name ?? "the collection"}`);
    },
    onError: (error) => onMessage(describeError(error)),
  });
  const create = useCreateCollection(onMessage, (id) => add.mutate(id));
  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [open]);
  const handMade = collections.data?.filter((collection) => collection.query === null) ?? [];

  return (
    <div className="split">
      <button className="btn" aria-haspopup="menu" aria-expanded={open} onClick={(event) => { event.stopPropagation(); setOpen(!open); }}>
        <Icon name="box" />Add to collection
      </button>
      {open && (
        <div className="menu split-menu" role="menu" onClick={(event) => event.stopPropagation()}>
          {handMade.map((collection) => (
            <button key={collection.id} role="menuitem" onClick={() => { setOpen(false); add.mutate(collection.id); }}>
              {collection.name}<span className="k">{collection.count ?? 0}</span>
            </button>
          ))}
          {handMade.length > 0 && <hr />}
          {creating ? (
            <NameInput placeholder="New collection" onSubmit={(name) => { setCreating(false); setOpen(false); create.mutate({ name, query: null }); }} onCancel={() => setCreating(false)} />
          ) : (
            <button role="menuitem" onClick={() => setCreating(true)}><Icon name="plus" />New collection…</button>
          )}
        </div>
      )}
    </div>
  );
}
