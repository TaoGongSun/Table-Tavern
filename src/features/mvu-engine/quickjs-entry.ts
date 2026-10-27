// 桌面 QuickJS 宿主的 bundle 入口（mvu-zod-semantics 規格 1.2、2.3 節）：`npm run build:mvu-bundle` 打成
// src-tauri/gen/mvu-engine.js（IIFE、全域名 TTMvu），由 src-tauri/src/mvu_engine/runtime.rs 求值後呼叫 `TTMvu.boot()`。
// 與 Rust 之間一律傳 JSON 字串。值解析在同一個 context 同步跑：QuickJS 沒有計時器，期限由 Rust 的中斷處理器守。
import lodash from "lodash";
import { z } from "zod";
import { buildMvuEngineSource } from "./engine-source";
import { MVU_EVENTS, type MvuCore } from "./host-contract";
import { isEvalOp, runEval } from "./value-parser";

export interface DesktopEngine {
  /** 墊片模組（卡片 import 的 mvu_zod）轉呼叫 registerMvuSchema：只認第一份 */
  register: (input: unknown) => void;
  /** zodStatus() 的 JSON */
  status: () => string;
  /** parseMessageDetailed：回契約（host-contract.ts 的 MvuDetailedResult）的 JSON；整次失敗丟錯 */
  parse: (message: string, oldJson: string, macrosJson: string) => Promise<string>;
  /** 開局：initvar 先 cleanUpMetadata 再過 schema；回 `{ outcome, stat_data }` 的 JSON */
  initialize: (statJson: string) => string;
}

/** UTF-8 位元組長度（不配置緩衝區） */
export function utf8Length(text: string): number {
  let bytes = 0;
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    if (code < 0x80) bytes += 1;
    else if (code < 0x800) bytes += 2;
    else if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
      const next = text.charCodeAt(i + 1);
      if (next >= 0xdc00 && next <= 0xdfff) {
        bytes += 4;
        i++;
      } else bytes += 3;
    } else bytes += 3;
  }
  return bytes;
}

/** QuickJS 沒有 TextEncoder（value-parser.ts 直接 new TextEncoder()）：只做 UTF-8 encode */
class Utf8Encoder {
  readonly encoding = "utf-8";
  encode(input = ""): Uint8Array {
    const text = String(input);
    const out = new Uint8Array(utf8Length(text));
    let at = 0;
    for (let i = 0; i < text.length; i++) {
      let code = text.charCodeAt(i);
      if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
        const next = text.charCodeAt(i + 1);
        if (next >= 0xdc00 && next <= 0xdfff) {
          code = 0x10000 + ((code - 0xd800) << 10) + (next - 0xdc00);
          i++;
        }
      }
      if (code >= 0xd800 && code <= 0xdfff) code = 0xfffd;
      if (code < 0x80) out[at++] = code;
      else if (code < 0x800) {
        out[at++] = 0xc0 | (code >> 6);
        out[at++] = 0x80 | (code & 0x3f);
      } else if (code < 0x10000) {
        out[at++] = 0xe0 | (code >> 12);
        out[at++] = 0x80 | ((code >> 6) & 0x3f);
        out[at++] = 0x80 | (code & 0x3f);
      } else {
        out[at++] = 0xf0 | (code >> 18);
        out[at++] = 0x80 | ((code >> 12) & 0x3f);
        out[at++] = 0x80 | ((code >> 6) & 0x3f);
        out[at++] = 0x80 | (code & 0x3f);
      }
    }
    return out;
  }
}

/** QuickJS 的資源類錯誤：記憶體不足（InternalError；連錯誤物件都建不出來時丟 null）、堆疊溢位。這些在 QuickJS
 *  可被 catch，交給核心的 host.fatal 判定後照拋、整次失敗。已知限制：卡片自己 throw null 也算 */
export function isQuickJsFatal(error: unknown): boolean {
  if (error === null || error === undefined) return true;
  if (typeof error !== "object") return false;
  const { name, message } = error as { name?: unknown; message?: unknown };
  if (name === "InternalError") return true;
  return name === "RangeError" && typeof message === "string" && message.includes("call stack");
}

type Macros = { user: string; char: string | null } | null;

/** 建一份核心與 Rust 用的介面；不碰全域（vitest 直接拿來對照） */
export function createDesktopEngine(): DesktopEngine {
  const createMvuCore = new Function(`${buildMvuEngineSource()}\nreturn createMvuCore;`)() as (env: unknown) => MvuCore;
  // 卡片腳本與核心同一個 realm：輸出入用開機時抓住的 JSON，卡片之後改全域 JSON 也不影響
  const { stringify, parse } = JSON;
  let macros: Macros = null;
  const core = createMvuCore({
    z,
    lodash,
    events: MVU_EVENTS,
    emit: async () => {},
    host: {
      evaluate: (op: unknown, text: unknown, done: (reply: unknown) => void) => {
        done(isEvalOp(op) && typeof text === "string" ? runEval(op, text) : { ok: false, error: "bad-request" });
        return null;
      },
      utf8Length,
      macros: () => macros,
      fatal: isQuickJsFatal,
    },
  });
  return {
    register: (input) => core.registerMvuSchema(input),
    status: () => stringify(core.zodStatus()),
    async parse(message, oldJson, macrosJson) {
      macros = parse(macrosJson) as Macros;
      try {
        return stringify(await core.parseMessageDetailed(message, parse(oldJson) as Record<string, unknown>));
      } finally {
        macros = null;
      }
    },
    initialize(statJson) {
      const stat: unknown = parse(statJson);
      core.cleanUpMetadata(stat);
      const variables: Record<string, unknown> = { stat_data: stat };
      const outcome = core.zodInitialize(variables);
      return stringify({ outcome, stat_data: variables.stat_data });
    },
  };
}

/** QuickJS 開機：補宿主全域（TextEncoder、安靜的 console），放卡片 schema 腳本要用的 z、_、$，回核心介面 */
export function boot(): DesktopEngine {
  const scope = globalThis as unknown as Record<string, unknown>;
  if (typeof scope.TextEncoder !== "function") scope.TextEncoder = Utf8Encoder;
  const quiet = () => {};
  scope.console = { log: quiet, info: quiet, warn: quiet, error: quiet, debug: quiet };
  scope.z = z;
  scope._ = lodash;
  // 上游是 jQuery 的 $(fn)：頁面就緒後執行。這裡沒有頁面，直接執行
  scope.$ = (fn: unknown) => {
    if (typeof fn === "function") fn();
  };
  return createDesktopEngine();
}
