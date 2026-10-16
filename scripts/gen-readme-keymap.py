"""README 의 단축키 표를 GUI(src/lib/keymap.ts)·TUI(crates/tui/src/help.rs) 에서 다시 만든다.
사용: python3 scripts/gen-readme-keymap.py  (README.md 의 <!-- keymap:start/end --> 사이를 교체)"""
import re, pathlib
root = pathlib.Path(__file__).resolve().parent.parent

def esc(s): return s.replace("|", "\\|")

def gui():
    src = (root / "src/lib/keymap.ts").read_text()
    out = []
    for m in re.finditer(r'title: "([^"]+)",\s*rows: \[(.*?)\n    \],', src, re.S):
        out.append(f"\n#### {m.group(1)}\n\n| macOS | Action |\n|---|---|")
        for k, d in re.findall(r'\["([^"]*)", "([^"]*)"(?:, "[^"]*")?\]', m.group(2)):
            out.append(f"| `{esc(k)}` | {esc(d)} |")
    return "\n".join(out)

def tui():
    src = (root / "crates/tui/src/help.rs").read_text()
    # 바꿀 수 있는 명령: keys.rs 의 기본 키 + app.rs ACTIONS 의 이름
    names = dict(re.findall(r'\(Action::(\w+), "([^"]*)", "[^"]*"\)', (root / "crates/tui/src/app.rs").read_text()))
    keys = (root / "crates/tui/src/keys.rs").read_text()
    out = ["\n#### Commands (default keys — change them in the F1 window)\n\n| Key | Action |\n|---|---|"]
    for a, ks in re.findall(r'\(Action::(\w+), &\[(.*?)\]\)', keys):
        k = " / ".join(re.findall(r'"([^"]*)"', ks)).replace("\\\\", "\\")
        out.append(f"| `{esc(k)}` | {esc(names[a])} |")
    for m in re.finditer(r'\("([^"]+)", &\[(.*?)\]\),', src, re.S):
        out.append(f"\n#### {m.group(1)}\n\n| Key | Action |\n|---|---|")
        for k, d in re.findall(r'\("([^"]*)", "([^"]*)"\)', m.group(2)):
            k = k.replace("\\\\", "\\")  # Rust 문자열의 \\ → \
            out.append(f"| `{esc(k)}` | {esc(d)} |")
    return "\n".join(out)

readme = root / "README.md"
text = readme.read_text()
for name, body in (("gui", gui()), ("tui", tui())):
    text = re.sub(rf"(<!-- keymap:{name}:start -->).*?(<!-- keymap:{name}:end -->)", lambda m: m.group(1) + "\n" + body + "\n\n" + m.group(2), text, flags=re.S)
readme.write_text(text)
print("README keymap updated")
