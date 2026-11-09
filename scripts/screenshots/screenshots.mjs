// README screenshots: the real UI (Vite dev server) and the real index of the demo library; the Tauri backend is
// replaced by small handlers that read the index. Started by run.sh.
//
//   node screenshots.mjs covers <index.sqlite> <library> <cli> <work dir>   renders cover.png for every model
//   node screenshots.mjs shots  <index.sqlite> <library> <cli> <out dir>    writes the screenshots
import { chromium } from "playwright-core";
import { DatabaseSync } from "node:sqlite";
import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";

const [, , mode, dbPath, libRoot, cli, outDir] = process.argv;
const port = process.env.PORT ?? "5179";
const db = new DatabaseSync(dbPath, { readOnly: true });

// Explicit columns: the fingerprint column holds 64-bit numbers JavaScript cannot represent.
const SUMMARY =
  "SELECT m.id, m.library_id, m.path, m.folder, m.kind, m.title, m.released, m.cover, m.collection, m.marked_by_user, " +
  "m.formats, m.file_count, m.total_size, COALESCE(mm.favorite,0) AS favorite, COALESCE(mm.printed,0) AS printed FROM models m " +
  "LEFT JOIN model_meta mm ON mm.library_id=m.library_id AND mm.path=m.path";
const summary = (r) => ({
  id: r.id, library: r.library_id, path: r.path, folder: r.folder, kind: r.kind, title: r.title, released: r.released || null,
  cover: r.cover, coverSource: "own", fallbackCover: null, rendered: null, collection: r.collection,
  markedByUser: !!r.marked_by_user, favorite: !!r.favorite, printed: !!r.printed,
  formats: r.formats ? r.formats.split(",") : [], fileCount: r.file_count, totalSize: r.total_size,
});
const metaOf = (r) => {
  const mm = db.prepare("SELECT * FROM model_meta WHERE library_id=? AND path=?").get(r.library_id, r.path);
  return { displayName: mm?.display_name ?? null, tags: mm ? JSON.parse(mm.tags) : [], favorite: !!mm?.favorite, printed: !!mm?.printed, notes: mm?.notes ?? "", hiddenTags: [] };
};
const folder = (r) => ({ path: r.path, name: r.display_name, released: r.released, directModels: r.direct_models, totalModels: r.total_models, cover: r.cover, isCollection: !!r.is_collection });
const tags = () =>
  db.prepare("SELECT value AS name, COUNT(*) AS count FROM model_meta, json_each(model_meta.tags) GROUP BY lower(value) ORDER BY count DESC, name").all()
    .map((r, i) => ({ ...r, group: null, pinned: i < 2, recent: 1700000000 + i }));

