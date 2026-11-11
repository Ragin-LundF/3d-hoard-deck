import { Fragment } from "react";
import { menuProps, openContextMenu, type MenuItem } from "../../shared/context-menu";
import { t } from "../../shared/i18n";

type Props = {
  libraryName: string;
  folder: string;
  onOpen: (folder: string) => void;
  /** The menu of a folder; `""` is the library itself. */
  menu: (folder: string) => MenuItem[];
};

/** Library name, then one button per folder level, and "⋯" with the current folder's menu. Names on disk are shown as
 * they are. */
export function Breadcrumbs({ libraryName, folder, onOpen, menu }: Props) {
  const parts = folder ? folder.split("/") : [];
  return (
    <nav className="bc" aria-label={t("Folder path")}>
      <button onClick={() => onOpen("")} {...menuProps(() => menu(""))}>{libraryName}</button>
      {parts.map((part, index) => {
        const path = parts.slice(0, index + 1).join("/");
        return (
          <Fragment key={index}>
            <span className="s">/</span>
            <button onClick={() => onOpen(path)} {...menuProps(() => menu(path))}>{part}</button>
          </Fragment>
        );
      })}
      <button className="bc-more" aria-haspopup="menu" aria-label={t("More for this folder")} title={t("More for this folder")}
        onClick={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          openContextMenu({ clientX: box.left, clientY: box.bottom + 4 }, menu(folder));
        }}>⋯</button>
    </nav>
  );
}
