// 宿主端的 MVU：updateVariables（建置時從桌面版沙盒那份原始碼包出來，見 web/mvu-engine-plugin.ts）、
// 初始化時的 schema 生成與元資料清理。值解析交給 Worker（evaluate.ts）。
import lodash from "lodash";
import { createMvuEngine, type MvuEngine } from "virtual:mvu-engine";
import type { Evaluator } from "./evaluate";

/** MVU 事件名（MagVarUpdate variable_def.ts）；宿主這邊沒有監聽器，只是片段要用到 */
const EVENTS = {
  VARIABLE_INITIALIZED: "mag_variable_initialized",
  VARIABLE_UPDATE_STARTED: "mag_variable_update_started",
  COMMAND_PARSED: "mag_command_parsed",
  VARIABLE_UPDATE_ENDED: "mag_variable_update_ended",
  BEFORE_MESSAGE_UPDATE: "mag_before_message_update",
  SINGLE_VARIABLE_UPDATED: "mag_variable_updated",
};

export interface MvuMacros {
  user: string;
  char: string | null;
}

export type MvuTable = Record<string, unknown>;

export interface HostMvu {
  /** MVU updateVariables：深拷貝 old 後照訊息裡的指令更新、回新資料（超出上限丟錯） */
  parseMessage: (message: string, old: MvuTable) => Promise<MvuTable>;
  /** 初始化：由帶 `$meta` 的 stat_data 生成 schema（含 strictTemplate 等旗標），再清掉元資料 */
  initializeSchema: (stat: MvuTable, oldSchema: unknown) => { stat: MvuTable; schema: MvuTable };
}

export function createHostMvu(evaluator: Evaluator, macros: MvuMacros): HostMvu {
  let engine: MvuEngine | null = null;
  const parentRef = {
    postMessage: (message: Record<string, unknown>) => {
      const { requestId, op, text } = message;
      if ((op !== "value" && op !== "patch") || typeof text !== "string") {
        engine?.resolveEval({ requestId, ok: false, error: "bad-request" });
        return;
      }
      void evaluator
        .run(op, text)
        .catch((reason: unknown) => ({ ok: false as const, error: `host-error: ${String(reason)}` }))
        .then((outcome) => engine?.resolveEval({ requestId, ...outcome }));
    },
  };
  engine = createMvuEngine({
    emit: async () => {},
    events: EVENTS,
    lodash,
    parentRef,
    token: "host",
    store: { macros },
  });
  const created = engine;
  return {
    parseMessage: (message, old) => created.parseMessage(message, old),
    initializeSchema(stat, oldSchema) {
      const schema = created.generateSchema(lodash.cloneDeep(stat), oldSchema);
      const meta = lodash.isPlainObject(stat.$meta) ? (stat.$meta as Record<string, unknown>) : {};
      if (schema.type === "object") {
        for (const flag of ["strictTemplate", "concatTemplateArray", "strictSet"]) {
          if (lodash.has(meta, flag)) schema[flag] = meta[flag];
        }
      }
      created.cleanUpMetadata(stat);
      return { stat, schema };
    },
  };
}
