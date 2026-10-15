/** GUI 단축키 목록 (Help › Keymap Reference 와 README 의 기준). [macOS 키, 설명]. 다른 OS 표기는 toOther 로 바꾼다 */
export type KeyRow = [mac: string, desc: string];
export interface KeySection {
  title: string;
  rows: KeyRow[];
}

export const KEYMAP: KeySection[] = [
  {
    title: "Search & Navigation",
    rows: [
      ["⇧⇧", "Search Everywhere (All / Database / Files / Symbols / Actions, ⇥ switches tabs)"],
      ["⌘O", "Go to Database Object"],
      ["⌥⌘O", "Go to Symbol (columns)"],
      ["⇧⌘O", "Go to File (open and recent tabs)"],
      ["⇧⌘A", "Find Action"],
      ["⌥⇧⌘F", "Full-text Search in database"],
      ["⌘E", "Recent Files"],
      ["⌃⇥ / ⌃⇧⇥", "Switcher (hold ⌃, release to open)"],
      ["⇧⌘/", "Keymap Reference (this window)"],
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
      ["⌘1", "Database Explorer (focus / hide)"],
      ["⌘3", "Find tool window (Full-text Search results)"],
      ["⇧⎋", "Hide active tool window"],
      ["⇧⌘F12", "Maximize editor (also: double-click a tab)"],
      ["⌘,", "Settings (theme)"],
      ["⌃`", "Quick switch theme (Dark / Light / High contrast / Sync with OS)"],
      ["⌃⇧. / ⌃⇧,", "Editor font size bigger / smaller"],
    ],
  },
  {
    title: "Editor tabs",
    rows: [
      ["⌘W", "Close tab (middle-click also closes)"],
      ["⌥ + click ×", "Close other tabs"],
      ["⇧⌘T", "Reopen closed tab"],
      ["⇧⌘] / ⇧⌘[", "Next / previous tab"],
      ["⌥⇥", "Go to the other split"],
      ["right-click tab", "Pin, Split Right/Down, Move to Opposite Group, Close others…"],
      ["drag tab", "Reorder, or drop on the right/bottom edge to split"],
    ],
  },
  {
    title: "Database Explorer",
    rows: [
      ["⌘↓ / F4", "Edit data (table) / open console (data source)"],
      ["⇧⌘L", "New query console"],
      ["⇧⌘F10", "Jump to query console"],
      ["⌘B", "Go to DDL"],
      ["⌥⌘G", "SQL Generator"],
      ["⌥⇧⌘G", "Copy DDL to clipboard"],
      ["⌥⇧⌘B", "Open DDL in console"],
      ["⌥⇧⌘C", "Copy reference"],
      ["F1", "Quick documentation"],
      ["⌘;", "Data source properties"],
      ["⌘N", "New data source"],
      ["⌘D", "Duplicate data source"],
      ["⌘R / ⌥⌘Y", "Refresh"],
      ["⌘F2", "Disconnect"],
      ["⌦", "Remove data source / drop object (asks first)"],
      ["type letters", "Speed search in the tree"],
    ],
  },
  {
    title: "Query console",
    rows: [
      ["⌘↩", "Execute (statement at caret, or the selection)"],
      ["⌘F2", "Cancel running statement"],
      ["⌥⌘E", "Query history"],
      ["⌃Space", "Code completion"],
      ["⌥⌘L", "Reformat code"],
      ["⌘/ · ⌥⌘/", "Line comment · block comment"],
      ["⌘D", "Duplicate line"],
      ["⌘⌫", "Delete line"],
      ["⌥⇧↑ / ⌥⇧↓", "Move line up / down"],
      ["⌥↑ / ⌥↓", "Extend / shrink selection"],
      ["⌃G · ⌃⌘G", "Select next occurrence · all occurrences"],
      ["⇧⌘U", "Toggle case"],
      ["⌘L", "Go to line"],
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
