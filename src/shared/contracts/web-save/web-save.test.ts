import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { looksLikeWebSave, parseWebSave } from "./web-save";

const read = (name: string) => readFileSync(fileURLToPath(new URL(`./${name}`, import.meta.url)), "utf8");
const fixture = (name: string) => JSON.parse(read(name)) as Record<string, unknown> & { [key: string]: any };
const parse = (value: unknown) => parseWebSave(JSON.stringify(value));

describe("web save contract v1", () => {
  it("accepts the contract fixtures", () => {
    for (const name of ["short.json", "worldbook-route.json", "worldbook-route-mvu.json", "minimal.json"]) {
      const result = parseWebSave(read(name));
      expect(result.ok, name).toBe(true);
    }
  });

  it("rejects an unknown version without guessing", () => {
    expect(parseWebSave(read("future-version.json"))).toEqual({ ok: false, error: { kind: "version", version: "2" } });
  });

  it("tells card files apart from web saves", () => {
    expect(parse({ spec: "chara_card_v2", data: { name: "A" } })).toEqual({ ok: false, error: { kind: "not_web_save" } });
    expect(parseWebSave("not json")).toEqual({ ok: false, error: { kind: "not_web_save" } });
    const encode = (text: string) => new TextEncoder().encode(text);
    expect(looksLikeWebSave(encode(read("short.json")))).toBe(true);
    expect(looksLikeWebSave(encode(read("future-version.json")))).toBe(true);
    expect(looksLikeWebSave(encode('{"spec":"chara_card_v2"}'))).toBe(false);
    expect(looksLikeWebSave(new Uint8Array([0x89, 0x50, 0x4e, 0x47]))).toBe(false);
  });

  it("checks the same structure rules as the desktop importer", () => {
    const broken = (edit: (save: ReturnType<typeof fixture>) => void) => {
      const save = fixture("short.json");
      edit(save);
      const result = parse(save);
      return result.ok ? null : result.error.kind;
    };
    expect(broken((save) => (save.messages[1].opening = true))).toBe("invalid");
    expect(broken((save) => (save.messages[2].id = save.messages[1].id))).toBe("invalid");
    expect(broken((save) => (save.messages[0].ts = "yesterday"))).toBe("invalid");
    expect(broken((save) => (save.mvu.seed = null))).toBe("invalid");
    expect(broken((save) => (save.mvu.layers.extension = { "": {} }))).toBe("invalid");
    expect(broken((save) => delete save.mvu.layers.preset)).toBe("invalid");
    expect(broken((save) => delete save.mvu)).toBe("invalid");
    expect(broken((save) => delete save.opening_index)).toBe("invalid");
    expect(broken((save) => (save.world_info.last_message_id = "nope"))).toBe("invalid");
    expect(broken((save) => (save.card_storage = { a: 1 }))).toBe("invalid");
    expect(broken((save) => (save.user_name = "兩\n行"))).toBe("invalid");
    expect(broken((save) => (save.import_route = "gm"))).toBe("invalid");
    // 不認得的頂層鍵略過
    expect(broken((save) => (save.future_field = 1))).toBeNull();
  });

  it("rejects every shared invalid fixture with the same class as the desktop importer", () => {
    const names = readdirSync(fileURLToPath(new URL("./invalid/", import.meta.url)));
    expect(names.length).toBeGreaterThanOrEqual(20);
    for (const name of names) {
      const result = parseWebSave(read(`invalid/${name}`));
      expect(result.ok, name).toBe(false);
      if (!result.ok) expect(result.error.kind, name).toBe(name.split("--")[0]);
    }
  });

  it("never carries keys or model settings", () => {
    for (const name of ["short.json", "worldbook-route.json"]) {
      const text = read(name);
      expect(text).not.toMatch(/sk-or-|api_?key|"model"/i);
    }
  });
});
