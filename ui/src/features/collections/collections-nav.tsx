import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { commands, describeError, unwrap } from "../../ipc";
import { menuProps, type MenuItem } from "../../shared/context-menu";
import { Icon } from "../../shared/icons";
import { NameInput } from "./name-input";
import { useCollections, useCreateCollection } from "./queries";
import { t, formatNumber } from "../../shared/i18n";

type Props = {
  current: number | null;
  onOpen: (id: number) => void;
  /** After the shown collection was deleted from its menu. */
  onDeleted: () => void;
  onError: (message: string) => void;
};

/** Sidebar list of collections with "New collection". */
export function CollectionsNav({ current, onOpen, onDeleted, onError }: Props) {
  const collections = useCollections();
  const queryClient = useQueryClient();
  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState<number | null>(null);
  const create = useCreateCollection(onError, onOpen);
  const fail = (error: unknown) => onError(describeError(error));
  const refresh = () => queryClient.invalidateQueries();
  /** Open, rename, delete: the same as the buttons on the collection's own page. */
  const menu = (id: number): MenuItem[] => [
    { label: t("Open"), onSelect: () => onOpen(id) },
    { label: t("Rename…"), onSelect: () => setRenaming(id) },
    "separator",
    {
      label: t("Delete collection"),
      danger: true,
      onSelect: () => void unwrap(commands.deleteCollection(id)).then(async () => {
        await refresh();
        if (current === id) onDeleted();
      }).catch(fail),
    },
  ];
  return (
    <div className="sec">
      <h3 className="with-action">
        {t("Collections")}
        <button className="tiny" onClick={() => setCreating(true)} aria-label={t("New collection")} title={t("New collection")}><Icon name="plus" /></button>
      </h3>
      {collections.data?.map((collection) => renaming === collection.id ? (
        <NameInput
          key={collection.id}
          placeholder={t("Collection name")}
          initial={collection.name}
          onSubmit={(name) => {
            setRenaming(null);
            void unwrap(commands.renameCollection(collection.id, name)).then(refresh).catch(fail);
          }}
          onCancel={() => setRenaming(null)}
        />
      ) : (
        <button key={collection.id} className={`nav${current === collection.id ? " on" : ""}`} onClick={() => onOpen(collection.id)} {...menuProps(() => menu(collection.id))}>
          <Icon name={collection.query ? "search" : "box"} />
          <span className="lbl">{collection.name}</span>
          {collection.count !== null && <span className="n">{formatNumber(collection.count)}</span>}
        </button>
      ))}
      {creating && (
        <NameInput
          placeholder={t("Collection name")}
          onSubmit={(name) => {
            setCreating(false);
            create.mutate({ name });
          }}
          onCancel={() => setCreating(false)}
        />
      )}
      {!creating && collections.data?.length === 0 && <p className="hint">{t("Group models you want to print together.")}</p>}
    </div>
  );
}
