/** A starting name for a picked path, which the user can change: "/Applications/Editor.app"
 * gives "Editor", "C:\\Apps\\tool.exe" gives "tool". Cut to the 128 characters a name may have. */
export function baseName(path: string): string {
  const last = path.split(/[\\/]/).filter((p) => p !== "").pop() ?? "";
  const dot = last.lastIndexOf(".");
  return (dot > 0 ? last.slice(0, dot) : last).slice(0, 128);
}
