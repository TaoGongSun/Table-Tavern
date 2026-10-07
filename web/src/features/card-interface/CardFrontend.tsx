// 訊息裡的一個前端介面：沙盒 iframe 載入同站的 sandbox.html（自帶較寬的政策），宿主把組好的卡片文件
// postMessage 進去由它寫進自己的文件——不用 srcdoc，免得繼承宿主頁的嚴格 CSP（計畫 2.4）。
// 文件裡依序是內建全域庫（jQuery／lodash／errorCatched）、讀訊息、MVU、宿主橋接墊片，再來才是卡片自己的程式
// （桌面版 interface-card 的 buildShellDocument）。逐字稿或變數一變就推新快照，iframe 不重掛。
import { useCallback, useEffect, useRef, useState } from "react";
import { buildShellDocument, type CardStorage } from "@desktop/features/card-interface/interface-card";
import type { CardChat } from "@desktop/features/card-interface/card-chat-shim";
import type { CardMvu } from "@desktop/features/card-interface/mvu/card-mvu-shim";
import { t } from "../../i18n";
import type { FrontendHost } from "./frontend-host";

export const SANDBOX_URL = `${import.meta.env.BASE_URL}sandbox.html`;

function newToken(): string {
  const bytes = new Uint8Array(12);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * 一支 iframe 的 load 次數：第 1 次是 sandbox.html 載好（送文件進去），第 2 次是寫進去的文件載完
 * （document.open／close 照規格會再觸發一次）。之後再有 load 就是卡片把自己導向別頁或重新載入：
 * 新頁不能再收到推送、也不能再拿舊 token 送訊息，整支除名換一支新的。
 */
const LOADS_BEFORE_LIVE = 2;
const MAX_REMOUNTS = 3;

export function CardFrontend(props: {
  /** 程式碼區塊裡的整頁內容 */
  html: string;
  /** 這則訊息的樓號（讀訊息墊片的 getCurrentMessageId） */
  floor: number;
  chat: CardChat;
  mvu: CardMvu | null;
  /** 這桌存下的卡片設定：掛載時回填進沙盒的 localStorage */
  storage: CardStorage;
  host: FrontendHost;
}) {
  // 每次重掛都是新的 iframe 與新的 token
  const [mount, setMount] = useState(0);
  // 一直把自己導走的介面不無限重掛
  if (mount >= MAX_REMOUNTS) {
    return (
      <p className="message-flag" data-testid="card-frontend-navigated">
        {t("cardFrontendNavigated")}
      </p>
    );
  }
  return <SandboxFrame key={mount} {...props} onNavigated={() => setMount((value) => value + 1)} />;
}

function SandboxFrame({
  html,
  floor,
  chat,
  mvu,
  storage,
  host,
  onNavigated,
}: {
  html: string;
  floor: number;
  chat: CardChat;
  mvu: CardMvu | null;
  storage: CardStorage;
  host: FrontendHost;
  onNavigated: () => void;
}) {
  const frameRef = useRef<HTMLIFrameElement>(null);
  const [token] = useState(newToken);
  const [height, setHeight] = useState<number | null>(null);
  const loads = useRef(0);
  const unregister = useRef<(() => void) | null>(null);
  // 掛載當下的快照與設定：文件只組一次，之後的變動走推送
  const latest = useRef({ chat, mvu, storage });
  latest.current = { chat, mvu, storage };

  const post = useCallback(
    (message: Record<string, unknown>) =>
      frameRef.current?.contentWindow?.postMessage({ source: "table-tavern-host", token, ...message }, "*"),
    [token],
  );

  useEffect(() => {
    const view = frameRef.current?.contentWindow;
    if (!view) return;
    const stop = host.register(view, { token, floor, onHeight: setHeight });
    unregister.current = stop;
    return () => {
      stop();
      unregister.current = null;
    };
  }, [host, token, floor]);

  // 逐字稿或變數變了：推給已經寫好文件、還沒被導走的沙盒（讀訊息與 MVU 墊片各自核 token 與形狀）
  useEffect(() => {
    if (loads.current >= 1 && loads.current <= LOADS_BEFORE_LIVE && unregister.current !== null) post({ kind: "chat", chat, mvu });
  }, [post, chat, mvu]);

  const onLoad = () => {
    loads.current += 1;
    if (loads.current === 1) {
      const snapshot = latest.current;
      post({ kind: "load", html: buildShellDocument(html, snapshot.storage, { chat: snapshot.chat, token, mvu: snapshot.mvu }) });
      return;
    }
    if (loads.current > LOADS_BEFORE_LIVE) {
      unregister.current?.();
      unregister.current = null;
      onNavigated();
    }
  };

  return (
    <iframe
      ref={frameRef}
      className="card-frontend"
      data-testid="card-frontend"
      src={SANDBOX_URL}
      sandbox="allow-scripts"
      loading="lazy"
      title={t("cardFrontendTitle")}
      style={height === null ? undefined : { height }}
      onLoad={onLoad}
    />
  );
}
