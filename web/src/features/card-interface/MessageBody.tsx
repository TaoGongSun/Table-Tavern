// 一則訊息的內容：文字片段走宿主的安全渲染（DOMPurify＋擋外部圖片），前端介面片段畫成沙盒 iframe。
// 卡片介面網頁版畫不出來的卡（DRM、雲端載入器，判定同桌面版）不掛 iframe，只留一行說明。
import { lazy, Suspense, useMemo } from "react";
import type { CardStorage } from "@desktop/features/card-interface/card-storage";
import type { CardChat, ChatFloor } from "@desktop/features/card-interface/card-chat-shim";
import type { CardMvu } from "@desktop/features/card-interface/mvu/card-mvu-shim";
import { renderHostMarkdown } from "../../shared/ui/host-markdown";
import { t } from "../../i18n";
import { splitFrontends } from "./frontend-blocks";
import type { FrontendHost } from "./frontend-host";

// 沙盒文件要內嵌 jQuery、lodash：只有真的出現卡片介面時才載這一包
const CardFrontend = lazy(() => import("./CardFrontend").then((module) => ({ default: module.CardFrontend })));

/** 短指紋（djb2）：前端內容一換，iframe 的 key 就換 */
function fingerprint(text: string): string {
  let hash = 5381;
  for (let index = 0; index < text.length; index++) hash = ((hash << 5) + hash + text.charCodeAt(index)) | 0;
  return String(hash >>> 0);
}

export interface FrontendContext {
  host: FrontendHost;
  floors: ChatFloor[];
  /** 這樓的 MVU 快照（null＝這桌沒有 MVU） */
  mvuFor: (floor: number) => CardMvu | null;
  storage: CardStorage;
  /** 卡片介面網頁版畫不出來（DRM、雲端載入器） */
  unsupported: boolean;
}

export function MessageBody({ text, floor, frontends }: { text: string; floor: number; frontends: FrontendContext }) {
  const segments = useMemo(
    () =>
      splitFrontends(text).map((segment) =>
        segment.kind === "text" ? { ...segment, html: renderHostMarkdown(segment.text) } : { ...segment, html: segment.html },
      ),
    [text],
  );
  const chat = useMemo<CardChat>(() => ({ currentId: floor, floors: frontends.floors }), [floor, frontends.floors]);
  const mvu = useMemo(() => frontends.mvuFor(floor), [frontends, floor]);
  return (
    <>
      {segments.map((segment, index) =>
        segment.kind === "text" ? (
          <div key={index} className="message-body" dangerouslySetInnerHTML={{ __html: segment.html }} />
        ) : frontends.unsupported ? (
          <p key={index} className="message-flag" data-testid="card-frontend-unsupported">
            {t("cardFrontendUnsupported")}
          </p>
        ) : (
          // 內容一換（編輯、重新代換）就換一支新的 iframe
          <Suspense key={`${index}:${fingerprint(segment.html)}`} fallback={<div className="card-frontend" />}>
            <CardFrontend
              html={segment.html}
              floor={floor}
              chat={chat}
              mvu={mvu}
              storage={frontends.storage}
              host={frontends.host}
            />
          </Suspense>
        ),
      )}
    </>
  );
}
