// VN 桌的播放器：背景鋪滿的舞台上疊對話框＋選項／繼續＋快捷列，遊戲選單收其餘入口；點畫面、鍵盤、滾輪推進（Ren'Py 慣例）。
// 重播桌（作品包，包 8 §2.6）不生成：選項走分支面板、章末與開始都只換幕（replay/replay-nav），收掉生成類入口。
// 選項、換一組、繼續共用一把同步鎖：按下當下佔用，送出或換組連同其後的正式格重抓都完成才放。
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { t, type MsgKey } from "../../i18n";
import type { ChatController } from "../play/chat/useChatController";
import type { VnAdvance } from "../play/scene/useSceneActions";
import type { SceneLabel } from "../../shared/contracts/backend-contracts";
import { ErrorNote } from "../../shared/ui/atoms";
import { backendText } from "../../shared/ui/backend-text";
import { exportVnCard } from "./card/vn-card";
import { ExportWorkDialog } from "./work/ExportWorkDialog";
import { Backlog } from "./Backlog";
import { ChapterCardView } from "./ChapterCardView";
import { EndingScreen } from "./EndingScreen";
import { ChapterPanel, EndingsPanel, ProgressPanel } from "./StoryPanels";
import { SavesPanel } from "./SavesPanel";
import { TitleScreen } from "./TitleScreen";
import { ArtGallery } from "./gallery/ArtGallery";
import { GameMenu, gameMenuItems, gameMoreItems } from "./GameMenu";
import { SettingsPanel } from "./SettingsPanel";
import { QuickBar } from "./QuickBar";
import { useVnKeys, type VnLayer } from "./useVnKeys";
import { useVnProgress } from "./useVnProgress";
import { useChapterEnd } from "./useChapterEnd";
import { ChoicePanel } from "./ChoicePanel";
import { PastChoicePanel } from "./PastChoicePanel";
import { useBranchOps, type VnEntry } from "./useBranchOps";
import { DialogueBox } from "./DialogueBox";
import { Stage } from "./Stage";
import { FlagHints } from "./bible/FlagHints";
import { useVnCells } from "./useVnCells";
import { useSprites } from "./useSprites";
import { backdropKey, useBackgrounds } from "./useBackgrounds";
import { useVnPlayback } from "./useVnPlayback";
import { nextScene, replayNav } from "./replay/replay-nav";
import { UnfinishedPath } from "./replay/UnfinishedPath";
import {
  AUTO_WAITS,
  TEXT_SPEEDS,
  cellHash,
  isTerminal,
  rerollError,
  stageAt,
  type AutoWait,
  type TextSpeed,
} from "./vn-cells";
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
  onBranch,
  onEnterTable,
  flagHints,
  onFlagHints,
  textSpeed,
  autoWait,
  onTextSpeed,
  onAutoWait,
  title,
  onTitle,
  replay,
  tagline,
  sceneLabels,
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
  /** 選項點分岔、切換分支（包 6 §2.3、§2.4）：跑完分支動作後重進本桌，分階段回報 */
  onBranch: (run: () => Promise<unknown>) => Promise<VnAdvance>;
  /** 進另一張桌（開新周目建好後） */
  onEnterTable: (worldId: string) => void;
  /** 格邊旗標提示開關（app 設定，預設開） */
  flagHints: boolean;
  onFlagHints: (on: boolean) => void;
  /** 打字速度與 auto 等待檔位（app 設定，預設 normal＝原本的固定值） */
  textSpeed: TextSpeed;
  autoWait: AutoWait;
  onTextSpeed: (speed: TextSpeed) => void;
  onAutoWait: (wait: AutoWait) => void;
  /** 標題畫面開著（從大廳進桌時，包 6 §2.8） */
  title: boolean;
  onTitle: (open: boolean) => void;
  /** 作品包匯入的重播桌（`WorldMeta.replay`） */
  replay: boolean;
  /** 主角一句話（`WorldMeta.tagline`）；空的不顯示 */
  tagline?: string;
  /** 幕標籤（重播桌找換幕子幕與劇情前一幕） */
  sceneLabels: Record<string, SceneLabel>;
}) {
  const [logOpen, setLogOpen] = useState(false);
  const [panel, setPanel] = useState<"menu" | "menu-more" | "endings" | "gallery" | "chapters" | "saves" | "settings" | "progress" | null>(null);
  // 隱藏介面（H）：對話框、選項、快捷列整塊收起，任一點擊或按鍵恢復；auto／skip 照跑
  const [hidden, setHidden] = useState(false);
  const [exporting, setExporting] = useState(false);
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

  // 章結束流程：游標推進（點擊或 auto，不含 skip 與進桌定位）到章末格就自動開始；字卡蓋住整個播放器。
  // 重播桌換到換幕子幕、字卡停一下就收（沒有首拍可等）；沒有子幕不開流程，改出「此路作者未完成」。
  // 在播放 hook 之前取得（字卡要讓播放暫停），用到的播放與重播導覽走 ref
  const playRef = useRef<{ expectNext: () => void }>({ expectNext: () => {} });
  const navRef = useRef<ReturnType<typeof replayNav> | null>(null);
  const chapter = useChapterEnd({
    advance: (chapterEnd) => (replay && navRef.current ? navRef.current.advance() : onAdvanceScene(chapterEnd)),
    reenter: onReenter,
    openBeat: async () => {
      if (replay) return new Promise((resolve) => setTimeout(resolve, 1200));
      playRef.current.expectNext();
      await chat.gmAdvance();
    },
    runLocked,
  });
  // 標題畫面、面板、log、章末字卡蓋著播放器時：游標不動、不打字、不登記已讀與位置、不觸發章末（收起後接著跑）；
  // 字卡的首拍與收卡不受影響
  const paused = title || panel !== null || logOpen || chapter.card !== null;
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
  const progress = useVnProgress(worldId);
  const [entry, setEntry] = useState<VnEntry | null>(null);
  const { isRead: readKey } = progress;
  const isRead = useCallback((cell: { cell: Parameters<typeof cellHash>[0] }) => readKey(cellHash(cell.cell)), [readKey]);
  const play = useVnPlayback({
    cells: vn.cells,
    lastEventKey: vn.lastEventKey,
    tail: vn.tail,
    generating: chat.generating !== null,
    scene: vn.formalScene,
    entry: entry !== null && vn.formalScene !== entry.from ? entry : null,
    resumeKey: vn.formalScene === null ? undefined : progress.positions[String(vn.formalScene)],
    hold: !progress.loaded,
    isRead,
    paused,
    charMs: TEXT_SPEEDS[textSpeed],
    autoFactor: AUTO_WAITS[autoWait],
    // 重播桌最後一幕的章末格沒有下一幕可換：不停（直接到結局或「此路作者未完成」，不用空點一下）
    skipChapter: replay && nextScene(sceneLabels, scene) === null,
  });
  realignRef.current = play.realign;
  playRef.current = play;
  const stage = useMemo(() => stageAt(vn.cells, play.flatIndex), [vn.cells, play.flatIndex]);
  const sprites = useSprites(worldId);
  const backgrounds = useBackgrounds(worldId);
  // 正式格有新事件落檔就排入這幕缺的背景（串流中寫完的 `[bg]` 行由 useVnCells 提早排）
  useEffect(() => {
    invoke("vn_backgrounds_fill", { worldId, scene }).catch(() => {});
  }, [worldId, scene, vn.lastEventKey]);
  const backdrop = stage.backdrop;
  const backdropId = backdrop ? backdropKey(backdrop) : null;
  const { load } = backgrounds;
  useEffect(() => {
    if (backdrop) load(backdrop);
  }, [backdrop, load]);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [rerollMessage, setRerollMessage] = useState<string | null>(null);

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

  const branch = useBranchOps({
    worldId,
    formalScene: vn.formalScene,
    live: play.live !== undefined,
    onBranch,
    onReenter,
    runLocked,
    submit: (option) => void startTurn(() => chat.submitText(option)),
    setEntry,
    jump: play.jump,
    scene,
  });
  const nav = replayNav({ worldId, scene, labels: sceneLabels, goto: branch.goto, jump: play.jump, setEntry, onBranch });
  navRef.current = nav;
  const chapterNow = play.current?.chapter ?? vn.cells[vn.cells.length - 1]?.chapter ?? 1;
  const endChapter = (chapterEnd: boolean) => {
    if (!replay || nav.next !== null) void chapter.start(chapterEnd, chapterNow, scene);
  };
  const skipRef = useRef(play.skip);
  skipRef.current = play.skip;
  const landedKey = useRef<string | undefined>(undefined);
  const startRef = useRef(endChapter);
  startRef.current = endChapter;
  useEffect(() => {
    if (paused) return;
    const previous = landedKey.current;
    landedKey.current = play.current?.key;
    const moved = previous !== undefined && previous !== play.current?.key;
    if (moved && play.current?.cell.type === "chapter" && !skipRef.current) startRef.current(true);
  }, [play.current, paused]);
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
  const pickChapter = (chapter: number) => {
    if (replay) {
      setPanel(null);
      return nav.chapter(chapter);
    }
    return runLocked(async () => {
      setPanel(null);
      await onReplayChapter(chapter);
    });
  };
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
  const exportWork = () => setExporting(true);
  const openPanel = (next: NonNullable<typeof panel>) => {
    if (next === "saves") setSaveError(null);
    setPanel(next);
  };
  // 新幕的游標定位好就收回指定位置，之後換幕照一般進桌位置
  const positioned = play.current !== undefined;
  useEffect(() => {
    if (entry !== null && vn.formalScene !== null && vn.formalScene !== entry.from && positioned) setEntry(null);
  }, [entry, vn.formalScene, positioned]);
  // 已讀與閱讀位置（§2.6）：正式格字全出才算讀過；skip 只停在已讀格上，不另擋
  const { markRead, setPosition } = progress;
  const readable = play.current && play.current.kind !== "pending" && !play.typing ? play.current : null;
  useEffect(() => {
    if (paused || !readable || vn.formalScene === null) return;
    markRead(cellHash(readable.cell));
    setPosition(vn.formalScene, readable.key);
  }, [paused, readable, vn.formalScene, markRead, setPosition]);
  const saveHere = (name: string) =>
    runLocked(async () => {
      const at = play.current;
      if (!at?.eventKey) return;
      setSaveError(null);
      try {
        await invoke("vn_save_create", { worldId, name, eventId: at.eventKey, cell: at.index });
        await progress.reloadSaves();
      } catch (reason) {
        setSaveError(backendText(reason));
      }
    });
  const deleteSave = (id: string) =>
    runLocked(async () => {
      if (!(await confirm(t("vnSaveDeleteConfirm"), { kind: "warning", okLabel: t("vnSaveDelete"), cancelLabel: t("dialogCancel") }))) return;
      await invoke("vn_save_delete", { worldId, id }).catch((reason) => setSaveError(backendText(reason)));
      await progress.reloadSaves();
    });
  const loadSave = async (id: string) => {
    setPanel(null);
    onTitle(false);
    await branch.load(id);
  };
  // 標題「開始」：只有開場（還沒玩）＝繼續；玩過就從第 1 章重玩（原本的進度留在原分支）；重播桌回第 0 幕幕首
  const playedAny = scene > 0 || vn.cells.some((cell) => cell.kind === "player");
  const startOver = async () => {
    if (replay) {
      onTitle(false);
      return nav.start();
    }
    if (!playedAny) return onTitle(false);
    if (!(await confirm(t("vnTitleRestartConfirm"), { kind: "info", okLabel: t("vnTitleStart"), cancelLabel: t("dialogCancel") }))) return;
    onTitle(false);
    await pickChapter(1);
  };
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
  const choices = play.end === "choices" && play.live?.cell.type === "choices" ? play.live.cell : null;
  const past =
    play.current?.cell.type === "choices" && play.current.choice?.answered && play.current.eventKey
      ? { cell: play.current.cell, answered: play.current.choice.answered, eventId: play.current.eventKey }
      : null;
  const ending = play.end === "ended" && onTerminal?.cell.type === "ending" ? onTerminal.cell : null;
  // 重播桌讀到幕末、不是結局、也沒有下一幕：作者沒寫完（選項點的情形由選項面板自己顯示）
  const unfinished = replay && !chapter.card && nav.next === null && (play.end === "continue" || play.end === "chapter");
  const choiceHere = play.playable.some((cell) => cell.cell.type === "choices" && cell.choice?.answered);
  const unfinishedPath = (
    <UnfinishedPath
      disabled={actionsDisabled}
      onBackToChoice={nav.canBack(choiceHere) ? () => nav.backToChoice(choiceHere) : undefined}
      onTitle={() => onTitle(true)}
    />
  );
  // 停在終止格（選項、章末、結局）時對話框留著前一格文字
  const shownCell = onTerminal ? play.playable[play.playable.indexOf(onTerminal) - 1] : play.current;
  // 播到一則事件的第一個可停格時亮它的旗標提示
  const current = play.current;
  const hints =
    flagHints && current && play.playable.find((cell) => cell.eventKey === current.eventKey) === current
      ? current.hints
      : undefined;

  // 推進（點畫面、鍵盤、滾輪下）：停在章末格＝進章末流程（與自動流程一致）；「繼續」會花額度，只靠按鈕
  const proceed = () => {
    if (play.end === "chapter" && !chapter.card && !unfinished) {
      if (!actionsDisabled) endChapter(true);
      return;
    }
    onAdvance();
  };
  // 舞台與對話框以外的推進：選項面板開著不動作
  const proceedOutside = () => {
    if (!choices && !past && !unfinished) proceed();
  };
  const layer: VnLayer =
    logOpen || panel !== null
      ? "closable"
      : title || chapter.card || exporting
        ? "blocked"
        : ending
          ? "menuOnly"
          : null;
  // 蓋層開著時下緣整塊 inert：聚焦中的「繼續」等按鈕不能在選單後面被原生 Enter／Space 按到（會花額度）；
  // 不支援 inert 的環境補一層：焦點在下緣就先拿掉
  const bottomRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const focused = document.activeElement;
    if (layer !== null && focused instanceof HTMLElement && bottomRef.current?.contains(focused)) focused.blur();
  }, [layer]);
  const closeLayer = () => {
    if (logOpen) setLogOpen(false);
    else setPanel(panel === "menu-more" ? "menu" : null);
  };
  useVnKeys({
    layer,
    hidden,
    onClose: closeLayer,
    onMenu: () => setPanel("menu"),
    onUnhide: () => setHidden(false),
    onHide: () => setHidden(true),
    onAdvance: proceedOutside,
    onBack: play.back,
    onAuto: () => play.setAuto(!play.auto),
    skip,
    setSkip,
  });
  // 滾輪：上＝上一格、下＝推進（Ren'Py 慣例，250ms 節流）；蓋層與還能捲的對話文字自己吃滾輪
  const wheelAt = useRef(0);
  const onWheel = (event: React.WheelEvent) => {
    if (layer !== null || hidden || event.deltaY === 0 || Date.now() - wheelAt.current < 250) return;
    const text = (event.target as Element).closest(".vn-text");
    if (text && text.scrollHeight > text.clientHeight) return;
    wheelAt.current = Date.now();
    if (event.deltaY < 0) play.back();
    else proceedOutside();
  };
  // 點畫面空白處＝點對話框；按鈕、選項、快捷列、錯誤訊息自己處理（對話框是按鈕，不會推兩次）
  const onClick = (event: React.MouseEvent) => {
    if (hidden) return setHidden(false);
    if (layer !== null) return;
    if ((event.target as Element).closest("button, a, input, select, textarea, label, .vn-dialogue, .vn-quick, .vn-choices, .vn-error")) return;
    proceedOutside();
  };
  // 右鍵＝Esc（開選單、子選單回主選單或收起隱藏）；面板、紀錄、匯出開著與輸入欄位留原生選單（存檔名稱要能貼上）
  const onContextMenu = (event: React.MouseEvent) => {
    if (panel === "menu-more") {
      event.preventDefault();
      return setPanel("menu");
    }
    if (layer === "closable" || layer === "blocked") return;
    event.preventDefault();
    if (hidden) setHidden(false);
    else setPanel("menu");
  };
  const menuActions = {
    replay,
    busy,
    nextDisabled: actionsDisabled || ending !== null,
    open: openPanel,
    close: () => setPanel(null),
    openMore: () => setPanel("menu-more"),
    onTitle: () => onTitle(true),
    onNextChapter: () => void askNextChapter(),
    onShowChat,
    onExportCard: () => void exportVnCard(worldId, tableName).catch((reason) => onError(backendText(reason))),
    onExportWork: exportWork,
    onHide: () => setHidden(true),
    onLobby: onGoLobby,
  };
  const hasProgress = Object.keys(progress.positions).length > 0;
  return (
    <main className="vn-player" onWheel={onWheel} onClick={onClick} onContextMenu={onContextMenu}>
      <Stage
        stage={stage}
        sprites={sprites}
        images={images}
        backdrop={backdropId ? backgrounds.images[backdropId] : undefined}
        cellKey={play.current?.key ?? ""}
        hold={paused}
      />
      {error && (
        <div className="vn-error">
          <ErrorNote text={error} transport={transport} onAdvanceScene={() => endChapter(false)} />
        </div>
      )}
      <div ref={bottomRef} className={`vn-bottom${hidden ? " vn-ui-hidden" : ""}`} inert={layer !== null}>
        {hints && hints.length > 0 && <FlagHints hints={hints} />}
        {choices && replay && play.live?.eventKey && (
          <PastChoicePanel
            worldId={worldId}
            eventId={play.live.eventKey}
            options={choices.options}
            answered={null}
            disabled={actionsDisabled}
            onCurrent={advance}
            onSwitch={(target) => void nav.choose(target)}
            deadEnd={unfinishedPath}
          />
        )}
        {choices && !replay && (
          <ChoicePanel
            options={choices.options}
            disabled={actionsDisabled}
            error={rerollMessage}
            onChoose={(option) => void startTurn(() => chat.submitText(option))}
            onReroll={play.live?.eventKey === vn.lastEventKey ? () => void reroll() : undefined}
          />
        )}
        {past && (
          <PastChoicePanel
            worldId={worldId}
            eventId={past.eventId}
            options={past.cell.options}
            answered={past.answered}
            disabled={actionsDisabled}
            onCurrent={advance}
            onSwitch={(target) => void (replay ? nav.choose(target) : branch.switchTo(target.tip))}
            onFork={replay ? undefined : (option) => void branch.forkAt(past.eventId, option)}
            deadEnd={unfinished ? <UnfinishedPath disabled={actionsDisabled} onTitle={() => onTitle(true)} /> : undefined}
          />
        )}
        {branch.reenter !== null && (
          <p className="vn-choice-error" role="alert">
            {t("vnBranchReenterFailed")}{" "}
            <button type="button" className="btn" disabled={actionsDisabled} onClick={() => void branch.retry()}>
              {t("vnBranchReenterRetry")}
            </button>
          </p>
        )}
        {unfinished && !past && <div className="vn-choices">{unfinishedPath}</div>}
        <DialogueBox
          cell={shownCell}
          shown={onTerminal ? Infinity : play.shown}
          waiting={play.end === "waiting"}
          // 停在章末格也照一般格給 ▼：點畫面任何地方就進章末流程
          ready={!play.typing && !play.tailing && (play.end === null || (play.end === "chapter" && !chapter.card && !unfinished))}
          onAdvance={proceed}
          // 「繼續」會花玩家額度（重播桌只是換到下一幕）：放在 ▼ 的位置做成小鈕，點畫面不會觸發
          onContinue={
            play.end === "continue" && !unfinished
              ? { disabled: actionsDisabled, run: () => void (replay ? nav.forward() : startTurn(chat.gmAdvance)) }
              : undefined
          }
        />
        <QuickBar
          canStop={!replay && chat.canStop}
          onStop={chat.stopResponse}
          canBack={play.canBack}
          onBack={play.back}
          auto={play.auto}
          onAuto={() => play.setAuto(!play.auto)}
          skip={skip}
          onSkip={() => setSkip(!skip)}
          onLog={() => setLogOpen(true)}
          busy={busy}
          onSaves={() => openPanel("saves")}
          onSettings={() => setPanel("settings")}
          onMenu={() => setPanel("menu")}
        />
      </div>
      {chapter.card && (
        <ChapterCardView
          card={chapter.card}
          // 新幕的格載入前 cells 還是舊幕的，章號會等於結束的章而誤判成「尾聲」
          nextChapter={scene !== chapter.card.fromScene && vn.formalScene === scene ? vn.cells[0]?.chapter : undefined}
          onRetry={() => void chapter.retry()}
          onClose={chapter.close}
          onStop={!replay && chat.canStop ? chat.stopResponse : undefined}
          replay={replay}
        />
      )}
      {ending && !logOpen && !panel && (
        <EndingScreen
          worldId={worldId}
          endingId={ending.id}
          disabled={actionsDisabled}
          onReplay={() => setPanel("chapters")}
          onNewPlaythrough={replay ? undefined : () => void newPlaythrough()}
          onExport={replay ? undefined : exportWork}
          onEndings={() => setPanel("endings")}
          onTitle={() => onTitle(true)}
        />
      )}
      {title && (
        <TitleScreen
          worldId={worldId}
          tableName={tableName}
          tagline={tagline}
          backdrop={(backdropId && backgrounds.images[backdropId]?.src) || undefined}
          disabled={actionsDisabled}
          progress={!progress.loaded ? "loading" : hasProgress ? "resume" : progress.saves.length > 0 ? "saved" : "fresh"}
          onStart={() => void startOver()}
          onContinue={() => onTitle(false)}
          onLoad={() => setPanel("saves")}
          onEndings={() => setPanel("endings")}
          onGallery={() => setPanel("gallery")}
          onSettings={() => setPanel("settings")}
          onLobby={onGoLobby}
        />
      )}
      {panel === "saves" && (
        <SavesPanel
          saves={progress.saves}
          label={t(title ? "vnTitleLoad" : "vnSaves")}
          defaultName={t("vnSaveDefaultName", { n: chapterNow })}
          canSave={!title && play.current?.kind !== "pending" && Boolean(play.current?.eventKey)}
          disabled={actionsDisabled}
          error={saveError}
          onSave={(name) => void saveHere(name)}
          onLoad={(id) => void loadSave(id)}
          onDelete={(id) => void deleteSave(id)}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === "settings" && (
        <SettingsPanel
          flagHints={flagHints}
          onFlagHints={onFlagHints}
          textSpeed={textSpeed}
          onTextSpeed={onTextSpeed}
          autoWait={autoWait}
          onAutoWait={onAutoWait}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === "menu" && <GameMenu title={tableName} items={gameMenuItems(menuActions)} onClose={() => setPanel(null)} />}
      {panel === "menu-more" && (
        <GameMenu
          title={t("vnMenuMoreTitle")}
          items={gameMoreItems(menuActions)}
          onClose={() => setPanel(null)}
          onBack={() => setPanel("menu")}
        />
      )}
      {panel === "endings" && <EndingsPanel worldId={worldId} onClose={() => setPanel(null)} />}
      {panel === "progress" && (
        <ProgressPanel
          worldId={worldId}
          chapter={chapterNow}
          disabled={actionsDisabled}
          onPick={(n) => void pickChapter(n)}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === "gallery" && (
        <ArtGallery worldId={worldId} images={images} backgrounds={backgrounds} replay={replay} onClose={() => setPanel(null)} />
      )}
      {panel === "chapters" && (
        <ChapterPanel
          label={replay ? t("vnWorkChapterPick") : undefined}
          chapter={chapterNow}
          disabled={actionsDisabled}
          onPick={(n) => void pickChapter(n)}
          onClose={() => setPanel(null)}
        />
      )}
      {logOpen && (
        <Backlog cells={vn.cells.slice(0, play.flatIndex + 1)} onClose={() => setLogOpen(false)} />
      )}
      {exporting && (
        <ExportWorkDialog
          worldId={worldId}
          tableName={tableName}
          onClose={() => setExporting(false)}
          onError={(reason) => onError(backendText(reason))}
        />
      )}
    </main>
  );
}
