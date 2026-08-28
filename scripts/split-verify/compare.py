#!/usr/bin/env python3
"""拆檔前後 production body 逐 byte 比對；可見度前綴差異單獨列出。

用法: compare.py <拆前原檔> <拆後檔...>
"""
import hashlib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from slice_items import slice_file

VIS = re.compile(r"^(pub(?:\([a-z]+\))?\s+)")
SKIP = re.compile(r"^(raw:(pub(\([a-z]+\))? )?use |mod:)")


def collect(paths):
    out = {}
    for path in paths:
        text = open(path, encoding="utf-8").read()
        seen = {}
        for entry in slice_file(text):
            key, body = entry[0], entry[2]
            if key == "_trailing":
                continue
            if key == "mod:tests" and "#[cfg(test)]" in body:
                continue
            if SKIP.match(key):
                continue
            n = seen.get(key, 0) + 1
            seen[key] = n
            ukey = key if n == 1 else f"{key}#{n}"
            if ukey in out:
                raise SystemExit(
                    f"同名 item 出現在兩個檔（{out[ukey][3]} 與 {os.path.basename(path)}）: {ukey}"
                    "——比對會互相覆寫，請先確認是不是搬重複了")
            lines = body.splitlines(keepends=True)
            i = 0
            while i < len(lines) and re.match(r"^\s*(#\[|//)", lines[i]):
                i += 1
            vis = ""
            if i < len(lines):
                m = VIS.match(lines[i])
                if m:
                    vis = m.group(1).strip()
                    lines[i] = lines[i][m.end():]
            norm = "".join(lines)
            out[ukey] = (
                hashlib.sha256(body.encode()).hexdigest()[:12],
                hashlib.sha256(norm.encode()).hexdigest()[:12],
                vis,
                os.path.basename(path),
                norm,
            )
    return out


before = collect([sys.argv[1]])
after = collect(sys.argv[2:])

miss = sorted(set(before) - set(after))
extra = sorted(set(after) - set(before))
vis_changed, body_changed = [], []
for k in sorted(set(before) & set(after)):
    b, a = before[k], after[k]
    if b[1] != a[1]:
        body_changed.append((k, a[3]))
    elif b[2] != a[2]:
        vis_changed.append((k, b[2] or "(private)", a[2], a[3]))

print(f"before items={len(before)}  after items={len(after)}")
print(f"遺失={len(miss)} {miss}")
print(f"多出={len(extra)} {extra}")
print(f"\n可見度變更 {len(vis_changed)} 項（白名單第 2 項）:")
for k, b, a, f in vis_changed:
    print(f"  {k}: {b} -> {a}  [{f}]")
print(f"\n內容變更 {len(body_changed)} 項:")
for k, f in body_changed:
    print(f"  {k}  [{f}]")
    import difflib
    d = list(difflib.unified_diff(
        before[k][4].splitlines(), after[k][4].splitlines(), "before", "after", lineterm="", n=0))
    for line in d[:14]:
        print("    " + line)
same = len(before) - len(miss) - len(vis_changed) - len(body_changed)
print(f"\n逐 byte 相同（含可見度）: {same}/{len(before)}")
