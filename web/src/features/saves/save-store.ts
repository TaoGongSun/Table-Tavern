// 網頁存檔在這個瀏覽器裡的多份保存（IndexedDB）。`meta` 是清單要的小資料，`data` 是整份契約 JSON（卡與
// PNG 可能很大，列清單不必整份讀出來），同一筆的兩邊在同一個交易裡寫，不會一半有一半沒有。`globals` 放
// ST 的跨對話 global 變數（照 ST 存在瀏覽器、所有存檔共用、重新整理不丟，D29〔作者裁決 2026-10-07〕）。
import type { VarsTable, WebSave } from "@desktop/shared/contracts/web-save/web-save";

export interface SaveMeta {
  id: string;
  /** 卡名 */
  title: string;
  updatedAt: number;
  messageCount: number;
}

export interface SaveStore {
  /** 新到舊 */
  list(): Promise<SaveMeta[]>;
  get(id: string): Promise<WebSave | null>;
  put(meta: SaveMeta, save: WebSave): Promise<void>;
  remove(id: string): Promise<void>;
  /** 讀出存著的 ST 跨對話 global 變數（沒存過是空表），只讀、不算載入。 */
  getGlobal(): Promise<VarsTable>;
  /** 把存著的 global 補進分頁（只補分頁沒有的鍵），補完這個庫才算載入過、准寫回。 */
  loadGlobal(target: Record<string, unknown>): Promise<void>;
  /**
   * 預約一次 global 寫入，要在取樣分頁 global 之前（至遲同一段同步程式）呼叫。版本在預約當下定（＝這份內容
   * 的新舊），實際寫入跨桌排同一條隊、舊版本不蓋新版本。寫入資格也在預約當下定：當時還沒載入過，這份取樣
   * 就少了庫裡的鍵，之後就算重試讀回成功也永遠不寫（免得空表蓋掉庫裡的值）。
   */
  reserveGlobalWrite(): (table: VarsTable) => Promise<void>;
}

const DB_NAME = "tt-web";
const DB_VERSION = 2;
const META = "meta";
const DATA = "data";
const GLOBALS = "globals";
const GLOBAL_KEY = "global";

const done = <T>(request: IDBRequest<T>) =>
  new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });

const committed = (transaction: IDBTransaction) =>
  new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error);
    transaction.onabort = () => reject(transaction.error ?? new Error("transaction aborted"));
  });

function openDb(factory: IDBFactory): Promise<IDBDatabase> {
  const request = factory.open(DB_NAME, DB_VERSION);
  request.onupgradeneeded = () => {
    const db = request.result;
    if (!db.objectStoreNames.contains(META)) db.createObjectStore(META, { keyPath: "id" });
    if (!db.objectStoreNames.contains(DATA)) db.createObjectStore(DATA);
    if (!db.objectStoreNames.contains(GLOBALS)) db.createObjectStore(GLOBALS);
  };
  return done(request);
}

/**
 * 打開這個瀏覽器的存檔庫。拿不到 IndexedDB（例如瀏覽器禁止網站存資料）就回 null。
 * 連線失效（玩家清掉網站資料、別的分頁要升級資料庫）就放掉，下一次操作重新開；開庫失敗也不快取失敗。
 */
