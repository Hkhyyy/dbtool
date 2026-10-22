/** GUI 단축키 목록 (Help › Keymap Reference 와 README 의 기준). [macOS 기본 키, 설명, 명령 id?]. 다른 OS 표기는 toOther 로 바꾼다.
 *  명령 id 가 있는 줄은 바꿀 수 있는 단축키(lib/shortcuts.ts)이고, 화면에는 현재 키가 보인다 */
export type KeyRow = [mac: string, desc: string, id?: string];
export interface KeySection {
  title: string;
  rows: KeyRow[];
}

export const KEYMAP: KeySection[] = [
  {
    title: "Search & Navigation",
    rows: [
      ["⇧⇧", "Search Everywhere (All / Database / Files / Symbols / Actions, ⇥ switches tabs)"],
      ["⌘O", "Go to Database Object", "goto.object"],
      ["⌥⌘O", "Go to Symbol (columns)", "goto.symbol"],
      ["⇧⌘O", "Go to File (open and recent tabs)", "goto.file"],
      ["⇧⌘A", "Find Action", "find.action"],
      ["⌥⇧⌘F", "Full-text Search in database", "fts"],
      ["⌘E", "Recent Files", "recent"],
      ["⌃⇥ / ⌃⇧⇥", "Switcher (hold ⌃, release to open)"],
      ["⇧⌘/", "Keymap Reference (this window)", "keymap"],
    ],
  },
  {
    title: "In search results (Search Everywhere / Go to)",
    rows: [
      ["↩", "Select in Database Explorer"],
      ["⌘↓", "Edit data (open table)"],
      ["⌘B", "Go to DDL"],
      ["F1", "Quick documentation"],
    ],
  },
  {
    title: "Tool windows & appearance",
    rows: [
      ["⌘1", "Database Explorer (focus / hide)", "explorer"],
      ["⌘3", "Find tool window (Full-text Search results)", "find.window"],
      ["⇧⎋", "Hide active tool window"],
      ["⇧⌘F12", "Maximize editor (also: double-click a tab)", "maximize"],
      ["⌘,", "Settings (theme)", "settings"],
      ["⌃`", "Quick switch theme (Dark / Light / High contrast / Sync with OS)", "theme"],
      ["⌃⇧.", "Editor font size bigger", "font.bigger"],
      ["⌃⇧,", "Editor font size smaller", "font.smaller"],
    ],
  },
  {
    title: "Editor tabs",
    rows: [
      ["⌘W", "Close tab (middle-click also closes)", "tab.close"],
      ["⌥ + click ×", "Close other tabs"],
      ["⇧⌘T", "Reopen closed tab", "tab.reopen"],
      ["⇧⌘]", "Next tab", "tab.next"],
      ["⇧⌘[", "Previous tab", "tab.prev"],
      ["⌥⇥", "Go to the next split (⇧ previous)", "split.other"],
      ["right-click tab", "Pin, Split Right/Down (up to 4 panes), Move to Next Group, Close others…"],
      ["drag tab", "Reorder, or drop on a pane's right/bottom edge to split it (up to 4)"],
    ],
  },
  {
    title: "Database Explorer",
    rows: [
      ["⌘↓ / F4", "Edit data (table) / open console (data source; Redis: key browser)", "tree.editData"],
      ["⇧⌘L", "New query console", "console.new"],
      ["⇧⌘F10", "Jump to query console", "tree.jumpConsole"],
      ["⌘B", "Go to DDL", "tree.ddl"],
      ["⌥⌘G", "SQL Generator", "tree.sqlGen"],
      ["⌥⇧⌘G", "Copy DDL to clipboard", "tree.ddlCopy"],
      ["⌥⇧⌘B", "Open DDL in console", "tree.ddlConsole"],
      ["⌥⇧⌘C", "Copy reference", "tree.copyRef"],
      ["F1", "Quick documentation", "tree.quickDoc"],
      ["⌘;", "Data source properties", "tree.properties"],
      ["⌘N", "New data source", "tree.newDs"],
      ["⌘D", "Duplicate data source", "tree.duplicate"],
      ["⌘R", "Refresh", "tree.refresh"],
      ["⌥⌘Y", "Refresh (anywhere)", "refresh"],
      ["⌘F2", "Disconnect", "tree.disconnect"],
      ["⌦", "Remove data source / drop object (asks first)", "tree.remove"],
      ["type letters", "Speed search in the tree"],
    ],
  },
  {
    title: "Query console",
    rows: [
      ["⌘↩", "Execute: asks what to run when there are several statements (⚙ Query Execution Settings), or the selection", "editor.run"],
      ["⌘F2", "Cancel running statement", "editor.cancel"],
      ["⌥⌘E", "Query history", "editor.history"],
      ["⌃Space", "Code completion"],
      ["⌥⌘L", "Reformat code", "editor.reformat"],
      ["⌘/", "Line comment"],
      ["⌥⌘/", "Block comment", "editor.blockComment"],
      ["⌘D", "Duplicate line", "editor.duplicate"],
      ["⌘⌫", "Delete line", "editor.deleteLine"],
      ["⌥⇧↑", "Move line up", "editor.moveUp"],
      ["⌥⇧↓", "Move line down", "editor.moveDown"],
      ["⌥↑", "Extend selection", "editor.extend"],
      ["⌥↓", "Shrink selection", "editor.shrink"],
      ["⌃G", "Select next occurrence", "editor.nextOcc"],
      ["⌃⌘G", "Select all occurrences", "editor.allOcc"],
      ["⇧⌘U", "Toggle case", "editor.toggleCase"],
      ["⌘L", "Go to line", "editor.gotoLine"],
      ["⌘F", "Find in editor"],
    ],
  },
  {
    title: "Data grid (results & table editor)",
    rows: [
      ["↩ / F2", "Edit cell"],
      ["⇧↩", "Open value editor"],
      ["⌥⌘N", "Set NULL"],
      ["⌥⌘D", "Set DEFAULT"],
      ["⌘N", "Add row"],
      ["⌘D", "Clone row"],
      ["⌘⌫", "Delete rows"],
      ["⌥⌘Z", "Revert selected changes"],
      ["⌘↩", "Submit changes"],
      ["⌘C / ⌘V", "Copy (with chosen extractor) / paste"],
      ["⌘A", "Select all"],
      ["⌘F", "Find in grid"],
      ["⌘R", "Reload"],
      ["⌘F2", "Cancel"],
      ["click header · ⌥ click", "Sort · add to multi-column sort"],
    ],
  },
];

/** macOS 기호 → Windows/Linux 표기 (앱은 ⌘ 를 Ctrl, ⌥ 를 Alt 로 받는다) */
export function toOther(k: string): string {
  return k
    .replace(/⌃/g, "Ctrl+")
    .replace(/⌘/g, "Ctrl+")
    .replace(/⌥/g, "Alt+")
    .replace(/⇧/g, "Shift+")
    .replace(/↩/g, "Enter")
    .replace(/⌫/g, "Backspace")
    .replace(/⌦/g, "Delete")
    .replace(/⎋/g, "Esc")
    .replace(/⇥/g, "Tab")
    .replace(/Ctrl\+Ctrl\+/g, "Ctrl+")
    .replace(/\+\s/g, " ");
}
