import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { commands, describeError, unwrap, type ModelSummaryDto_Serialize } from "../../ipc";
import { Icon } from "../../shared/icons";
import { ModelCard } from "../library/cards";
import { NameInput } from "./name-input";
import { useCollections } from "./queries";
import { t, plural } from "../../shared/i18n";

/** shortcut: collections show up to this many models at once; page them when collections get that big. */
const LIMIT = 500;

type Props = { id: number; onOpenModel: (id: number) => void; onDeleted: () => void; onError: (message: string) => void };

/** A collection: hand-made ones can be reordered by dragging and members removed; smart ones show their search. */
export function CollectionView({ id, onOpenModel, onDeleted, onError }: Props) {
  const queryClient = useQueryClient();
  const collection = useCollections().data?.find((candidate) => candidate.id === id);
  const models = useQuery({ queryKey: ["collection", id], queryFn: () => unwrap(commands.collectionModels(id, 0, LIMIT)) });
  const [renaming, setRenaming] = useState(false);
  const [dragged, setDragged] = useState<number | null>(null);
  const refresh = () => queryClient.invalidateQueries({ predicate: (query) => ["collection", "collections"].includes(String(query.queryKey[0])) });
  const fail = (error: unknown) => onError(describeError(error));
  const rename = useMutation({ mutationFn: (name: string) => unwrap(commands.renameCollection(id, name)), onSuccess: refresh, onError: fail });
  const remove = useMutation({ mutationFn: (model: number) => unwrap(commands.removeFromCollection(id, [model])), onSuccess: refresh, onError: fail });
  const move = useMutation({ mutationFn: ({ model, index }: { model: number; index: number }) => unwrap(commands.moveInCollection(id, model, index)), onSuccess: refresh, onError: fail });
  const remove_collection = useMutation({
    mutationFn: () => unwrap(commands.deleteCollection(id)),
    onSuccess: async () => {
      await refresh();
      onDeleted();
    },
    onError: fail,
  });
  if (!collection) return <div className="empty">{t("This collection no longer exists.")}</div>;
  const handMade = collection.query === null;
  const list: ModelSummaryDto_Serialize[] = models.data?.models ?? [];

  return (
    <>
      <div className="libhead">
        <div className="libtitle">
          {renaming ? (
            <NameInput placeholder={t("Collection name")} initial={collection.name} onSubmit={(name) => { setRenaming(false); rename.mutate(name); }} onCancel={() => setRenaming(false)} />
          ) : (
            <h1>{collection.name}</h1>
          )}
          <span className="crumbs">{models.data ? plural(models.data.total, "{count} model", "{count} models") : ""}</span>
          <span className="spacer" />
          <button className="btn ghost" onClick={() => setRenaming(true)}><Icon name="pen" />{t("Rename")}</button>
          <button className="btn ghost" onClick={() => remove_collection.mutate()} title={t("Deletes the collection; its models stay in the library")}>{t("Delete collection")}</button>
        </div>
        <div className="crumbs">
          {handMade ? t("Drag cards to change the order. Add models from their page.") : <>{t("Smart collection: shows everything matching")} <span className="code">{collection.query}</span></>}
        </div>
      </div>
      {list.length === 0 ? (
        <div className="empty">{handMade ? t("No models yet. Open a model and use “Add to collection”.") : t("Nothing matches this search right now.")}</div>
      ) : (
        <div className="gridwrap">
          <div className="grow static">
            {list.map((model, index) => (
              <div
                key={model.id}
                className={`member${dragged === model.id ? " dragging" : ""}`}
                draggable={handMade}
                onDragStart={() => setDragged(model.id)}
                onDragEnd={() => setDragged(null)}
                onDragOver={(event) => handMade && dragged !== null && event.preventDefault()}
                onDrop={() => {
                  if (dragged !== null && dragged !== model.id) move.mutate({ model: dragged, index });
                  setDragged(null);
                }}
              >
                <ModelCard model={model} onOpen={() => onOpenModel(model.id)} />
                {handMade && (
                  <button className="member-remove" onClick={() => remove.mutate(model.id)} aria-label={t("Remove {name} from the collection", { name: model.title })} title={t("Remove from collection")}>
                    <Icon name="close" />
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