export function openSaveStore(factory: IDBFactory | undefined = globalThis.indexedDB): SaveStore | null {
  if (!factory) return null;
  let db: Promise<IDBDatabase> | null = null;
  const connect = () =>
    (db ??= openDb(factory).then(
      (opened) => {
        const forget = () => {
          opened.close();
          db = null;
        };
        opened.onversionchange = forget;
        opened.onclose = () => (db = null);
        return opened;
      },
      (error: unknown) => {
        db = null;
        throw error;
      },
    ));
  /** 開交易；手上的連線已經關掉（InvalidStateError）就重開一次再試。 */
  const transaction = async (stores: string[], mode: IDBTransactionMode) => {
    try {
      return (await connect()).transaction(stores, mode);
    } catch (error) {
      if (!(error instanceof DOMException && error.name === "InvalidStateError")) throw error;
      db = null;
      return (await connect()).transaction(stores, mode);
    }
  };
  /** 寫入交易：交易裡任何一步失敗（含同步丟出的錯）整筆撤銷，不留只寫了一半的存檔。 */
  const write = async (work: (transaction: IDBTransaction) => void, stores: string[] = [META, DATA]) => {
    const tx = await transaction(stores, "readwrite");
    const finished = committed(tx);
    try {
      work(tx);
    } catch (error) {
      tx.abort();
      await finished.catch(() => {});
      throw error;
    }
    await finished;
  };
  let globalLoaded = false;
  let globalReserved = 0;
  let globalWritten = 0;
  let globalQueue: Promise<unknown> = Promise.resolve();
  const readGlobal = async () => {
    const store = (await transaction([GLOBALS], "readonly")).objectStore(GLOBALS);
    return ((await done(store.get(GLOBAL_KEY))) as VarsTable | undefined) ?? {};
  };
  return {
    async list() {
      const store = (await transaction([META], "readonly")).objectStore(META);
      const all = await done(store.getAll() as IDBRequest<SaveMeta[]>);
      return all.sort((a, b) => b.updatedAt - a.updatedAt);
    },
    async get(id) {
      const store = (await transaction([DATA], "readonly")).objectStore(DATA);
      return ((await done(store.get(id))) as WebSave | undefined) ?? null;
    },
    async put(meta, save) {
      // 同一格已經是更新的版本（晚到的舊寫入）就不蓋；請求回呼裡丟錯會讓整筆交易撤銷
      await write((tx) => {
        const metas = tx.objectStore(META);
        const existing = metas.get(meta.id);
        existing.onsuccess = () => {
          const current = existing.result as SaveMeta | undefined;
          if (current && current.updatedAt > meta.updatedAt) return;
          metas.put(meta);
          tx.objectStore(DATA).put(save, meta.id);
        };
      });
    },
    async remove(id) {
      await write((tx) => {
        tx.objectStore(META).delete(id);
        tx.objectStore(DATA).delete(id);
      });
    },
    getGlobal: readGlobal,
    async loadGlobal(target) {
      const stored = await readGlobal();
      // 補鍵與標記載入在同一段同步程式：載入之後取樣的分頁一定含庫裡的鍵
      for (const [key, value] of Object.entries(stored)) {
        if (!(key in target)) target[key] = value;
      }
      globalLoaded = true;
    },
    reserveGlobalWrite() {
      if (!globalLoaded) return async () => {};
      const version = (globalReserved += 1);
      return (table) => {
        const run = globalQueue.then(async () => {
          if (version <= globalWritten) return;
          await write((tx) => tx.objectStore(GLOBALS).put(table, GLOBAL_KEY), [GLOBALS]);
          globalWritten = version;
        });
        globalQueue = run.catch(() => {});
        return run;
      };
    },
  };
}

/** 請瀏覽器把本站資料當成要保留的（不在空間吃緊時自動清）。回 null＝瀏覽器不支援。 */
export async function requestPersistence(storage: StorageManager | undefined = globalThis.navigator?.storage): Promise<boolean | null> {
  if (!storage?.persist) return null;
  try {
    return (await storage.persisted?.()) || (await storage.persist());
  } catch {
    return false;
  }
}

/**
 * 把存起來的跨對話 global 變數放回分頁（D29）：只補分頁沒有的鍵（開站時分頁是空的；重試時分頁已玩過的值
 * 比較新）。讀不到回 false：這段期間 global 只活在分頁、不寫回庫，畫面提示可重試。
 */
export async function restoreGlobals(store: SaveStore | null, target: Record<string, unknown>): Promise<boolean> {
  if (!store) return true;
  try {
    await store.loadGlobal(target);
    return true;
  } catch {
    return false;
  }
}

let counter = 0;
export const newSaveId = () => `s${Date.now().toString(36)}${(counter += 1).toString(36)}${Math.random().toString(36).slice(2, 6)}`;

/** Safari（含 iOS 上的瀏覽器）的 ITP 會在七天沒開本站後清掉網站資料。 */
export function isSafari(userAgent: string = globalThis.navigator?.userAgent ?? ""): boolean {
  return /Safari\//.test(userAgent) && !/Chrome\/|Chromium\/|Android/.test(userAgent);
}
