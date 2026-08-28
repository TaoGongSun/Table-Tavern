#!/usr/bin/env python3
"""把 Rust 檔切成頂層 item 切片，用於拆檔前後的 body 逐 byte 比對。

用法: slice_items.py <outdir> <file...>
自我驗證: 每個輸入檔的所有切片（含 separator）串接後必須逐 byte 等於原檔。
"""
import hashlib
import os
import re
import sys

ATTR = re.compile(r"^(#\[|#!\[|//)")
DECL = re.compile(
    r"^(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:async\s+)?"
    r"(?:extern\s+\"[^\"]*\"\s+)?(fn|struct|enum|type|const|static|trait|mod|union|use|impl|extern|macro_rules!)\b"
)
NAME = re.compile(
    r"^(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:async\s+)?"
    r"(?:extern\s+\"[^\"]*\"\s+)?(fn|struct|enum|type|const|static|trait|mod|union)\s+([A-Za-z0-9_]+)"
)


def item_key(code_line):
    s = code_line.strip()
    m = NAME.match(s)
    if m:
        return f"{m.group(1)}:{m.group(2)}"
    # impl / use / extern block / macro_rules 用整行正規化當 key
    s = s.rstrip("{").strip()
    s = re.sub(r"\s+", " ", s)
    return f"raw:{s}"


def slice_file(text, skip_cfg_test=True):
    """回傳 [(key, raw_slice, body)]，raw_slice 串接後等於原檔。"""
    lines = text.splitlines(keepends=True)
    out = []
    pending = []  # 尚未歸屬的行（空行/attr/comment）
    body = []
    state = "idle"
    for line in lines:
        stripped = line.strip()
        toplevel = bool(stripped) and not line[0].isspace()
        if state == "open":
            body.append(line)
            if (
                toplevel
                and stripped[0] in "}])"
                and not stripped.endswith(("{", "[", "(", ","))
            ):
                out.append(("", pending, body))
                pending, body, state = [], [], "idle"
            continue
        if not stripped:
            pending.append(line)
            continue
        if not toplevel:
            raise SystemExit(f"意外的縮排頂層行: {line!r}")
        if ATTR.match(stripped):
            pending.append(line)
            continue
        # code 起始行
        body = [line]
        if stripped.endswith(";"):
            out.append(("", pending, body))
            pending, body = [], []
        else:
            state = "open"
    if state == "open":
        raise SystemExit("檔尾仍有未閉合的 item")
    if pending:
        out.append(("", pending, []))

    result = []
    for _, pre, bod in out:
        raw = "".join(pre) + "".join(bod)
        if not bod:
            result.append(("_trailing", raw, ""))
            continue
        # attribute/doc comment 屬於 item body，前導空行是 separator
        i = 0
        while i < len(pre) and not pre[i].strip():
            i += 1
        sep, attrs = "".join(pre[:i]), "".join(pre[i:])
        key = item_key(bod[0])
        result.append((key, raw, attrs + "".join(bod), sep))
    return result


def main():
    outdir, files = sys.argv[1], sys.argv[2:]
    os.makedirs(outdir, exist_ok=True)
    manifest = []
    for path in files:
        text = open(path, encoding="utf-8").read()
        sliced = slice_file(text)
        assert "".join(s[1] for s in sliced) == text, f"{path}: 切片串接 != 原檔"
        seen = {}
        for entry in sliced:
            key, _raw, bodytext = entry[0], entry[1], entry[2]
            if key == "_trailing":
                continue
            # cfg(test) mod tests 不參與 production 比對
            if key == "mod:tests" and "#[cfg(test)]" in bodytext:
                continue
            n = seen.get(key, 0) + 1
            seen[key] = n
            ukey = key if n == 1 else f"{key}#{n}"
            safe = re.sub(r"[^A-Za-z0-9_.#-]", "_", ukey)[:120]
            with open(os.path.join(outdir, safe + ".rs"), "w", encoding="utf-8") as fh:
                fh.write(bodytext)
            digest = hashlib.sha256(bodytext.encode("utf-8")).hexdigest()[:16]
            manifest.append(f"{ukey}\t{digest}\t{len(bodytext.splitlines())}")
    manifest.sort()
    with open(os.path.join(outdir, "MANIFEST.tsv"), "w", encoding="utf-8") as fh:
        fh.write("\n".join(manifest) + "\n")
    print(f"items={len(manifest)} -> {outdir}/MANIFEST.tsv")


if __name__ == "__main__":
    main()
