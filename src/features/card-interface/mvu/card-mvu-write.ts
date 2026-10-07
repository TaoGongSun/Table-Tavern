// 卡片沙盒寫入的宿主端（計畫 8.5、8.8）：上限驗證、依寫入目標排隊、Promise 結算與被拒時推回權威值。
// 純邏輯，送後端、更新逐字稿、回覆沙盒都由 controller 注入。
import { type TranscriptEvent } from "../../../shared/contracts/backend-contracts";
import { tableTextProblem } from "../../../shared/contracts/vars-table";

/** 後端 `card_vars_write` 的回傳（Rust `CardWrite`）；`gone`＝宿主自己判的：await 回來時桌、桌世代或幕已換，
 *  結果不屬於現在這桌，不換進逐字稿、不推權威值 */
export type CardWriteResult =
  | { status: "ok"; event: TranscriptEvent }
  | { status: "layer_ok"; rev: string }
  | { status: "stale" | "busy"; found: boolean; rev: string | null; table: string | null }
  | { status: "rejected"; code: string; found: boolean; rev: string | null; table: string | null }
  | { status: "gone" };

/** 沙盒送來的一筆寫入 */
export interface MvuWriteRequest {
  requestId: string;
  /** 寫入目標：事件 id，舊事件是 "@逐字稿位置" */
  target: string;
  /** 整張新表（JSON 文字，保留鍵順序） */
  payload: string;
  /** 沙盒這張表是以哪一版為底（null＝那樓原本沒有表）：同目標鏈的第一筆以它當預期版本 */
  base: string | null;
  /** 產生這張表時快照的桌世代與幕 */
  generation: number;
  scene: number;
}

export type LayerName = "chat" | "character" | "global" | "preset" | "script" | "extension";

/** 非 message 層的寫入目標 key（`chat`、`global`、`preset`、`character:<id>`、`script:<原 ID>`、`extension:<原 ID>`）；
 *  事件 key（事件 id 或 "@位置"）回 null */
export function parseLayerKey(key: string): { layer: LayerName; id: string | null } | null {
  if (key === "chat" || key === "global" || key === "preset") return { layer: key, id: null };
  const match = /^(character|script|extension):([\s\S]+)$/.exec(key);
  return match === null ? null : { layer: match[1] as LayerName, id: match[2] };
}

/** 這個非 message 層現在不能寫：還沒讀回，或讀取失敗（該層或它所在的 script／extension 類別標了 error） */
export function layerBlocked(layers: Record<string, { error?: string }> | null, key: string): boolean {
  if (layers === null) return true;
  if (layers[key]?.error !== undefined) return true;
  const category = /^(script|extension):/.exec(key);
  return category !== null && layers[`${category[1]}:`]?.error !== undefined;
}

export interface SettleResult {
  requestId: string;
  ok: boolean;
  /** 確認落檔的版本 */
  rev?: string;
  error?: string;
}

/** 被拒時推回沙盒的目標權威值；table null＝那樓沒有表 */
export interface Authority {
  key: string;
  table: Record<string, unknown> | null;
  rev: string | null;
}

/** 舊事件（"@位置"）第一次寫入後配到 id：沙盒把這個目標的本地值、在途筆數與版本搬到新 key */
export interface Migration {
  from: string;
  to: string;
}

/** 驗一張要寫入的表；不符回原因代碼（整批拒絕），符合回 null。上限與後端同一組（shared/contracts/vars-table）。 */
export function validateTable(payload: string): string | null {
  return tableTextProblem(payload);
}

