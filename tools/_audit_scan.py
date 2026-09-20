import re, sys, os

path = sys.argv[1]
txt = open(path).read()
syms = {}
for m in re.finditer(r'pub\s+(?:unsafe\s+)?extern "C" fn (rw_[a-z0-9_]+)', txt):
    name = m.group(1)
    i = txt.index('{', m.end())
    depth = 0
    j = i
    while j < len(txt):
        if txt[j] == '{':
            depth += 1
        elif txt[j] == '}':
            depth -= 1
            if depth == 0:
                break
        j += 1
    syms[name] = (txt[:m.start()].count('\n') + 1, txt[i + 1:j])

rows = []
for n, (ln, b) in syms.items():
    s = b.strip()
    rows.append((len(s.splitlines()), n, ln, s))
rows.sort()
for nl, n, ln, b in rows[:40]:
    print("--- %s [line %d] (%d lines)" % (n, ln, nl))
    print(b[:500])
