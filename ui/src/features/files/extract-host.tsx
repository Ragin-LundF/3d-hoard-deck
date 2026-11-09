import { commands, unwrap } from "../../ipc";
import { HOME, useRoute } from "../../shared/route";
import { ExtractDialog } from "./extract-dialog";
import { closeExtract, useExtractRequest } from "./extract-store";

/** The extraction dialog, mounted once in the app shell so it keeps running while the user browses on. */
export function ExtractHost({ onMessage }: { onMessage: (message: string) => void }) {
  const request = useExtractRequest();
  const [route, navigate] = useRoute();
  if (!request) return null;
  const openFolder = (library: number, folder: string) => navigate({ ...HOME, library, folder });
  // The model page the dialog started on may be gone after the regroup (its archives became folders): then the
  // folder that held them is the place to be, not an empty page.
  const close = async () => {
    closeExtract();
    if (route.view !== "model") return;
    const still = await unwrap(commands.getModel(route.id)).catch(() => null);
    const archive = request.items[0]?.archive ?? "";
    if (!still) openFolder(request.library.id, archive.split("/").slice(0, -1).join("/"));
  };
  return (
    <ExtractDialog
      library={request.library}
      items={request.items}
      pending={request.pending}
      onClose={() => void close()}
      onMessage={onMessage}
      onOpenFolder={(library, folder) => {
        closeExtract();
        openFolder(library, folder);
      }}
    />
  );
}
