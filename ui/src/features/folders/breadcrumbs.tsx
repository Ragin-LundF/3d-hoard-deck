import { Fragment } from "react";

/** Library name, then one button per folder level. Names on disk are shown as they are. */
export function Breadcrumbs({ libraryName, folder, onOpen }: { libraryName: string; folder: string; onOpen: (folder: string) => void }) {
  const parts = folder ? folder.split("/") : [];
  return (
    <nav className="bc" aria-label="Folder path">
      <button onClick={() => onOpen("")}>{libraryName}</button>
      {parts.map((part, index) => (
        <Fragment key={index}>
          <span className="s">/</span>
          <button onClick={() => onOpen(parts.slice(0, index + 1).join("/"))}>{part}</button>
        </Fragment>
      ))}
    </nav>
  );
}
