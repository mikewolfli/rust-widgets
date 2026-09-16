"""Temporary diagnostic: trace depth_delta execution."""
import re
from pathlib import Path

src = Path("tools/check_locking.sh").read_text(encoding="utf-8")
ns: dict = {"re": re}
exec(
    compile(src[src.index("PATTERN = re.compile") : src.index("findings: list[str] = []")], "x", "exec"),
    ns,
)
d = ns["depth_delta"]

line = "fn commit_text(&self, _text: &str) {}"
print("in_comment:", line.startswith(("///", "//!", "// ")) or line.rstrip() == "//")
in_string = False
delta = 0
index = 0
while index < len(line):
    ch = line[index]
    branch = ""
    if ch == '"':
        if in_string:
            in_string = False
            branch = "end-string"
        else:
            in_string = True
            branch = "start-string"
    elif ch == "'":
        branch = "APOSTROPHE"
    elif ch in "{}(":
        delta += 1
        branch = "open"
    elif ch in "})":
        delta -= 1
        branch = "close"
    print(f"  {index:2d} {ch!r} delta={delta:+d} {branch}")
    index += 1
print("total:", delta, "function says:", d(line))
