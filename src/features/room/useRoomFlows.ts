// 首頁開房、重開、結束、回房間、關房後回大廳（room-host-desktop-play）。開房＝一定建新桌〔作者裁決 2026-11-03〕：
// 建桌 → 進桌 → 選了卡就照「匯入角色卡」同一條路逐張匯進這張桌（第二張起照常過路由框、開場旁白照常貼）→
// 全部匯完才綁新房間、換成房間畫面：房間的公開名冊與歷史只在第一次連上時送一次，要等桌子擺好〔作者裁決 2026-11-03〕。
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { readRoomStatus } from "./room-status";
import { enterRoom, useRoomHost } from "./useRoomHost";

interface Deps {
  table: string;
  busy: boolean;
  runTableOp: (op: () => Promise<void>) => Promise<void>;
  enterTable: (id: string) => Promise<unknown>;
  refreshWorlds: () => Promise<void>;
  setError: (message: string) => void;
  /** 匯入：同 CastRail 的「匯入角色卡」；路由框或身分框開著時先不匯下一張 */
  importFile: (file: File) => Promise<void>;
  importPending: boolean;
  /** 開場旁白的選擇框開著或正在貼 */
  openingPending: boolean;
  aiReady: boolean;
  openAiSettings: () => void;
}

/** 後端房間錯誤碼換成白話；其餘照原文 */
const roomError = (reason: unknown) => (String(reason).includes("ROOM_OTHER_TABLE") ? t("roomOtherTable") : String(reason));

export function useRoomFlows(deps: Deps) {
  const host = useRoomHost();
  const [dialog, setDialog] = useState<{ server: string } | null>(null);
  const [queue, setQueue] = useState<{ world: string; files: File[] } | null>(null);
  // 選了卡開房：匯完才開房
  const [pending, setPending] = useState<{ world: string; server: string } | null>(null);
  // 這次從開房／重開進來的桌：房間斷了（伺服器上被清、被取代）也留在房間畫面，讓它自己建新房或給「重新連線」
  const [shown, setShown] = useState<string | null>(null);
  // 正在跑的那一張（匯入函式本身回來為止；之後的身分框、路由框、開場旁白框由 importPending／openingPending 接手）
  const [importing, setImporting] = useState(false);

  // 選了卡：一張一張匯進新桌，跟開新桌匯入同一條路——上一張的對話框都收了才匯下一張；
  // 某張按取消或匯入失敗就少那張，照樣往下（跟一般匯入一樣不整批作廢）
  const { table, busy, importPending, openingPending, importFile } = deps;
  const settled = !busy && !importing && !importPending && !openingPending;
  useEffect(() => {
    if (!queue || queue.world !== table || !settled) return;
    const [file, ...rest] = queue.files;
    setQueue(rest.length > 0 ? { world: queue.world, files: rest } : null);
    setImporting(true);
    void importFile(file).finally(() => setImporting(false));
  }, [queue, table, settled, importFile]);

  // 全部匯完、對話框都收了才開房；中途離開這張桌就不開了（桌子留成一般桌），不延到下次回來
  useEffect(() => {
    if (!pending || busy) return;
    if (pending.world !== table) {
      setPending(null);
      setQueue(null);
      return;
    }
    if (queue || !settled) return;
    setPending(null);
    void bind(pending.world, pending.server);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pending, queue, table, busy, settled]);

  async function start() {
    if (host.activeWorld) {
      // 另一間房開著：一次只開一間，帶他回那間
      await deps.enterTable(host.activeWorld);
      return;
    }
    if (!deps.aiReady) {
      deps.setError(t("roomNeedAi"));
      deps.openAiSettings();
      return;
    }
    const status = await readRoomStatus().catch(() => null);
    setDialog({ server: status?.server ?? "" });
  }

  function open(files: File[], server: string) {
    setDialog(null);
    return deps.runTableOp(async () => {
      deps.setError("");
      try {
        const id = await invoke<string>("create_world", { name: t("roomTableName") });
        await deps.refreshWorlds();
        await deps.enterTable(id);
        if (files.length > 0) setQueue({ world: id, files });
        setPending({ world: id, server });
      } catch (reason) {
        deps.setError(roomError(reason));
      }
    });
  }

  /** 綁新房間、桌名換成房間碼、換成房間畫面；房間沒開成就把剛建的空桌收掉 */
  function bind(id: string, server: string) {
    return deps.runTableOp(async () => {
      try {
        const status = await enterRoom(id, server);
        // 房間桌用房間碼命名：不掛自動名（空桌回收不收它，Rust 端也另有一道）
        if (status.roomCode) {
          await invoke("rename_world", { worldId: id, newName: `${t("roomTableName")} ${status.roomCode}` }).catch(() => {});
        }
        await deps.refreshWorlds();
        await host.refresh();
        setShown(id);
      } catch (reason) {
        deps.setError(roomError(reason));
      }
    });
  }

  /** 重開房間：同一個房間碼接回；伺服器上已清掉的話房間畫面會建新房並告知 */
  function reopen(id: string) {
    return deps.runTableOp(async () => {
      deps.setError("");
      try {
        const saved = await readRoomStatus(id);
        await enterRoom(id, saved.server);
        await host.refresh();
        setShown(id);
        await deps.enterTable(id);
      } catch (reason) {
        deps.setError(roomError(reason));
      }
    });
  }

  /** 結束並刪除房間（桌卡選單）：伺服器上結束、清本機憑證；桌留著 */
  async function end(id: string) {
    const ok = await confirm(t("roomEndConfirm"), { title: t("roomEnd"), kind: "warning", okLabel: t("roomEnd"), cancelLabel: t("dialogCancel") });
    if (!ok) return;
    try {
      await invoke("room_close", { worldId: id });
    } catch (reason) {
      deps.setError(String(reason));
    }
    await host.refresh();
  }

  return {
    host,
    dialog,
    closeDialog: () => setDialog(null),
    start,
    open,
    reopen,
    end,
    /** 這張桌現在該顯示房間畫面 */
    isRoomTable: (id: string) => id !== "" && (host.activeWorld === id || shown === id),
    /** 關房（暫停）之後：不再顯示房間畫面 */
    paused: () => {
      setShown(null);
      void host.refresh();
    },
  };
}
