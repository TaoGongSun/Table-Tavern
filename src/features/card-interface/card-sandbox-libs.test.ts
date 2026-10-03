import { describe, expect, it, vi } from "vitest";
import { buildErrorCatchedSource, buildSandboxLibs, inlineScript } from "./card-sandbox-libs";

interface DomWindow {
  $: ((selector: unknown) => { text: () => string; length: number }) & { fn: { jquery: string } };
  _: { get: (object: unknown, path: string) => unknown; VERSION: string };
  errorCatched: <T extends (...args: never[]) => unknown>(fn: T) => T;
  eval: (source: string) => unknown;
  document: Document;
  close: () => void;
}

// jsdom 沒有型別宣告：動態載入，跑一份真文件看內建庫在沙盒裡能不能用
async function run(html: string): Promise<DomWindow> {
  const name = "jsdom";
  const { JSDOM } = (await import(/* @vite-ignore */ name)) as {
    JSDOM: new (html: string, options: object) => { window: DomWindow };
  };
  return new JSDOM(html, { runScripts: "dangerously" }).window;
}

describe("內建全域庫", () => {
  it("$ 與 _ 在殼裡可用，版本與酒館同代", async () => {
    const win = await run(`<!DOCTYPE html><html><head>${buildSandboxLibs()}</head><body><p id="x">值</p></body></html>`);
    expect(win.$("#x").text()).toBe("值");
    expect(win.$.fn.jquery).toBe("3.7.1");
    expect(win._.get({ a: { b: 2 } }, "a.b")).toBe(2);
    expect(win._.VERSION).toBe("4.17.21");
    win.close();
  });

  it("卡片自己再載一份庫覆蓋全域後，$ 與 _ 仍可用", async () => {
    const libs = buildSandboxLibs();
    const win = await run(`<!DOCTYPE html><html><head>${libs}${libs}</head><body><p id="x">值</p></body></html>`);
    expect(win.$("#x").text()).toBe("值");
    expect(win._.get({ a: 1 }, "a")).toBe(1);
    win.close();
  });

  it("內嵌原始碼不含可關掉外層 script 的字串", () => {
    const libs = buildSandboxLibs();
    // 只有三支外層 script 的結尾；jQuery 原始碼裡的 "<script" 字面不會關掉任何東西
    const bodies = libs.split("</script>").filter(Boolean);
    expect(bodies).toHaveLength(3);
    for (const body of bodies) {
      expect(body).not.toMatch(/<\/script/i);
      expect(body).not.toContain("<!--");
    }
    expect(inlineScript('a("</ScRiPt>")<!--')).toBe('a("<\\/ScRiPt>")<\\!--');
  });
});

describe("errorCatched", () => {
  function load(): DomWindow["errorCatched"] {
    const win = {} as { errorCatched: DomWindow["errorCatched"] };
    new Function("window", buildErrorCatchedSource())(win);
    return win.errorCatched;
  }

  it("保留 this、參數與回傳值", () => {
    const errorCatched = load();
    const target = {
      base: 10,
      add(this: { base: number }, a: number, b: number) {
        return this.base + a + b;
      },
    };
    const wrapped = errorCatched(target.add);
    expect(wrapped.call(target, 1, 2)).toBe(13);
  });

  it("同步例外：記錄後照樣拋出（酒館助手的語意）", () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const wrapped = load()(() => {
      throw new Error("壞了");
    });
    expect(() => wrapped()).toThrow("壞了");
    expect(error).toHaveBeenCalled();
    error.mockRestore();
  });

  it("回傳 Promise：回傳「記錄後再拋」的接續 Promise", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const wrapped = load()(async () => {
      throw new Error("非同步壞了");
    });
    await expect(wrapped()).rejects.toThrow("非同步壞了");
    expect(error).toHaveBeenCalledTimes(1);
    error.mockRestore();
    const ok = load()(async () => 5);
    await expect(ok()).resolves.toBe(5);
  });
});
