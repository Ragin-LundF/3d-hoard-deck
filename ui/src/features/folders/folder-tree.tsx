import { useQuery } from "@tanstack/react-query";
import { useState, type MouseEvent } from "react";
import { commands, unwrap, type FolderDto_Serialize } from "../../ipc";
import { Icon } from "../../shared/icons";

type Props = {
  library: number;
  current: string | null;
  onOpen: (folder: string) => void;
  onMenu: (folder: FolderDto_Serialize) => (event: MouseEvent) => void;
};

function useChildren(library: number, folder: string, enabled: boolean) {
  return useQuery({
    queryKey: ["folders", library, folder],
    enabled,
    queryFn: () => unwrap(commands.folderChildren(library, folder)),
  });
}

/** Sidebar tree; each level loads only when expanded. */
export function FolderTree(props: Props) {
  const { data } = useChildren(props.library, "", true);
  return (
    <div role="tree" aria-label="Folders">
      {data?.map((folder) => <TreeItem key={folder.path} folder={folder} depth={0} {...props} />)}
    </div>
  );
}

function TreeItem({ folder, depth, ...props }: Props & { folder: FolderDto_Serialize; depth: number }) {
  const [open, setOpen] = useState(false);
  const hasChildren = folder.totalModels > folder.directModels;
  const { data } = useChildren(props.library, folder.path, open && hasChildren);
  return (
    <div role="treeitem" aria-expanded={hasChildren ? open : undefined} aria-selected={props.current === folder.path}>
      <button
        className={`nav${props.current === folder.path ? " on" : ""}`}
        style={{ paddingLeft: 8 + depth * 14 }}
        onClick={() => props.onOpen(folder.path)}
        onContextMenu={props.onMenu(folder)}
        title={folder.path}
      >
        <span
          className={`chev${open ? " open" : ""}${hasChildren ? "" : " none"}`}
          role="button"
          aria-label={open ? "Collapse" : "Expand"}
          onClick={(event) => {
            event.stopPropagation();
            setOpen(!open);
          }}
        >
          <Icon name="chevron" />
        </span>
        <Icon name={folder.isCollection ? "box" : "folder"} />
        <span className="lbl">{folder.name}</span>
        <span className="n">{folder.totalModels.toLocaleString()}</span>
      </button>
      {open && data?.map((child) => <TreeItem key={child.path} folder={child} depth={depth + 1} {...props} />)}
    </div>
  );
}
