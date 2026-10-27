// MVU 共用核心片段：schema：generateSchema、getSchemaForPath、reconcileAndApplySchema、cleanUpMetadata、applyTemplate。片段規則與串接順序見 engine-source.ts。
export const SOURCE_SCHEMA = String.raw`
  // ---- schema ----
  function isArraySchema(node) {
    return !!node && node.type === "array";
  }
  function isObjectSchema(node) {
    return !!node && node.type === "object";
  }
  function setOwn(target, key, value) {
    Object.defineProperty(target, key, { value: value, enumerable: true, writable: true, configurable: true });
  }

  function generateSchema(data, oldNode, parentRecursive) {
    parentRecursive = parentRecursive === true;
    if (oldNode === "没有用别管这个") return { type: "any" };
    if (Array.isArray(data)) {
      var extensible = false;
      var recursive = parentRecursive;
      var oldElement;
      var template;
      if (oldNode) {
        if (isArraySchema(oldNode)) {
          extensible = oldNode.extensible === true;
          recursive = oldNode.recursiveExtensible === true || parentRecursive;
          oldElement = oldNode.elementType;
          template = oldNode.template;
        } else {
          console.error("Type mismatch: expected array schema but got " + oldNode.type);
        }
      }
      // 只含 $meta 的元素（$arrayMeta）：提取陣列的元資料後從資料移除
      var metaIndex = data.findIndex(function (item) {
        return lodash.isObject(item) && !lodash.isDate(item) && "$arrayMeta" in item && "$meta" in item && item["$arrayMeta"] === true;
      });
      if (metaIndex !== -1) {
        var metaElement = data[metaIndex];
        if (metaElement.$meta.extensible !== undefined) extensible = metaElement.$meta.extensible;
        if (metaElement.$meta.template !== undefined) template = metaElement.$meta.template;
        data.splice(metaIndex, 1);
      }
      var markerIndex = data.indexOf(EXTENSIBLE_MARKER);
      if (markerIndex > -1) {
        extensible = true;
        data.splice(markerIndex, 1);
      }
      var arrayNode = {
        type: "array",
        extensible: extensible || parentRecursive,
        recursiveExtensible: recursive,
        elementType: data.length > 0 ? generateSchema(data[0], oldElement, recursive) : { type: "any" }
      };
      if (template !== undefined) arrayNode.template = template;
      return arrayNode;
    }
    if (lodash.isObject(data) && !lodash.isDate(data)) {
      var oldExtensible = false;
      var oldRecursive = parentRecursive;
      var oldProperties;
      if (oldNode) {
        if (isObjectSchema(oldNode)) {
          oldExtensible = oldNode.extensible === true;
          oldRecursive = oldNode.recursiveExtensible === true || parentRecursive;
          oldProperties = oldNode.properties;
        } else {
          console.error("Type mismatch: expected object schema but got " + oldNode.type);
        }
      }
      var meta = data.$meta;
      var node = {
        type: "object",
        properties: {},
        extensible: oldExtensible || (meta && meta.extensible === true) || (meta && meta.recursiveExtensible === true) || parentRecursive,
        recursiveExtensible: oldRecursive || (meta && meta.recursiveExtensible === true)
      };
      node.extensible = node.extensible === true;
      node.recursiveExtensible = node.recursiveExtensible === true;
      if (meta && meta.template !== undefined) node.template = meta.template;
      else if (oldNode && isObjectSchema(oldNode) && oldNode.template) node.template = oldNode.template;
      if (data.$meta) delete data.$meta;
      for (var key in data) {
        var oldChild = oldProperties ? oldProperties[key] : undefined;
        var childRecursive = node.extensible !== false && node.recursiveExtensible;
        var child = generateSchema(data[key], oldChild, childRecursive);
        // 屬性是否必填：父節點可擴充預設選填，否則必填；$meta.required 列到的強制必填；舊 schema 明說的最優先
        var required = !node.extensible;
        if (meta && Array.isArray(meta.required) && meta.required.indexOf(key) >= 0) required = true;
        if (oldChild && oldChild.required === false) required = false;
        else if (oldChild && oldChild.required === true) required = true;
        setOwn(node.properties, key, lodash.assign({}, child, { required: required }));
      }
      return node;
    }
    var kind = typeof data;
    if (kind === "string" || kind === "number" || kind === "boolean") return { type: kind };
    return { type: "any" };
  }

  // 路徑對到 schema 節點：數字段走 elementType，其餘走 properties；找不到回 null
  function getSchemaForPath(schema, path) {
    if (!path || !schema) return schema || null;
    var segments = lodash.toPath(path);
    var current = schema;
    for (var i = 0; i < segments.length; i++) {
      if (!current) return null;
      var segment = segments[i];
      if (/^\d+$/.test(segment)) {
        if (isArraySchema(current)) current = current.elementType;
        else return null;
      } else if (isObjectSchema(current) && current.properties && hasOwn.call(current.properties, segment)) {
        current = current.properties[segment];
      } else {
        return null;
      }
    }
    return current;
  }

  function reconcileAndApplySchema(variables) {
    var clone = lodash.cloneDeep(variables.stat_data);
    var next = generateSchema(clone, variables.schema);
    if (!isObjectSchema(next)) return;
    var old = variables.schema;
    if (old && old.strictTemplate !== undefined) next.strictTemplate = old.strictTemplate;
    if (old && old.strictSet !== undefined) next.strictSet = old.strictSet;
    if (old && old.concatTemplateArray !== undefined) next.concatTemplateArray = old.concatTemplateArray;
    if (lodash.has(variables.stat_data, "$meta.strictTemplate")) next.strictTemplate = variables.stat_data["$meta"].strictTemplate;
    if (lodash.has(variables.stat_data, "$meta.strictSet")) next.strictSet = variables.stat_data["$meta"].strictSet;
    if (lodash.has(variables.stat_data, "$meta.concatTemplateArray")) {
      next.concatTemplateArray = variables.stat_data["$meta"].concatTemplateArray;
    }
    variables.schema = next;
  }

  // 清掉資料裡的元資料標記：陣列的擴充標記與 $arrayMeta 元素、物件的 $meta
  function cleanUpMetadata(data) {
    if (Array.isArray(data)) {
      var i = data.length;
      while (i--) {
        var item = data[i];
        if (item === EXTENSIBLE_MARKER) {
          data.splice(i, 1);
        } else if (lodash.isObject(item) && !lodash.isDate(item) && "$arrayMeta" in item && "$meta" in item && item["$arrayMeta"] === true) {
          data.splice(i, 1);
        } else {
          cleanUpMetadata(item);
        }
      }
    } else if (lodash.isObject(data) && !lodash.isDate(data)) {
      delete data.$meta;
      for (var key in data) cleanUpMetadata(data[key]);
    }
  }

  // 套模板：值的屬性優先；型別對不上就略過模板
  function applyTemplate(value, template, strictCast, concat) {
    if (!template) return value;
    var valueIsObject = lodash.isObject(value) && !Array.isArray(value) && !lodash.isDate(value);
    var valueIsArray = Array.isArray(value);
    var templateIsArray = Array.isArray(template);
    if (valueIsObject && !templateIsArray) return lodash.merge({}, template, value);
    if (valueIsArray && templateIsArray) return concat ? lodash.concat(value, template) : lodash.merge([], template, value);
    if (
      ((valueIsObject || valueIsArray) && templateIsArray !== valueIsArray) ||
      (!valueIsObject && !valueIsArray && lodash.isObject(template) && !Array.isArray(template))
    ) {
      console.error("Template type mismatch, skipping template merge.");
      return value;
    }
    if (!valueIsObject && !valueIsArray && templateIsArray) {
      if (strictCast) return value;
      return concat ? lodash.concat([value], template) : lodash.merge([], template, [value]);
    }
    return value;
  }

`;