function parseTable(text: string | null): Record<string, unknown> | null {
  if (text === null) return null;
  try {
    const value: unknown = JSON.parse(text);
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

export interface MvuWriteQueue {
  enqueue: (request: MvuWriteRequest) => void;
  /** 換殼、切桌、關面板、token 失效：未結算的請求一律 reject（closed）。在飛那筆的後端結果照實落檔，不推給舊殼 */
  close: () => void;
}

/**
 * 依寫入目標各自排隊：同一目標同時只送一筆；在飛時的新寫入只更新樂觀值（整張表），回來後以最新樂觀值再送。
 * 某目標的一版提交成功，不晚於它的請求一起 resolve；被拒時在飛那批與其後未送的一起 reject，並推回權威值。
 * 不同目標互不覆蓋、互不代結算。
 */
export function createMvuWriteQueue(deps: {
  /** 目標現在的權威版本（宿主手上的逐字稿，只給本地驗證失敗推回權威值用）；undefined＝目標已不在 */
  revOf: (key: string) => string | null | undefined;
  /** 目標現在的權威表（本地驗證失敗時推回用） */
  tableOf: (key: string) => Record<string, unknown> | null;
  send: (
    key: string,
    expectedRev: string | null,
    payload: string,
    identity: { generation: number; scene: number },
  ) => Promise<CardWriteResult>;
  /** 後端確認落檔的那則（key＝寫入目標，舊事件的 "@位置" 也認得出原事件）與這條寫入鏈的身分：由宿主核對身分
   *  沒變才換進逐字稿（關掉之後也照這個規則） */
  committed: (key: string, event: TranscriptEvent, identity: { generation: number; scene: number }) => void;
  /** 非 message 層（key 不是事件）確認落檔：新版本與落檔的整張表（JSON 文字），宿主更新自己手上的現況 */
  layerCommitted: (key: string, rev: string, vars: string, identity: { generation: number; scene: number }) => void;
  /** 回覆沙盒 */
  settle: (results: SettleResult[], authority?: Authority, migrate?: Migration) => void;
}): MvuWriteQueue {
  interface TargetState {
    /** 樂觀值：最新一筆還沒送出的整張表 */
    optimistic: string | null;
    /** 樂觀值涵蓋、還沒送出的請求 */
    unsent: string[];
    /** 在飛那一批的請求；null＝沒有在飛 */
    inflight: string[] | null;
    /** 在飛期間有本地驗證失敗：回來後要推一次權威值 */
    resync: boolean;
    /** 這條寫入鏈的預期版本與身分：鏈的第一筆取沙盒給的底版，之後每次確認換成新版本 */
    chain: { rev: string | null; generation: number; scene: number } | null;
  }
  const targets = new Map<string, TargetState>();
  // 舊事件 "@位置" → 第一次寫入後配到的 id：之後送來的同目標寫入（沙盒還沒收到搬家通知）一律改寫到 id
  const aliases = new Map<string, string>();
  let closed = false;

  const stateOf = (key: string) => {
    let state = targets.get(key);
    if (!state) {
      state = { optimistic: null, unsent: [], inflight: null, resync: false, chain: null };
      targets.set(key, state);
    }
    return state;
  };

  const settle = (results: SettleResult[], authority?: Authority, migrate?: Migration) => {
    if (!closed && results.length + (authority ? 1 : 0) > 0) deps.settle(results, authority, migrate);
  };

  const flush = async (key: string) => {
    const state = stateOf(key);
    if (state.inflight !== null || state.optimistic === null || closed) return;
    const ids = state.unsent;
    const payload = state.optimistic;
    state.unsent = [];
    state.optimistic = null;
    state.inflight = ids;
    const chain = state.chain!;
    let result: CardWriteResult;
    try {
      result = await deps.send(key, chain.rev, payload, { generation: chain.generation, scene: chain.scene });
    } catch (reason) {
      result = { status: "rejected", code: `error: ${String(reason)}`, found: false, rev: null, table: null };
    }
    state.inflight = null;
    if (result.status === "gone") {
      // 結果不屬於現在這桌：在飛與未送的一起 stale，不推權威值（那是別桌或換過世代的資料）
      const rejectedIds = [...ids, ...state.unsent];
      state.unsent = [];
      state.optimistic = null;
      state.resync = false;
      state.chain = null;
      settle(rejectedIds.map((requestId) => ({ requestId, ok: false, error: "stale" })));
      return;
    }
    if (result.status === "ok" || result.status === "layer_ok") {
      const identity = { generation: chain.generation, scene: chain.scene };
      const rev = result.status === "ok" ? result.event.vars_rev : result.rev;
      if (result.status === "ok") deps.committed(key, result.event, identity);
      else deps.layerCommitted(key, result.rev, payload, identity);
      chain.rev = rev ?? null;
      // 舊事件第一次寫入配到 id：整條寫入鏈（含排在後面的）搬到新 id，沙盒同步搬家
      let current = key;
      let migrate: Migration | undefined;
      const id = result.status === "ok" ? result.event.id : undefined;
      if (key.startsWith("@") && typeof id === "string" && id !== key) {
        targets.delete(key);
        targets.set(id, state);
        aliases.set(key, id);
        current = id;
        migrate = { from: key, to: id };
      }
      settle(
        ids.map((requestId) => ({ requestId, ok: true, rev })),
        undefined,
        migrate,
      );
      if (state.resync) {
        state.resync = false;
        settle([], { key: current, table: deps.tableOf(current), rev: deps.revOf(current) ?? null });
      }
      if (state.unsent.length === 0) state.chain = null;
      void flush(current);
      return;
    }
    const code = result.status === "rejected" ? result.code : result.status;
    const rejectedIds = [...ids, ...state.unsent];
    state.unsent = [];
    state.optimistic = null;
    state.resync = false;
    state.chain = null;
    settle(
      rejectedIds.map((requestId) => ({ requestId, ok: false, error: code })),
      { key, table: result.found ? parseTable(result.table) : null, rev: result.rev },
    );
  };

  return {
    enqueue(incoming) {
      if (closed) {
        deps.settle([{ requestId: incoming.requestId, ok: false, error: "closed" }]);
        return;
      }
      const request = { ...incoming, target: aliases.get(incoming.target) ?? incoming.target };
      const target = request.target;
      const state = stateOf(target);
      const problem = validateTable(request.payload);
      if (problem !== null) {
        // 這筆與排在它前面還沒送的一起拒絕（樂觀值已含這筆）；在飛那批照常等，回來後再推權威值
        const rejectedIds = [...state.unsent, request.requestId];
        state.unsent = [];
        state.optimistic = null;
        if (state.inflight === null) state.chain = null;
        const results = rejectedIds.map((requestId) => ({ requestId, ok: false, error: problem }));
        if (state.inflight === null) {
          settle(results, {
            key: request.target,
            table: deps.tableOf(request.target),
            rev: deps.revOf(request.target) ?? null,
          });
        } else {
          state.resync = true;
          settle(results);
        }
        return;
      }
      if (state.chain === null) {
        state.chain = { rev: request.base, generation: request.generation, scene: request.scene };
      } else if (state.chain.generation !== request.generation || state.chain.scene !== request.scene) {
        settle([{ requestId: request.requestId, ok: false, error: "stale" }]);
        return;
      }
      state.unsent.push(request.requestId);
      state.optimistic = request.payload;
      void flush(request.target);
    },
    close() {
      if (closed) return;
      const results: SettleResult[] = [];
      targets.forEach((state) => {
        for (const requestId of [...(state.inflight ?? []), ...state.unsent]) {
          results.push({ requestId, ok: false, error: "closed" });
        }
        state.unsent = [];
        state.optimistic = null;
      });
      if (results.length > 0) deps.settle(results);
      closed = true;
    },
  };
}
