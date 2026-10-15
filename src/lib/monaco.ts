import * as monaco from "monaco-editor/editor/editor.api";
// editor.api 는 핵심만 있고 자동완성(suggest)·찾기·괄호매칭 같은 기능이 빠져 있다 → editor.main 으로 전부 등록
import "monaco-editor/editor/editor.main";
import "monaco-editor/languages/definitions/pgsql/register";
import "monaco-editor/languages/definitions/mysql/register";
import "monaco-editor/languages/definitions/redis/register";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";
import { loader } from "@monaco-editor/react";

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };
loader.config({ monaco });

monaco.editor.defineTheme("dbtool-dark", {
  base: "vs-dark",
  inherit: true,
  rules: [
    { token: "keyword", foreground: "CF8E6D" },
    { token: "string", foreground: "6AAB73" },
    { token: "number", foreground: "2AACB8" },
    { token: "comment", foreground: "7A7E85", fontStyle: "italic" },
    { token: "predefined", foreground: "56A8F5" },
  ],
  colors: {
    "editor.background": "#1E1F22",
    "editor.lineHighlightBackground": "#26282E",
    "editorLineNumber.foreground": "#4B5059",
    "editorLineNumber.activeForeground": "#A1A3AB",
    "editor.selectionBackground": "#214283",
  },
});

export { monaco };

monaco.editor.defineTheme("dbtool-light", {
  base: "vs",
  inherit: true,
  rules: [
    { token: "keyword", foreground: "0033B3" },
    { token: "string", foreground: "067D17" },
    { token: "number", foreground: "1750EB" },
    { token: "comment", foreground: "8C8C8C", fontStyle: "italic" },
    { token: "predefined", foreground: "00627A" },
  ],
  colors: {
    "editor.background": "#FFFFFF",
    "editor.lineHighlightBackground": "#F5F8FE",
    "editorLineNumber.foreground": "#AEB3C2",
    "editorLineNumber.activeForeground": "#767A8A",
    "editor.selectionBackground": "#A6D2FF",
  },
});

monaco.editor.defineTheme("dbtool-hc", {
  base: "hc-black",
  inherit: true,
  rules: [
    { token: "keyword", foreground: "FFB86C" },
    { token: "string", foreground: "8DF08D" },
    { token: "number", foreground: "6FE8F5" },
    { token: "comment", foreground: "B0B0B0", fontStyle: "italic" },
  ],
  colors: { "editor.background": "#000000" },
});
