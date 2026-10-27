import { describe, expect, it } from "vitest";
import { findSchemaScript, isMvuZodUrl, splitSchemaImports } from "./schema-script";

const MVU_ZOD = "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js";
const join = (parts: string[]) => parts.join(JSON.stringify("SHIM"));

describe("findSchemaScript（規格 1.5）", () => {
  const schema = (content: string, enabled?: boolean) => ({ name: "变量结构", content, ...(enabled === undefined ? {} : { enabled }) });
  it("取腳本順序第一支啟用、含 registerMvuSchema 的", () => {
    const data = {
      extensions: {
        tavern_helper: {
          scripts: [{ name: "MVU", content: "import 'x';" }, schema("A registerMvuSchema", false), schema("B registerMvuSchema"), schema("C registerMvuSchema")],
        },
      },
    };
    expect(findSchemaScript(data)).toBe("B registerMvuSchema");
  });
  it("舊版 TavernHelper_scripts（巢狀）也認；停用的上層整支不算", () => {
    expect(findSchemaScript({ extensions: { TavernHelper_scripts: [{ type: "script", value: schema("old registerMvuSchema") }] } })).toBe("old registerMvuSchema");
    expect(findSchemaScript({ extensions: { TavernHelper_scripts: [{ enabled: false, value: schema("x registerMvuSchema") }] } })).toBeNull();
  });
  it("沒有註冊 schema 的腳本是非 zod 桌", () => {
    expect(findSchemaScript({ extensions: { tavern_helper: { scripts: [{ content: "import 'MagVarUpdate';" }] } } })).toBeNull();
    expect(findSchemaScript({})).toBeNull();
  });
});

describe("isMvuZodUrl（同 runtime.rs is_mvu_zod_url）", () => {
  it.each([
    MVU_ZOD,
    "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js",
    "https://fastly.jsdelivr.net/gh/StageDog/tavern_resource@main/dist/util/mvu_zod.js",
    "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource@1.2.3/dist/util/mvu_zod.js",
  ])("收 %s", (url) => expect(isMvuZodUrl(url)).toBe(true));
  it.each([
    "http://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js",
    "https://evil.example/gh/StageDog/tavern_resource/dist/util/mvu_zod.js",
    "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu.js",
    "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource@/dist/util/mvu_zod.js",
    "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource@x/../dist/util/mvu_zod.js",
    "https://cdn.jsdelivr.net/gh/StageDog/tavern_resourcex/dist/util/mvu_zod.js",
    "https://cdn.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js?x=1",
    "mvu_zod",
  ])("拒 %s", (url) => expect(isMvuZodUrl(url)).toBe(false));
});

describe("splitSchemaImports", () => {
  it("白名單 import（含別名、export from、跳脫字元）換成墊片，其餘原文不動", () => {
    const source = `import { registerMvuSchema as r } from '${MVU_ZOD}';\nexport { registerMvuSchema } from "${MVU_ZOD}";\nconst u = import.meta.url;\nexport const Schema = z.object({});\n$(() => r(Schema));`;
    const split = splitSchemaImports(source);
    expect(split.ok).toBe(true);
    if (!split.ok) return;
    expect(join(split.parts)).toBe(source.replace(`'${MVU_ZOD}'`, '"SHIM"').replace(`"${MVU_ZOD}"`, '"SHIM"'));
    const escaped = splitSchemaImports(`import { registerMvuSchema } from "${MVU_ZOD.replace("h", "\\u0068")}";`);
    expect(escaped.ok && join(escaped.parts)).toBe('import { registerMvuSchema } from "SHIM";');
  });
  it.each([
    ["白名單外的靜態 import", "import x from 'https://evil.example/x.js';"],
    ["裸模組名", "import 'lodash';"],
    ["同站相對路徑", "import './other.js';"],
    ["blob 網址", "import 'blob:https://a/b';"],
    ["白名單外的 export from", "export * from 'https://evil.example/x.js';"],
    ["白名單外的動態 import 字面量", "const m = import('https://evil.example/x.js');"],
    ["動態 import 非字面量（即使值是白名單網址）", `const u = '${MVU_ZOD}'.slice(0); import(u);`],
    ["動態 import 字面量串接", `import('${MVU_ZOD}' + '');`],
    ["動態 import 模板字串（即使沒有插值、值是白名單網址）", `import(\`${MVU_ZOD}\`);`],
    ["動態 import 模板字串插值", "import(`https://evil.example/${1}.js`);"],
    ["夾在運算式裡的白名單外動態 import", "const f = () => (0, import('x'));"],
    ["語法錯", "import {"],
  ])("拒收：%s", (_name, source) => {
    expect(splitSchemaImports(source).ok).toBe(false);
  });
  it("白名單網址的動態 import 字串字面量（含註解、import 屬性）換成墊片（同桌面 loader 放行）", () => {
    const source = `const m = await import('${MVU_ZOD}');\nconst n = await import(/* x */ "${MVU_ZOD}" /* y */, { with: {} });`;
    const split = splitSchemaImports(source);
    expect(split.ok && join(split.parts)).toBe('const m = await import("SHIM");\nconst n = await import(/* x */ "SHIM" /* y */, { with: {} });');
  });
  it("字串與註解裡的 import 字樣不算", () => {
    const source = "// import('x')\nconst s = \"import 'y'\";\n/* import z from 'z' */";
    const split = splitSchemaImports(source);
    expect(split.ok && join(split.parts)).toBe(source);
  });
});
