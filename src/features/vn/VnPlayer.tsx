// VN 桌的播放器：舞台＋對話框＋選項／繼續＋工具列（auto、skip、log、停止、聊天視圖）。
// 選項、換一組、繼續共用一把同步鎖：按下當下佔用，送出或換組連同其後的正式格重抓都完成才放。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { t, type MsgKey } from "../../i18n";
import type { ChatController } from "../play/chat/useChatController";
import { backendText } from "../../shared/ui/backend-text";
import { IconHome, IconStop } from "../../shared/ui/icons";
import { Backlog } from "./Backlog";
import { ChoicePanel } from "./ChoicePanel";
import { DialogueBox } from "./DialogueBox";
import { Stage } from "./Stage";
import { useVnCells } from "./useVnCells";
import { useSprites } from "./useSprites";
import { backdropKey, useBackgrounds } from "./useBackgrounds";
import { useVnPlayback } from "./useVnPlayback";
import { rerollError, stageAt } from "./vn-cells";
import "./vn-player.css";

const REROLL_MESSAGES: Record<string, MsgKey> = {
  busy: "vnRerollBusy",
  stale: "vnRerollStale",
  failed: "vnRerollFailed",
  unavailable: "vnRerollUnavailable",
};

/** 實測量時間用（方案 §7）：測試通道讀 performance.getEntriesByType("mark") */
function mark(name: string, detail?: unknown) {
  if (typeof performance !== "undefined" && typeof performance.mark === "function") {
    performance.mark(name, { detail });
  }
}

