// Rust `room::Status` 的前端型別與輪詢：房間在 Rust 跑（執行端迴圈與房間畫面的成員連線），畫面只讀狀態。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { ExecutorErrorKind } from "../../../party/server/core/types";

export interface RoomStatus {
  /** 執行端在跑（房間開著） */
  active: boolean;
  /** 房間屬於哪一桌；沒有房間時 null */
  worldId: string | null;
  server: string;
  roomCode: string | null;
  inviteUrl: string | null;
  pairCode: string | null;
  /** 邀請連結多帶配對碼（拿房主用） */
  payerLink: string | null;
  online: boolean;
  keepAwake: boolean | null;
  modelLabel: string | null;
  lastError: string | null;
  /** `lastError` 是 GM 生成失敗時執行端分好的種類；其他錯誤為 null */
  lastErrorKind: ExecutorErrorKind | null;
  /** 伺服器上的房間已被清：重開會建新房 */
  gone: boolean;
  /** 允許玩家用 AI 生圖（這桌的房間設定，預設開；room-player-cards §3.7） */
  imageGen: boolean;
  /** 本機生圖來源是 API（房間生圖唯一路線）；不是就不顯示生圖開關 */
  imageGenRoute: boolean;
  /** 最近的玩家生圖（最多 20 筆、新的在後、只在記憶體）；`name` 是卡名 */
  genLog: Array<{ at: number; name: string; ok: boolean }>;
}

export const readRoomStatus = (worldId?: string) => invoke<RoomStatus>("room_status", { worldId: worldId ?? null });

/** 每 `ms` 讀一次；`worldId` 給了就在房間沒在跑時回那張桌留著的憑證摘要 */
export function useRoomStatus(worldId?: string, ms = 2000) {
  const [status, setStatus] = useState<RoomStatus | null>(null);
  const refresh = useCallback(async () => {
    try {
      setStatus(await readRoomStatus(worldId));
    } catch {
      // 讀不到就維持上一份
    }
  }, [worldId]);
  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), ms);
    return () => clearInterval(timer);
  }, [refresh, ms]);
  return { status, refresh };
}

/** 等狀態滿足條件（輪詢），逾時回 null */
export async function waitRoomStatus(test: (status: RoomStatus) => boolean, timeoutMs = 10_000, worldId?: string) {
  const until = Date.now() + timeoutMs;
  while (Date.now() < until) {
    const status = await readRoomStatus(worldId).catch(() => null);
    if (status && test(status)) return status;
    await new Promise((resolve) => setTimeout(resolve, 300));
  }
  return null;
}

/** 從 payerLink 取出配對碼原文（`&p=`） */
export const pairCodeOf = (link: string | null) => (link ? (new URL(link).hash.match(/[#&]p=([0-9A-Za-z]+)/)?.[1] ?? null) : null);
