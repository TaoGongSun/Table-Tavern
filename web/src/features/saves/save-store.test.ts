import { forceCloseDatabase, IDBFactory, IDBObjectStore } from "fake-indexeddb";
import { afterEach, describe, expect, it, vi } from "vitest";
import minimal from "@desktop/shared/contracts/web-save/minimal.json";
import type { WebSave } from "@desktop/shared/contracts/web-save/web-save";
import { isSafari, newSaveId, openSaveStore, requestPersistence, restoreGlobals } from "./save-store";

const SAVE = minimal as unknown as WebSave;
const meta = (id: string, updatedAt: number) => ({ id, title: `卡 ${id}`, updatedAt, messageCount: 2 });

describe("IndexedDB 存檔庫", () => {
  it("新增、列出（新到舊）、讀回、覆寫、刪除", async () => {
    const store = openSaveStore(new IDBFactory())!;
    expect(await store.list()).toEqual([]);
    await store.put(meta("a", 1), SAVE);
    await store.put(meta("b", 3), { ...SAVE, user_name: "B" });
    await store.put(meta("c", 2), SAVE);
    expect((await store.list()).map((item) => item.id)).toEqual(["b", "c", "a"]);
    expect(await store.get("b")).toEqual({ ...SAVE, user_name: "B" });

    await store.put(meta("a", 9), { ...SAVE, user_name: "改過" });
    expect((await store.list()).map((item) => [item.id, item.updatedAt])).toEqual([
      ["a", 9],
      ["b", 3],
      ["c", 2],
    ]);
    expect((await store.get("a"))?.user_name).toBe("改過");

    await store.remove("b");
    expect((await store.list()).map((item) => item.id)).toEqual(["a", "c"]);
    expect(await store.get("b")).toBeNull();
  });

  it("同一個瀏覽器重開仍讀得到（換一個連線）", async () => {
    const factory = new IDBFactory();
    await openSaveStore(factory)!.put(meta("keep", 1), SAVE);
    expect(await openSaveStore(factory)!.get("keep")).toEqual(SAVE);
  });

  it("跨對話 global 存在同一個庫：沒存過是空表、換一條連線（重新整理）讀得回來", async () => {
    const factory = new IDBFactory();
    const store = openSaveStore(factory)!;
    expect(await store.getGlobal()).toEqual({});
    await store.loadGlobal({});
    await store.reserveGlobalWrite()({ 名聲: 5 });
    const target: Record<string, unknown> = {};
    expect(await restoreGlobals(openSaveStore(factory), target)).toBe(true);
    expect(target).toEqual({ 名聲: 5 });
    // 重試時分頁已有的值比較新，只補缺
    const played: Record<string, unknown> = { 名聲: 9 };
    await restoreGlobals(openSaveStore(factory), played);
    expect(played).toEqual({ 名聲: 9 });
  });

  it("a late global write from a previous table never overwrites the current one", async () => {
    const store = openSaveStore(new IDBFactory())!;
    await store.loadGlobal({});
    // 舊桌先取樣（score 1）、還沒寫；換桌後取樣 score 2 並寫好；舊桌那筆後來才到
    const oldTable = store.reserveGlobalWrite();
    const newTable = store.reserveGlobalWrite();
    await newTable({ score: 2 });
    await oldTable({ score: 1 });
    expect(await store.getGlobal()).toEqual({ score: 2 });
    // 照順序到的話照常一筆筆寫
    const first = store.reserveGlobalWrite();
    const second = store.reserveGlobalWrite();
    await Promise.all([first({ score: 3 }), second({ score: 4 })]);
    expect(await store.getGlobal()).toEqual({ score: 4 });
  });

  it("a failed global load never gets overwritten by an empty table", async () => {
    const factory = new IDBFactory();
    const seeded = openSaveStore(factory)!;
    await seeded.loadGlobal({});
    await seeded.reserveGlobalWrite()({ kept: "old" });

    // 這次開站讀 global 失敗：回 false（畫面提示可重試），這段期間不寫回
    const store = openSaveStore(factory)!;
    const loadGlobal = store.loadGlobal;
    store.loadGlobal = async () => Promise.reject(new Error("transient"));
    const tab: Record<string, unknown> = {};
    expect(await restoreGlobals(store, tab)).toBe(false);
    expect(tab).toEqual({});
    await store.reserveGlobalWrite()({});
    expect(await openSaveStore(factory)!.getGlobal()).toEqual({ kept: "old" });

    // 重試成功後才准寫回
    store.loadGlobal = loadGlobal;
    expect(await restoreGlobals(store, tab)).toBe(true);
    expect(tab).toEqual({ kept: "old" });
    await store.reserveGlobalWrite()({ ...tab, added: 1 });
    expect(await openSaveStore(factory)!.getGlobal()).toEqual({ kept: "old", added: 1 });
  });

  it("瀏覽器沒有 IndexedDB 就回 null（畫面說明存不了）", () => {
    expect(openSaveStore(undefined)).toBeNull();
  });

  it("存檔 id 不重複", () => {
    const ids = new Set(Array.from({ length: 200 }, () => newSaveId()));
    expect(ids.size).toBe(200);
  });
});