export function VnPlayer({
  worldId,
  scene,
  tableName,
  chat,
  images,
  notice,
  busy,
  onError,
  onShowChat,
  onGoLobby,
}: {
  worldId: string;
  scene: number;
  tableName: string;
  /** App 層包好鎖的聊天控制器（lockedChat） */
  chat: ChatController;
  /** 角色 id → 角色卡原圖，當立繪佔位 */
  images: Record<string, string>;
  notice: ReactNode;
  /** 生成中或進出桌進行中 */
  busy: boolean;
  onError: (message: string) => void;
  onShowChat: () => void;
  onGoLobby: () => void;
}) {
  const realignRef = useRef<ReturnType<typeof useVnPlayback>["realign"]>(() => {});
  const vn = useVnCells({
    worldId,
    scene,
    events: chat.events,
    generating: chat.generating,
    streamText: chat.streamText,
    onLanded: (...args) => realignRef.current(...args),
    onError,
  });
  const play = useVnPlayback({
    cells: vn.cells,
    lastEventKey: vn.lastEventKey,
    tail: vn.tail,
    generating: chat.generating !== null,
  });
  realignRef.current = play.realign;
  const stage = useMemo(() => stageAt(vn.cells, play.flatIndex), [vn.cells, play.flatIndex]);
  const sprites = useSprites(worldId);
  const backgrounds = useBackgrounds(worldId);
  // 正式格有新事件落檔就排入這幕缺的背景（串流暫定格不排：中斷的回合不替玩家花額度）
  useEffect(() => {
    invoke("vn_backgrounds_fill", { worldId, scene }).catch(() => {});
  }, [worldId, scene, vn.lastEventKey]);
  const backdrop = stage.backdrop;
  const backdropId = backdrop ? backdropKey(backdrop) : null;
  const { load } = backgrounds;
  useEffect(() => {
    if (backdrop) load(backdrop);
  }, [backdrop, load]);
  const backdropState = backdropId ? backgrounds.status[backdropId]?.state : undefined;
  const [logOpen, setLogOpen] = useState(false);
  const [rerollMessage, setRerollMessage] = useState<string | null>(null);

  const lock = useRef(false);
  const [locked, setLocked] = useState(false);
  async function runLocked(work: () => Promise<void>) {
    if (lock.current) return;
    lock.current = true;
    setLocked(true);
    try {
      await work();
    } finally {
      lock.current = false;
      setLocked(false);
    }
  }

  const turnStart = useRef(false);
  const startTurn = (send: () => Promise<void>) =>
    runLocked(async () => {
      setRerollMessage(null);
      turnStart.current = true;
      mark("vn:turn-start");
      play.expectNext();
      await send();
    });

  const reroll = () =>
    runLocked(async () => {
      setRerollMessage(null);
      try {
        await invoke<string[]>("vn_reroll_choices", { worldId });
        await chat.reload();
        await vn.refresh();
      } catch (reason) {
        const code = rerollError(reason);
        setRerollMessage(code ? t(REROLL_MESSAGES[code]) : backendText(reason));
      }
    });

  // 量測點：回合開始後第一格、角色接力的第一格（gapMs＝玩家已在等的時間）、選項面板出現
  const waitingSince = useRef<number | null>(null);
  useEffect(() => {
    waitingSince.current = play.end === "waiting" ? performance.now() : null;
  }, [play.end]);
  const lastPlayable = play.playable[play.playable.length - 1];
  useEffect(() => {
    if (turnStart.current && lastPlayable?.kind === "pending") {
      turnStart.current = false;
      mark("vn:first-cell");
    }
  }, [lastPlayable]);
  const relayMarked = useRef<object | null>(null);
  useEffect(() => {
    const who = chat.generating;
    if (!who || who.id === "" || !vn.streaming || relayMarked.current === who) return;
    relayMarked.current = who;
    const since = waitingSince.current;
    mark("vn:relay-first-cell", { gapMs: since === null ? 0 : performance.now() - since });
  }, [chat.generating, vn.streaming]);
  useEffect(() => {
    if (play.end === "choices") mark("vn:choices-ready");
  }, [play.end, play.current?.key]);

  const { skip, setSkip, advance } = play;
  const onAdvance = useCallback(() => {
    if (skip) setSkip(false);
    else advance();
  }, [skip, setSkip, advance]);

  const actionsDisabled = locked || busy;
  const onChoices = play.current?.cell.type === "choices" ? play.current.cell : null;
  const choices = play.end === "choices" ? onChoices : null;
  // 停在選項格時對話框留著前一格文字
  const shownCell = onChoices ? play.playable[play.playable.indexOf(play.current!) - 1] : play.current;
  return (
    <main className="vn-player">
      <header className="vn-toolbar">
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          aria-label={t("backToLobby")}
          title={t("backToLobby")}
          disabled={busy}
          onClick={onGoLobby}
        >
          <IconHome />
        </button>
        <h1>{tableName}</h1>
        <span className="toolbar-spacer" />
        {chat.canStop && (
          <button type="button" className="btn" onClick={chat.stopResponse}>
            <IconStop />
            {t("stopResponse")}
          </button>
        )}
        <button
          type="button"
          className="btn btn-ghost"
          aria-pressed={play.auto}
          onClick={() => play.setAuto(!play.auto)}
        >
          {t("vnAuto")}
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          aria-pressed={skip}
          onClick={() => setSkip(!skip)}
        >
          {t("vnSkip")}
        </button>
        {backdrop && (
          <button
            type="button"
            className="btn btn-ghost"
            disabled={backdropState === "queued" || backdropState === "running"}
            onClick={() => void backgrounds.regenerate(backdrop).catch((reason) => onError(backendText(reason)))}
          >
            {t("vnBackgroundRegenerate")}
          </button>
        )}
        <button type="button" className="btn btn-ghost" onClick={() => setLogOpen(true)}>
          {t("vnLog")}
        </button>
        <button type="button" className="btn btn-ghost" onClick={onShowChat}>
          {t("vnShowChat")}
        </button>
      </header>
      {notice}
      <Stage
        stage={stage}
        sprites={sprites}
        images={images}
        backdrop={backdropId ? backgrounds.images[backdropId] : undefined}
        cellKey={play.current?.key ?? ""}
      />
      <div className="vn-bottom">
        {choices && (
          <ChoicePanel
            options={choices.options}
            disabled={actionsDisabled}
            error={rerollMessage}
            onChoose={(option) => void startTurn(() => chat.submitText(option))}
            onReroll={() => void reroll()}
          />
        )}
        {play.end === "continue" && (
          <button
            type="button"
            className="btn btn-primary vn-continue"
            disabled={actionsDisabled}
            onClick={() => void startTurn(chat.gmAdvance)}
          >
            {t("vnContinue")}
          </button>
        )}
        <DialogueBox
          cell={shownCell}
          shown={onChoices ? Infinity : play.shown}
          waiting={play.end === "waiting"}
          ready={!play.typing && !play.tailing && play.end === null}
          onAdvance={onAdvance}
        />
      </div>
      {logOpen && (
        <Backlog cells={vn.cells.slice(0, play.flatIndex + 1)} onClose={() => setLogOpen(false)} />
      )}
    </main>
  );
}