const handlers = {
  index_location: () => "/Users/you/.3dhoarddeck",
  list_slicers: () => [
    { name: "Bambu Studio", program: "/Applications/BambuStudio.app", args: ["{files}"], extensions: ["stl", "3mf", "obj", "step", "stp"] },
    { name: "Lychee Slicer", program: "/Applications/LycheeSlicer.app", args: ["{files}"], extensions: ["stl", "obj", "lys"] },
  ],
  list_libraries: () => db.prepare("SELECT id, name FROM libraries").all().map((r) => ({ id: r.id, name: r.name, root: "/Volumes/nas/3D Print", access: "managed", metadata: "indexOnly" })),
  folder_children: ({ library, folder: parent }) => db.prepare("SELECT * FROM folders WHERE library_id=? AND parent=? ORDER BY display_name").all(library ?? 1, parent ?? "").map(folder),
  browse_folder: ({ request: q }) => {
    const rows = q.recursive
      ? db.prepare(`${SUMMARY} WHERE m.library_id=? AND (?='' OR m.folder=? OR m.folder LIKE ?||'/%') ORDER BY m.title`).all(q.library, q.folder, q.folder, q.folder)
      : db.prepare(`${SUMMARY} WHERE m.library_id=? AND m.folder=? ORDER BY m.title`).all(q.library, q.folder);
    return { folders: q.recursive ? [] : handlers.folder_children({ library: q.library, folder: q.folder }), images: [], models: rows.map(summary), next: null };
  },
  search_models: ({ request: q }) => {
    const text = q.text.trim();
    const rows = text.startsWith("tag:")
      ? db.prepare(`${SUMMARY} WHERE mm.tags LIKE ? ORDER BY m.title`).all(`%"${text.slice(4).replace(/"/g, "")}"%`)
      : db.prepare(`${SUMMARY} WHERE m.title LIKE ? ORDER BY m.title`).all(`%${text.split(" ")[0]}%`);
    return { models: rows.map(summary), total: rows.length };
  },
  get_model: ({ id }) => {
    const r = db.prepare(`${SUMMARY} WHERE m.id=?`).get(id);
    const files = db.prepare("SELECT * FROM model_files WHERE model_id=? ORDER BY path").all(id).map((f) => ({ path: f.path, kind: f.kind, size: f.size, variant: f.variant }));
    return r && { summary: summary(r), originalTitle: r.title, meta: metaOf(r), autoTags: [], files };
  },
  list_collections: () => [{ id: 1, name: "Chess set", query: null, count: 2 }, { id: 2, name: "Resin favorites", query: "tag:resin", count: null }],
  list_tags: tags,
  tag_facets: () => tags().map(({ name, count }) => ({ name, count })),
  list_tag_groups: () => [],
  propose_presets: () => [],
  file_journal: () => [],
  folder_rules: () => [],
  tag_rules: () => [],
  pending_extractions: () => [],
  check_for_update: () => null,
  "plugin:app|version": () => "1.1.0",
  ai_settings: () => ({ settings: { provider: "off", baseUrl: "", model: "", sendImage: true, sendPath: true, maxTags: 3, timeoutSeconds: 120, prompt: "" }, hasKey: false, keyFromEnvironment: false, remote: false, defaultUrl: "http://localhost:1234/v1" }),
  tag_suggestions: () => [],
  pending_suggestions: () => 0,
};

const browser = await chromium.launch({
  ...(process.env.CHROME ? { executablePath: process.env.CHROME } : {}),
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"],
});
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: mode === "covers" ? 1 : 2, locale: "en-US" });
page.on("pageerror", (error) => console.log("PAGE ERROR", error.message));
await page.exposeFunction("__hoardInvoke", (cmd, args) => (handlers[cmd] ? handlers[cmd](args ?? {}) : null));
// Files of the demo library, thumbnails and 3MF plates (decoded by the real Rust reader through the CLI).
await page.route("**/__demo/**", (route) => {
  const decoded = decodeURIComponent(route.request().url().split("/__demo/")[1]);
  const plates = decoded.startsWith("plates/");
  const rel = decoded.split("/").slice(plates || decoded.startsWith("thumb/") ? 2 : 1).join("/");
  try {
    const body = plates ? execFileSync(cli, ["plates", `${libRoot}/${rel}`, "--binary"]) : readFileSync(`${libRoot}/${rel}`);
    route.fulfill({ body, headers: { "access-control-allow-origin": "*" } });
  } catch {
    route.fulfill({ status: 404 });
  }
});
await page.addInitScript((origin) => {
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    invoke: (cmd, args) => window.__hoardInvoke(cmd, args),
    transformCallback: () => 1,
    convertFileSrc: (path) => `${origin}/__demo/${encodeURIComponent(path)}`,
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
}, `http://localhost:${port}`);

const go = async (hash, theme) => {
  await page.goto(`http://localhost:${port}/${hash}`);
  await page.evaluate((value) => localStorage.setItem("hoarddeck.theme", value), theme);
  await page.reload();
  await page.waitForTimeout(900);
};
const models = db.prepare("SELECT id, path, kind, title FROM models").all();
const byTitle = (title) => models.find((model) => model.title === title)?.id;
const to3d = async () => {
  const button = page.getByRole("button", { name: "3D", exact: true });
  if (await button.count()) await button.click();
  await page.waitForSelector(".vinfo", { timeout: 20000 });
};

