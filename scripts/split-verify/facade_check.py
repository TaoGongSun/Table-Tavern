#!/usr/bin/env python3
"""拆檔後 facade 完整性：拆前的頂層 pub 項目，mod.rs 有沒有全部供得出來。

用法: facade_check.py <拆前原檔> <拆後的 mod.rs>

pub_api.py 刻意跳過 `pub use`，所以它證明的是「定義都還在、簽名沒變」，
證明不了「對外路徑接回來了」——那要靠這支比對 re-export 名單。
"""
import pathlib
import re
import sys

TOP = re.compile(
    r"^(pub(?:\(crate\))?)\s+(?:unsafe\s+)?"
    r"(?:fn|struct|enum|type|const|static|trait|union)\s+([A-Za-z0-9_]+)"
)
REEXPORT = re.compile(r"^(pub(?:\(crate\))?) use [a-z_]+::(\{[^}]*\}|[A-Za-z0-9_]+);", re.M)


def before_items(path):
    out = {}
    for line in pathlib.Path(path).read_text(encoding="utf-8").split("\n"):
        if line.startswith("#[cfg(test)]"):
            break  # 同檔測試區塊之後不算 production
        hit = TOP.match(line)
        if hit:
            out[hit.group(2)] = hit.group(1)
    return out


def facade_items(path):
    text = pathlib.Path(path).read_text(encoding="utf-8")
    out = {}
    for hit in REEXPORT.finditer(text):
        vis, body = hit.group(1), hit.group(2).strip("{}")
        for name in (n.strip() for n in body.split(",")):
            if name:
                out[name] = vis
    for line in text.split("\n"):
        hit = TOP.match(line)
        if hit:
            out[hit.group(2)] = hit.group(1)  # 留在 mod.rs 沒搬走的
    return out


before, after = before_items(sys.argv[1]), facade_items(sys.argv[2])
miss = {k: v for k, v in before.items() if k not in after}
extra = {k: v for k, v in after.items() if k not in before}
vis = {k: (before[k], after[k]) for k in before if k in after and before[k] != after[k]}
print(f"拆前頂層 pub 項目 = {len(before)}   facade 供得出來 = {len(after)}")
print(f"漏掉 = {len(miss)} {miss or ''}")
print(f"多出 = {len(extra)} {extra or ''}")
print(f"可見度改變 = {len(vis)} {vis or ''}")
sys.exit(1 if (miss or extra or vis) else 0)
