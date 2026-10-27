// MVU 共用核心片段：路徑：JSON Pointer 轉換、模型常寫的路徑修正。片段規則與串接順序見 engine-source.ts。
export const SOURCE_PATH = String.raw`
  // ---- 路徑 ----
  function segmentsToPath(segments) {
    return segments
      .map(function (segment) {
        return '["' + segment.replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"]';
      })
      .join("");
  }
  function pointerToPath(pointer) {
    if (!pointer) return "";
    var body = pointer.charAt(0) === "/" ? pointer.substring(1) : pointer;
    return segmentsToPath(
      body.split("/").map(function (segment) {
        return segment.replace(/~1/g, "/").replace(/~0/g, "~");
      })
    );
  }
  // 模型常寫的路徑修正：裸數字是索引、帶引號的數字是字串鍵、含空白的鍵改成括號寫法、整段被引號包住的點分欄位去引號
  function pathFix(path) {
    if (!path) return path;
    var brackets = path.replace(/\[([^\]]*)\]/g, function (_match, rawInner) {
      var inner = rawInner.trim();
      if (!inner) return "[]";
      var wasQuoted = false;
      var first = inner.charAt(0);
      var last = inner.charAt(inner.length - 1);
      if (inner.length >= 2 && (first === '"' || first === "'") && first === last) {
        wasQuoted = true;
        inner = inner.slice(1, -1);
      }
      if (/^\d+$/.test(inner)) {
        if (!wasQuoted) return "[" + inner + "]";
        return '["' + inner.replace(/"/g, '\\"') + '"]';
      }
      if (/\s/.test(inner)) return '["' + inner.replace(/"/g, '\\"') + '"]';
      return "[" + inner + "]";
    });
    return brackets.replace(/(^|\.)(["'])([^"']*)\2(?=\.|\[|$)/g, function (_match, prefix, _quote, name) {
      if (!/\s/.test(name) && !/[.[\]]/.test(name)) return prefix + name;
      var escaped = name.replace(/"/g, '\\"');
      return prefix === "." ? '["' + escaped + '"]' : prefix + '["' + escaped + '"]';
    });
  }

`;
