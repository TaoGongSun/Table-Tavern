/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 端對端測試換成本機假端點；正式建置不設。 */
  readonly VITE_TT_OPENROUTER_API?: string;
  readonly VITE_TT_OPENROUTER_AUTH?: string;
  readonly VITE_TT_GITHUB_API?: string;
}

/** 建置時由 mvu-engine-plugin.ts 包出的 MVU updateVariables（桌面版沙盒同一份原始碼） */
declare module "virtual:mvu-engine" {
  export interface MvuEngineEnv {
    emit: (name: string, args: unknown[], isolate: boolean) => Promise<void>;
    events: Record<string, string>;
    lodash: unknown;
    /** 值解析請求送這裡（`{ kind: "mvu-eval", requestId, op, text }`），結果經 resolveEval 回來 */
    parentRef: { postMessage: (message: Record<string, unknown>, target: string) => void };
    token: string;
    store: { macros: { user: string; char: string | null } | null };
  }
  export interface MvuEngine {
    parseMessage: (message: string, oldData: Record<string, unknown>) => Promise<Record<string, unknown>>;
    resolveEval: (data: { requestId: unknown; ok: boolean; value?: unknown; error?: unknown }) => void;
    generateSchema: (data: unknown, oldNode: unknown) => Record<string, unknown>;
    cleanUpMetadata: (data: unknown) => void;
  }
  export function createMvuEngine(env: MvuEngineEnv): MvuEngine;
}