if (mode === "covers") {
  for (const model of models) {
    await go(`#view=model&id=${model.id}`, "light");
    await to3d().catch(() => console.log("no 3D view for", model.title));
    const plateOne = page.getByRole("button", { name: "1", exact: true });
    if (await plateOne.count()) await plateOne.click();
    await page.addStyleTag({ content: ".vtools,.vinfo,.vnav,.stage-switch{display:none!important}" });
    await page.waitForTimeout(400);
    const dir = model.kind === "file" ? model.path.replace(/\/[^/]*$/, "") : model.path;
    const name = model.kind === "file" ? model.path.replace(/^.*\//, "").replace(/\.[^.]*$/, "") : "cover";
    const box = await page.locator(".stage").boundingBox();
    const side = Math.min(box.width, (box.height * 4) / 3);
    const clip = { x: box.x + (box.width - side) / 2, y: box.y + (box.height - (side * 3) / 4) / 2, width: side, height: (side * 3) / 4 };
    writeFileSync(`${libRoot}/${dir}/${name}.png`, await page.screenshot({ clip }));
  }
  console.log("covers rendered");
} else {
  const shot = async (name, hash, action, theme = "dark") => {
    await go(hash, theme);
    if (action) await action();
    await page.waitForTimeout(700);
    const png = await page.screenshot();
    if (name) writeFileSync(`${outDir}/${name}.png`, png);
    return png;
  };
  await shot("library", "#view=library&lib=1&folder=Miniatures%2FForge+Masters");
  const allDark = await shot("all-models", "#view=library&lib=1&all=1");
  await shot("search", "#view=library&lib=1&q=gear");
  await shot("model-3d", `#view=model&id=${byTitle("Twisted Vase")}`, to3d);
  await shot("plates", `#view=model&id=${byTitle("Gearbox Kit")}`, to3d);
  await shot("model-tags", `#view=model&id=${byTitle("Twisted Vase")}`, () => page.getByRole("tab", { name: /Tags/ }).click());
  await shot("settings", "#view=settings&tab=library&lib=1&sub=access");
  const allLight = await shot(null, "#view=library&lib=1&all=1", null, "light");
  writeFileSync(`${outDir}/light-dark.png`, await diagonal(allDark, allLight));
  console.log("screenshots written");
}
await browser.close();

/** One picture: dark on the left, light on the right, split along a diagonal with a thin accent line. */
async function diagonal(dark, light) {
  const split = await browser.newPage();
  const data = await split.evaluate(async ([darkUrl, lightUrl]) => {
    const load = (src) => new Promise((resolve) => { const image = new Image(); image.onload = () => resolve(image); image.src = src; });
    const [a, b] = await Promise.all([load(darkUrl), load(lightUrl)]);
    const canvas = Object.assign(document.createElement("canvas"), { width: a.width, height: a.height });
    const context = canvas.getContext("2d");
    const [top, bottom] = [a.width * 0.62, a.width * 0.38];
    context.drawImage(a, 0, 0);
    context.save();
    context.beginPath();
    context.moveTo(top, 0);
    context.lineTo(a.width, 0);
    context.lineTo(a.width, a.height);
    context.lineTo(bottom, a.height);
    context.closePath();
    context.clip();
    context.drawImage(b, 0, 0);
    context.restore();
    context.strokeStyle = "#FF7A45";
    context.lineWidth = 6;
    context.beginPath();
    context.moveTo(top, 0);
    context.lineTo(bottom, a.height);
    context.stroke();
    return canvas.toDataURL("image/png");
  }, [`data:image/png;base64,${dark.toString("base64")}`, `data:image/png;base64,${light.toString("base64")}`]);
  await split.close();
  return Buffer.from(data.split(",")[1], "base64");
}
