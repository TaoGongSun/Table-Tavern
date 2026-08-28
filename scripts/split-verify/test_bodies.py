#!/usr/bin/env python3
"""拆檔前後每支測試的 body 逐 byte 比對（raw hash，縮排與尾端空白都算）。

用法: test_bodies.py <拆前原檔> <拆後檔...>

leaf-name multiset 只證明「測試還在、名字沒變」，證不到 body 沒被削弱
（斷言被拿掉、assert 改成寬鬆版都不會改名字）。這支比對 body 本身。
"""
import hashlib
import pathlib
import re
import sys

TEST_FN = re.compile(r"^(\s*)fn ([A-Za-z0-9_]+)\(")


def collect(path):
    lines = pathlib.Path(path).read_text(encoding="utf-8").split("\n")
    out = {}
    for i, line in enumerate(lines):
        hit = TEST_FN.match(line)
        if not hit or not any(lines[j].strip() == "#[test]" for j in range(max(0, i - 3), i)):
            continue
        indent, name = hit.group(1), hit.group(2)
        end = i
        while lines[end] != indent + "}":
            end += 1
        body = "\n".join(lines[i:end + 1])
        if name in out:
            raise SystemExit(f"測試名重複: {name}（{path}）")
        out[name] = hashlib.sha256(body.encode()).hexdigest()[:12]
    return out


before = collect(sys.argv[1])
after = {}
for p in sys.argv[2:]:
    for k, v in collect(p).items():
        if k in after:
            raise SystemExit(f"測試名跨檔重複: {k}")
        after[k] = v

miss = sorted(set(before) - set(after))
extra = sorted(set(after) - set(before))
changed = sorted(k for k in set(before) & set(after) if before[k] != after[k])
print(f"拆前測試 = {len(before)}   拆後 = {len(after)}")
print(f"遺失 = {len(miss)} {miss or ''}")
print(f"新增 = {len(extra)} {extra or ''}")
print(f"body 被改 = {len(changed)} {changed or ''}")
sys.exit(1 if (miss or extra or changed) else 0)
