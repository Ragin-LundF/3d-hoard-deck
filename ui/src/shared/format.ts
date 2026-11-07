export function formatBytes(bytes: number | null): string {
  if (bytes === null) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

export function formatCount(count: number, one: string, many: string): string {
  return `${count.toLocaleString()} ${count === 1 ? one : many}`;
}

/** CSS class for a file type badge; `3mf` cannot start a class name. */
export function formatClass(format: string): string {
  return format === "3mf" ? "ft f3mf" : `ft ${format}`;
}

export function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

export function folderName(path: string): string {
  return path ? fileName(path) : "";
}
