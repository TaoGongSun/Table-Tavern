// 一桌的卡片介面宿主：掛一個訊息監聽器收所有卡片 iframe 的訊息（形狀與來源核對在 frontend-host.ts），
// 並把逐字稿整理成讀訊息墊片要的每一樓。
import { useEffect, useMemo, useRef } from "react";
import type { ChatFloor } from "@desktop/features/card-interface/card-chat-shim";
import type { ChatEntry } from "../chat/chat-turn";
import { createFrontendHost, type FrontendHost, type FrontendHostDeps } from "./frontend-host";

export function useFrontendHost(deps: FrontendHostDeps): FrontendHost {
  const depsRef = useRef(deps);
  depsRef.current = deps;
  const host = useMemo(() => createFrontendHost(depsRef), []);
  useEffect(() => {
    const listener = (event: MessageEvent) => host.handle(event);
    window.addEventListener("message", listener);
    return () => window.removeEventListener("message", listener);
  }, [host]);
  return host;
}

/** 交給卡片的每一樓：酒館存的訊息原文（存檔的 `text`），玩家樓用玩家名、角色樓用卡名。 */
export function chatFloors(entries: ChatEntry[], userName: string, charName: string): ChatFloor[] {
  return entries.map((entry) => ({
    name: entry.role === "user" ? userName : charName,
    role: entry.role === "user" ? "user" : "assistant",
    message: entry.text,
  }));
}
