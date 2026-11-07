import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type MouseEvent } from "react";
import { commands, describeError, unwrap, type FolderRoleDto } from "../../ipc";
import { t } from "../../shared/i18n";

export type MenuTarget = { library: number; folder: string; isCollection: boolean; x: number; y: number };

/** Right-click on a folder: the user decides whether it is one model, a collection, or detected automatically. */
export function useFolderMenu() {
  const [target, setTarget] = useState<MenuTarget | null>(null);
  const open = (library: number, folder: string, isCollection: boolean) => (event: MouseEvent) => {
    event.preventDefault();
    setTarget({ library, folder, isCollection, x: event.clientX, y: event.clientY });
  };
  return { target, open, close: () => setTarget(null) };
}

export function FolderMenu({ target, onClose, onError }: { target: MenuTarget; onClose: () => void; onError: (message: string) => void }) {
  const queryClient = useQueryClient();
  const setRole = useMutation({
    mutationFn: (role: FolderRoleDto | null) => unwrap(commands.setFolderRole(target.library, target.folder, role)),
    onSuccess: () => queryClient.invalidateQueries(),
    onError: (error) => onError(describeError(error)),
  });
  useEffect(() => {
    const close = () => onClose();
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    window.addEventListener("click", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);
  const choose = (role: FolderRoleDto | null) => () => {
    setRole.mutate(role);
    onClose();
  };
  return (
    <div className="menu" role="menu" style={{ left: target.x, top: target.y }} onClick={(event) => event.stopPropagation()}>
      <button role="menuitem" onClick={choose("model")}>{t("Treat as one model")}</button>
      <button role="menuitemcheckbox" aria-checked={target.isCollection} onClick={choose("collection")}>{t("Treat as collection")}</button>
      <hr />
      <button role="menuitem" onClick={choose(null)}>{t("Detect automatically")}</button>
      <p className="menu-hint">{t("Rules for whole patterns (suggested collections) are listed in Settings › Libraries.")}</p>
    </div>
  );
}
