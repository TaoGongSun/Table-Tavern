// VN 桌的播放器：舞台＋對話框＋選項／繼續＋工具列（auto、skip、log、停止、聊天視圖）。
// 選項、換一組、繼續共用一把同步鎖：按下當下佔用，送出或換組連同其後的正式格重抓都完成才放。
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { t, type MsgKey } from "../../i18n";
import type { ChatController } from "../play/chat/useChatController";
import type { VnAdvance } from "../play/scene/useSceneActions";
import { ErrorNote } from "../../shared/ui/atoms";
import { backendText } from "../../shared/ui/backend-text";
import { IconHome, IconStop } from "../../shared/ui/icons";
import { MoreMenu } from "../../shared/ui/MoreMenu";
import { exportVnCard } from "./card/vn-card";
import { Backlog } from "./Backlog";
import { ChapterCardView } from "./ChapterCardView";
import { EndingScreen } from "./EndingScreen";
import { ChapterPanel, EndingsPanel } from "./StoryPanels";
import { useChapterEnd } from "./useChapterEnd";
import { ChoicePanel } from "./ChoicePanel";
import { DialogueBox } from "./DialogueBox";
import { Stage } from "./Stage";
import { FlagHints } from "./bible/FlagHints";
import { useVnCells } from "./useVnCells";
import { useSprites } from "./useSprites";
import { backdropKey, useBackgrounds } from "./useBackgrounds";
import { useVnPlayback } from "./useVnPlayback";
import { isTerminal, rerollError, stageAt } from "./vn-cells";
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
  error,
  transport,
  busy,
  onError,
  onAdvanceScene,
  onReenter,
  onShowChat,
  onGoLobby,
  onReplayChapter,
  onEnterTable,
  flagHints,
  onFlagHints,
}: {
  worldId: string;
  scene: number;
  tableName: string;
  /** App 層包好鎖的聊天控制器（lockedChat） */
  chat: ChatController;
  /** 角色 id → 角色卡原圖，當立繪佔位 */
  images: Record<string, string>;
  /** App 層的錯誤字（容量滿時附換幕鈕，走章結束流程的「只換幕」） */
  error: string;
  transport?: string;
  /** 生成中或進出桌進行中 */
  busy: boolean;
  onError: (message: string) => void;
  /** VN 換幕（包 5 §2.3）：chapterEnd＝章結束才加章；分階段回報 */
  onAdvanceScene: (chapterEnd: boolean) => Promise<VnAdvance>;
  /** 換幕已提交、重進失敗時只重試重進 */
  onReenter: () => Promise<boolean>;
  onShowChat: () => void;
  onGoLobby: () => void;
  /** 從某一章重玩（包 5 §2.9）：截幕首分岔後重進本桌 */
  onReplayChapter: (chapter: number) => Promise<void>;
  /** 進另一張桌（開新周目建好後） */
  onEnterTable: (worldId: string) => void;
  /** 格邊旗標提示開關（app 設定，預設開） */
  flagHints: boolean;
  onFlagHints: (on: boolean) => void;
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
    scene: vn.formalScene,
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
  const [panel, setPanel] = useState<"endings" | "chapters" | null>(null);
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

  // 章結束流程：游標推進（點擊或 auto，不含 skip 與進桌定位）到章末格就自動開始；字卡蓋住整個播放器
  const chapter = useChapterEnd({
    advance: onAdvanceScene,
    reenter: onReenter,
    openBeat: async () => {
      play.expectNext();
      await chat.gmAdvance();
    },
    runLocked,
  });
  const chapterNow = play.current?.chapter ?? vn.cells[vn.cells.length - 1]?.chapter ?? 1;
  const endChapter = (chapterEnd: boolean) => void chapter.start(chapterEnd, chapterNow, scene);
  const skipRef = useRef(play.skip);
  skipRef.current = play.skip;
  const landedKey = useRef<string | undefined>(undefined);
  const startRef = useRef(endChapter);
  startRef.current = endChapter;
  useEffect(() => {
    const previous = landedKey.current;
    landedKey.current = play.current?.key;
    const moved = previous !== undefined && previous !== play.current?.key;
    if (moved && play.current?.cell.type === "chapter" && !skipRef.current) startRef.current(true);
  }, [play.current]);
  // 重進後播放器換到新幕才開首拍（同一輪的聊天控制器還綁著舊幕）
  const { beginBeat } = chapter;
  useEffect(() => {
    const card = chapter.card;
    if (card?.phase === "opening" && card.beat && scene !== card.fromScene) void beginBeat();
  }, [chapter.card, scene, beginBeat]);
  // 下一章第一格出來就收起字卡（首拍的接力還在背景跑）
  const closeCard = chapter.close;
  useEffect(() => {
    if (chapter.card?.phase === "opening" && lastPlayable?.kind === "pending") closeCard();
  }, [chapter.card?.phase, lastPlayable, closeCard]);
  const replay = (chapter: number) =>
    runLocked(async () => {
      setPanel(null);
      await onReplayChapter(chapter);
    });
  const newPlaythrough = () =>
    runLocked(async () => {
      const ok = await confirm(t("vnNewPlaythroughConfirm"), {
        kind: "info",
        okLabel: t("vnNewPlaythrough"),
        cancelLabel: t("dialogCancel"),
      });
      if (!ok) return;
      try {
        const nameTemplate = t("vnPlaythroughName", { name: "{name}", n: "{n}" });
        onEnterTable(await invoke<string>("vn_new_playthrough", { worldId, nameTemplate }));
      } catch (reason) {
        onError(backendText(reason));
      }
    });
  const askNextChapter = async () => {
    if (await confirm(t("vnNextChapterConfirm"), { kind: "info", okLabel: t("vnNextChapter"), cancelLabel: t("dialogCancel") })) endChapter(true);
  };

  const { skip, setSkip, advance } = play;
  const onAdvance = useCallback(() => {
    if (skip) setSkip(false);
    else advance();
  }, [skip, setSkip, advance]);

  const actionsDisabled = locked || busy;
  const onTerminal = play.current && isTerminal(play.current.cell) ? play.current : null;
  const choices = play.end === "choices" && onTerminal?.cell.type === "choices" ? onTerminal.cell : null;
  const ending = play.end === "ended" && onTerminal?.cell.type === "ending" ? onTerminal.cell : null;
  // 停在終止格（選項、章末、結局）時對話框留著前一格文字
  const shownCell = onTerminal ? play.playable[play.playable.indexOf(onTerminal) - 1] : play.current;
  // 播到一則事件的第一個可停格時亮它的旗標提示
  const current = play.current;
  const hints =
    flagHints && current && play.playable.find((cell) => cell.eventKey === current.eventKey) === current
      ? current.hints
      : undefined;
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
        <button
          type="button"
          className="btn btn-ghost"
          aria-pressed={flagHints}
          onClick={() => onFlagHints(!flagHints)}
        >
          {t("vnFlagHintsToggle")}
        </button>
        <button type="button" className="btn btn-ghost" onClick={() => setLogOpen(true)}>
          {t("vnLog")}
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={actionsDisabled || ending !== null}
          onClick={() => void askNextChapter()}
        >
          {t("vnNextChapter")}
        </button>
        <button type="button" className="btn btn-ghost" onClick={onShowChat}>
          {t("vnShowChat")}
        </button>
        <MoreMenu
          label={t("moreActions")}
          disabled={busy}
          items={[
            { key: "endings", label: t("vnEndingsList"), onSelect: () => setPanel("endings") },
            { key: "replay", label: t("vnReplayChapter"), onSelect: () => setPanel("chapters") },
            {
              key: "export-card",
              label: t("vnCardExport"),
              onSelect: () =>
                void exportVnCard(worldId, tableName).catch((reason) => onError(backendText(reason))),
            },
          ]}
        />
      </header>
      {error && (
        <ErrorNote text={error} transport={transport} onAdvanceScene={() => endChapter(false)} />
      )}
      <Stage
        stage={stage}
        sprites={sprites}
        images={images}
        backdrop={backdropId ? backgrounds.images[backdropId] : undefined}
        cellKey={play.current?.key ?? ""}
      />
      <div className="vn-bottom">
        {hints && hints.length > 0 && <FlagHints hints={hints} />}
        {choices && (
          <ChoicePanel
            options={choices.options}
            disabled={actionsDisabled}
            error={rerollMessage}
            onChoose={(option) => void startTurn(() => chat.submitText(option))}
            onReroll={() => void reroll()}
          />
        )}
        {play.end === "chapter" && !chapter.card && (
          <button
            type="button"
            className="btn btn-primary vn-continue"
            disabled={actionsDisabled}
            onClick={() => endChapter(true)}
          >
            {t("vnNextChapter")}
          </button>
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
          shown={onTerminal ? Infinity : play.shown}
          waiting={play.end === "waiting"}
          ready={!play.typing && !play.tailing && play.end === null}
          onAdvance={onAdvance}
        />
      </div>
      {chapter.card && (
        <ChapterCardView
          card={chapter.card}
          nextChapter={vn.cells[0]?.chapter}
          onRetry={() => void chapter.retry()}
          onClose={chapter.close}
        />
      )}
      {ending && !logOpen && !panel && (
        <EndingScreen
          worldId={worldId}
          endingId={ending.id}
          disabled={actionsDisabled}
          onReplay={() => setPanel("chapters")}
          onNewPlaythrough={() => void newPlaythrough()}
          onEndings={() => setPanel("endings")}
          onGoLobby={onGoLobby}
        />
      )}
      {panel === "endings" && <EndingsPanel worldId={worldId} onClose={() => setPanel(null)} />}
      {panel === "chapters" && (
        <ChapterPanel
          chapter={chapterNow}
          disabled={actionsDisabled}
          onPick={(n) => void replay(n)}
          onClose={() => setPanel(null)}
        />
      )}
      {logOpen && (
        <Backlog cells={vn.cells.slice(0, play.flatIndex + 1)} onClose={() => setLogOpen(false)} />
      )}
    </main>
  );
}
