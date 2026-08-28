#!/usr/bin/env python3
"""抽出 Rust 檔中所有 pub 可見項目的正規化簽名，供拆檔前後 multiset 比對。

頂層 item 的 pub 宣告 + item 內縮排一層的 pub 成員（struct 欄位、impl 方法、enum 變體）。
"""
import re
import sys

SIG_END = re.compile(r"[{;=]")


def normalize(sig):
    return re.sub(r"\s+", " ", sig).strip().rstrip(",")


def collect(path):
    lines = open(path, encoding="utf-8").read().splitlines()
    out = []
    in_test = 0
    depth = 0
    for i, line in enumerate(lines):
        stripped = line.strip()
        indent = len(line) - len(line.lstrip())
        # 跳過 #[cfg(test)] mod tests 區塊
        if stripped.startswith("#[cfg(test)]"):
            in_test = indent + 1
        if in_test and stripped == "}" and indent == in_test - 1:
            in_test = 0
            continue
        if in_test:
            continue
        if not stripped.startswith("pub"):
            continue
        if re.match(r"^pub(\([a-z]+\))?\s+use\b", stripped):
            continue
        if indent > 4:
            continue
        # 收集完整簽名（多行簽名收到 { 或 ; 或 , 為止）
        buf = [stripped]
        j = i
        while not re.search(r"[{;,]\s*$", buf[-1].rstrip()) and j + 1 < len(lines):
            j += 1
            buf.append(lines[j].strip())
        sig = normalize(" ".join(buf))
        sig = re.sub(r"\s*\{\s*$", "", sig)
        sig = sig.rstrip(";").rstrip(",")
        out.append(sig)
    return out


sigs = []
for p in sys.argv[1:]:
    sigs.extend(collect(p))
sigs.sort()
print("\n".join(sigs))
