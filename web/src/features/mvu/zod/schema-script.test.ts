import { describe, expect, it } from "vitest";
import { findSchemaScript, isMvuZodUrl } from "./schema-script";

const MVU_ZOD = "https://testingcf.jsdelivr.net/gh/StageDog/tavern_resource/dist/util/mvu_zod.js";

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