/** 真的 IndexedDB 請求包成 Promise（測試裡直接操作資料庫用）。 */
const request = <T>(req: IDBRequest<T>) =>
  new Promise<T>((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });

/** 記下存檔庫開過的每一條連線（模擬瀏覽器強制關閉用）。 */
function trackedFactory() {
  const factory = new IDBFactory();
  const opened: IDBDatabase[] = [];
  const open = factory.open.bind(factory);
  factory.open = (name: string, version?: number) => {
    const req = open(name, version);
    req.addEventListener("success", () => opened.push(req.result));
    return req;
  };
  return { factory, opened };
}

describe("存檔庫的失敗與競態", () => {
  afterEach(() => vi.restoreAllMocks());

  it("交易中途失敗（配額滿）整筆撤銷：新格不留半份、舊格維持原版", async () => {
    const store = openSaveStore(new IDBFactory())!;
    await store.put(meta("old", 1), SAVE);
    const put = IDBObjectStore.prototype.put;
    vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (this: IDBObjectStore, ...args: Parameters<IDBObjectStore["put"]>) {
      if (this.name === "data") throw new DOMException("quota exceeded", "QuotaExceededError");
      return put.apply(this, args);
    });
    await expect(store.put(meta("new", 2), SAVE)).rejects.toThrow();
    await expect(store.put(meta("old", 3), { ...SAVE, user_name: "改不進去" })).rejects.toThrow();
    vi.restoreAllMocks();
    expect(await store.list()).toEqual([meta("old", 1)]);
    expect(await store.get("new")).toBeNull();
    expect(await store.get("old")).toEqual(SAVE);
  });

  it("開庫失敗：回錯、不卡住，條件解除後下一次就開得起來", async () => {
    const factory = new IDBFactory();
    // 別處已把資料庫升到更新的版本：這版開不起來（VersionError）
    (await request(factory.open("tt-web", 99))).close();
    const store = openSaveStore(factory)!;
    await expect(store.list()).rejects.toThrow();
    await expect(store.put(meta("a", 1), SAVE)).rejects.toThrow();
    await request(factory.deleteDatabase("tt-web"));
    await store.put(meta("a", 1), SAVE);
    expect((await store.list()).map((item) => item.id)).toEqual(["a"]);
  });

  it("玩家清掉網站資料：舊連線放掉、不擋刪庫，之後的存檔寫進新庫", async () => {
    const { factory, opened } = trackedFactory();
    const store = openSaveStore(factory)!;
    await store.put(meta("before", 1), SAVE);
    await request(factory.deleteDatabase("tt-web"));
    expect(await store.list()).toEqual([]);
    await store.put(meta("after", 2), SAVE);
    expect((await store.list()).map((item) => item.id)).toEqual(["after"]);
    // 瀏覽器強制關閉連線（例如儲存空間被清）：下一次操作重開
    forceCloseDatabase(opened[opened.length - 1] as never);
    await store.put(meta("reopened", 3), SAVE);
    expect((await store.list()).map((item) => item.id)).toEqual(["reopened", "after"]);
  });

  it("同一格晚到的舊寫入不蓋掉較新的版本", async () => {
    const store = openSaveStore(new IDBFactory())!;
    await store.put(meta("slot", 20), { ...SAVE, user_name: "新回合" });
    await store.put(meta("slot", 10), { ...SAVE, user_name: "舊回合" });
    expect((await store.get("slot"))?.user_name).toBe("新回合");
    expect((await store.list())[0].updatedAt).toBe(20);
  });
});

describe("請瀏覽器保留資料", () => {
  const manager = (persisted: boolean, grant: boolean | Error) =>
    ({
      persisted: async () => persisted,
      persist: async () => {
        if (grant instanceof Error) throw grant;
        return grant;
      },
    }) as unknown as StorageManager;

  it("已經保留就不再請求；請求結果照實回報；不支援回 null", async () => {
    expect(await requestPersistence(manager(true, false))).toBe(true);
    expect(await requestPersistence(manager(false, true))).toBe(true);
    expect(await requestPersistence(manager(false, false))).toBe(false);
    expect(await requestPersistence(manager(false, new Error("denied")))).toBe(false);
    expect(await requestPersistence(undefined)).toBeNull();
  });

  it("Safari（含 iOS 上的其他瀏覽器）才提示七天清資料", () => {
    const mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";
    const ios = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/130.0 Mobile/15E148 Safari/604.1";
    const chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
    const android = "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Mobile Safari/537.36";
    expect([mac, ios, chrome, android].map((ua) => isSafari(ua))).toEqual([true, true, false, false]);
  });
});
