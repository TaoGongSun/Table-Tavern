// MVU 共用核心片段：路徑：JSON Pointer 轉換、模型常寫的路徑修正、只走自有屬性的路徑存取。片段規則與串接順序見 engine-source.ts。
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

  // 路徑字串轉分段，同 lodash castPath：整串本身是自有鍵、或沒有點與括號語法，就當單一鍵（原型上的不算）
  var DEEP_PATH = /\.|\[(?:[^[\]]*|(["'])(?:(?!\1)[^\\]|\\.)*?\1)\]/;
  function pathSegments(path, root) {
    if (Array.isArray(path)) return path;
    var text = String(path);
    if (/^\w*$/.test(text) || !DEEP_PATH.test(text) || (isContainer(root) && hasOwn.call(root, text))) return [text];
    return lodash.toPath(text);
  }

  // 路徑存取只走自有屬性（MagVarUpdate 與 zod 共用）：原型上的 constructor、prototype、__proto__ 一律當不存在，
  // 寫入用 defineProperty，模型寫的路徑碰不到 Object.prototype（上游用 lodash 會走到繼承屬性，這裡刻意不照做
  // 〔作者裁決 2026-10-27〕）
  function isContainer(value) {
    return value !== null && typeof value === "object";
  }
  // 讀取：同 lodash 也讀得到字串等原始值的自有屬性（如 "abc".length）；遇到函式就停，不往函式裡讀
  function readable(node, key) {
    return node !== null && node !== undefined && typeof node !== "function" && hasOwn.call(node, key);
  }
  function ownGet(target, path) {
    var segments = pathSegments(path, target);
    var node = target;
    for (var i = 0; i < segments.length; i++) {
      if (!readable(node, segments[i])) return undefined;
      node = node[segments[i]];
    }
    return node;
  }
  function ownHas(target, path) {
    var segments = pathSegments(path, target);
    if (segments.length === 0) return false;
    var node = target;
    for (var i = 0; i < segments.length; i++) {
      if (!readable(node, segments[i])) return false;
      node = node[segments[i]];
    }
    return true;
  }
  function ownPut(container, key, value) {
    var name = String(key);
    if (Array.isArray(container) && name !== "__proto__") container[name] = value;
    else Object.defineProperty(container, name, { value: value, enumerable: true, writable: true, configurable: true });
  }
  // 同 lodash.set：中途不存在或不是物件的段換成新容器（下一段像索引就建陣列），但只建自有屬性；函式也換掉，
  // 不往函式裡寫（transform 放進表的函式，其 prototype 碰不到）
  function ownSet(target, path, value) {
    var segments = pathSegments(path, target);
    var node = target;
    for (var i = 0; i < segments.length - 1; i++) {
      var key = String(segments[i]);
      var next = hasOwn.call(node, key) ? node[key] : undefined;
      if (!isContainer(next)) {
        next = /^(?:0|[1-9]\d*)$/.test(String(segments[i + 1])) ? [] : {};
        ownPut(node, key, next);
      }
      node = next;
    }
    if (segments.length) ownPut(node, segments[segments.length - 1], value);
    return target;
  }
  function ownUnset(target, path) {
    var segments = pathSegments(path, target);
    if (segments.length === 0) return;
    var parent = ownGet(target, segments.slice(0, -1));
    var key = String(segments[segments.length - 1]);
    if (isContainer(parent) && hasOwn.call(parent, key)) delete parent[key];
  }
  function ownPick(source, keys) {
    var picked = {};
    keys.forEach(function (key) {
      if (hasOwn.call(source, key)) ownPut(picked, key, source[key]);
    });
    return picked;
  }
`;
