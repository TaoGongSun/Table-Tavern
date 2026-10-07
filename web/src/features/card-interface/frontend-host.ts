// 宿主收卡片沙盒訊息的唯一入口（計畫 2.4）：來源必須是不透明來源（event.origin 是 "null"）、來源視窗是目前掛著的
// iframe、文件 token 相符、資料形狀對，任一不符就丟。只能靠來源視窗與 token 認人；iframe 一卸載（含被導向別頁後
// 重掛）就除名，舊 iframe 殘留的訊息與外部視窗偽造的訊息都進不來。每次掛載都是新的 iframe 與新 token。
import { sanitizeCardStorage, type CardStorage } from "@desktop/features/card-interface/card-storage";

/** 一支掛著的卡片 iframe */
export interface FrameRecord {
  /** 這份文件的 token：推送與需要認文件的訊息都帶它 */
  token: string;
  /** 這支 iframe 屬於第幾則訊息（樓號） */
  floor: number;
  onHeight: (height: number) => void;
}

/** 卡片寫 MVU 變數、要求值解析：形狀已核過、token 相符才交過來，回覆由呼叫端送回同一個視窗 */
export interface MvuChannel {
  write: (frame: FrameRecord, data: Record<string, unknown>, reply: (message: Record<string, unknown>) => void) => void;
  evaluate: (frame: FrameRecord, data: Record<string, unknown>, reply: (message: Record<string, unknown>) => void) => void;
}

export interface FrontendHostDeps {
  /** 卡片按鈕送出的玩家句（酒館的送出框誘餌、triggerSlash('/send …')） */
  onInput: (text: string) => void;
  /** 卡片存設定（沙盒 localStorage 的整份快照），已過上限與型別檢查 */
  onStorage: (entries: CardStorage) => void;
  mvu?: MvuChannel;
}

/** iframe 最高多高（px）：卡片回報什麼都夾在這裡面 */
export const MAX_FRAME_HEIGHT = 20_000;

export interface FrontendHost {
  /** 掛上一支 iframe；回傳除名函式（卸載時呼叫） */
  register: (source: MessageEventSource, record: FrameRecord) => () => void;
  handle: (event: MessageEvent) => void;
}

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

export function createFrontendHost(depsRef: { current: FrontendHostDeps }): FrontendHost {
  const frames = new Map<MessageEventSource, FrameRecord>();
  return {
    register(source, record) {
      frames.set(source, record);
      return () => {
        if (frames.get(source) === record) frames.delete(source);
      };
    },
    handle(event) {
      const source = event.source;
      const frame = source === null ? undefined : frames.get(source);
      if (frame === undefined || source === null || event.origin !== "null") return;
      const data: unknown = event.data;
      if (!isObject(data) || data.source !== "table-tavern-card" || data.token !== frame.token) return;
      const deps = depsRef.current;
      const reply = (message: Record<string, unknown>) =>
        (source as Window).postMessage({ source: "table-tavern-host", token: frame.token, ...message }, { targetOrigin: "*" });
      switch (data.kind) {
        case "height":
          if (typeof data.height === "number" && Number.isFinite(data.height)) {
            frame.onHeight(Math.min(Math.max(Math.ceil(data.height), 0), MAX_FRAME_HEIGHT));
          }
          return;
        case "storage": {
          const entries = sanitizeCardStorage(data.entries);
          if (entries !== null) deps.onStorage(entries);
          return;
        }
        case "input":
          if (typeof data.text === "string") deps.onInput(data.text);
          return;
        case "mvu-write":
          deps.mvu?.write(frame, data, reply);
          return;
        case "mvu-eval":
          deps.mvu?.evaluate(frame, data, reply);
          return;
        default:
          // close（Esc）等桌面版覆蓋層才用得到的訊息：內嵌在訊息裡的介面不理
          return;
      }
    },
  };
}
