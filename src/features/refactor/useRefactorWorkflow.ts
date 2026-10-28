import { useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../../i18n";
import {
  assembleRefactorOutcome,
  mergeRefactorInterfaces,
  refactorTaskUids,
  unfinishedSourceUids,
  buildRefactorPersonPlan,
  defaultRefactorSelection,
  REFACTOR_IMPORT_INVALID,
  restoreDropped,
  sourceEntryTitle,
  type RefactorApplySummary,
  type RefactorCharacter,
  type RefactorExpandOutcome,
  type RefactorInterface,
  type RefactorLocalAssembly,
  type RefactorNewEntry,
  type RefactorOutcome,
  type RefactorPersonExpandOutcome,
  type RefactorPersonQueueItem,
  type RefactorRewriteOutcome,
  type RefactorSelection,
  type RefactorSplitGroup,
  type RefactorSurveyOutcome,
  selectionIsEmpty,
} from "./refactor-review";
import {
  parseRefactorCardValue,
  REFACTOR_IMPORT_NEWER,
  type RefactorApplied,
  type RefactorCard,
  type RefactorCardOpenResult,
} from "./refactor-card";
import { confirmFrames, type RefactorFrameCandidate } from "./refactor-frame";
import { sourceContainerRoles } from "./refactor-slot";
import { REFACTOR_PARALLEL_LIMIT, runRefactorCalls, withRateLimitRetry } from "./refactor-run";
import {
  detectRefactorCardState,
  probeFailureKind,
  stripTransportPrefix,
  type RefactorMode,
  type RefactorRecommendOutcome,
  type RefactorRunTicket,
} from "./refactor-mode";
import type { CardInterface } from "../card-interface/interface-card";
import { useTurnWait } from "../play/useTurnWait";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { EntryChanges } from "../worldbook/worldbook-model";

export interface RefactorModeAsk {
  recommend: RefactorMode | null;
  evidence: string;
  expanded: boolean;
  picked: RefactorMode;
  /** 第二段 resume 憑證；null＝初判失敗或非 claude lane，第二段直接重送全卡。 */
  ticket: RefactorRunTicket | null;
  /** 初判送出後失敗或解析不出：框內附原因（raw＝後端原文）；初判成功為 null。 */
  failure: { kind: "failed" | "unparsable"; raw: string } | null;
}

interface UseRefactorWorkflowOptions {
  world: string;
  worldName: string;
  setStatusMessage: (message: string) => void;
  /** live() 為 false（換桌或卸載）時刷新鏈要在下一個 await 邊界停下；entryChanges 是這次改了哪些既有條目
   * （開著的條目草稿據此定基），null＝無從判斷（清回、套用後讀不到世界書） */
  refreshAfterApply: (live: () => boolean, entryChanges: EntryChanges | null) => Promise<void>;
  /** 清回前先把開著的既有條目草稿存起來並關掉（乾淨的也關）；存不起來回 false，不清回 */
  flushDraft: () => Promise<boolean>;
  /** 同步問「這桌有沒有回合在跑」：有的話套用會排在它後面（後端持整桌獨占） */
  isTurnRunning: () => boolean;
}

// 後端拒套已遊玩桌的錯誤（refactor/reset.rs REFACTOR_PLAYED），一字不差。
const REFACTOR_PLAYED = "refactor-played";
// 後端拒收空選取的錯誤（refactor/reset.rs REFACTOR_EMPTY_SELECTION）：前端已先擋，萬一打到後端也換成玩家看得懂的話
const REFACTOR_EMPTY_SELECTION = "refactor-empty-selection";

// 重構卡存檔對話框預設檔名：桌名可能含檔名非法字元，一律代換成 -；空桌名就不接前綴，只用在地化字尾。
// 尾碼只給玩家分辨三階（不含圖／含角色圖），格式判定只認檔內 chunk。
function refactorCardFileName(tableName: string, withImages: boolean): string {
  const safe = tableName.replace(/[\\/:*?"<>|\x00-\x1f\x7f]/g, "-");
  const suffix = t(withImages ? "refactorExportFileNameImages" : "refactorExportFileName");
  return `${safe ? `${safe}-` : ""}${suffix}.png`;
}

// 存檔對話框：PNG 為預設；含角色圖只給 PNG（JSON 放不了圖，後端也會拒）
function refactorCardFilters(withImages: boolean) {
  const png = { name: t("refactorOutcomePng"), extensions: ["png"] };
  return withImages ? [png] : [png, { name: t("refactorOutcomeJson"), extensions: ["json"] }];
}

// 匯出完成訊息帶檔案大小：玩家互傳時要知道塞不塞得進聊天軟體的附件上限
function exportedMessage(bytes: number): string {
  const size =
    bytes < 1024 * 1024 ? `${Math.max(1, Math.round(bytes / 1024))} KB` : `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return t("refactorExportDone", { size });
}

function refactorApplyMessage(summary: RefactorApplySummary) {
  return [
    summary.new_characters > 0 && t("refactorApplyDoneCharacters", { n: summary.new_characters }),
    summary.player_assigned && t("refactorApplyDonePlayer"),
    summary.stage_cards > 0 && t("refactorApplyDoneStage", { n: summary.stage_cards }),
    summary.new_entries > 0 && t("refactorApplyDoneEntries", { n: summary.new_entries }),
    summary.deleted_entries > 0 && t("refactorApplyDoneDeleted", { n: summary.deleted_entries }),
    summary.interface_applied && t("refactorApplyDoneInterface"),
    summary.mechanisms_applied > 0 && t("refactorApplyDoneMechanisms", { n: summary.mechanisms_applied }),
    summary.images_applied > 0 && t("refactorApplyDoneImages", { n: summary.images_applied }),
    (summary.images_failed?.length ?? 0) > 0 &&
      t("refactorApplyImagesFailed", { names: summary.images_failed.join(t("listSeparator")) }),
    summary.card_save_failed && t("refactorApplySaveFailed"),
  ]
    .filter(Boolean)
    .join("・");
}

export function useRefactorWorkflow({
  world,
  worldName,
  setStatusMessage,
  refreshAfterApply,
  flushDraft,
  isTurnRunning,
}: UseRefactorWorkflowOptions) {
  const { run: runQueued, waiting: waitingForTurn } = useTurnWait(isTurnRunning, world);
  // AI 卡重構：結果卡（產物讀進來後的人審／套用）與下面的「盤點→展開」進度是兩段獨立狀態，
  // 交會點是 setRefactorOutcome——AI 兩階段跑完、或選檔路徑讀完 JSON，都寫進同一份結果卡。
  const [outcome, setOutcome] = useState<RefactorOutcome | null>(null);
  const [selection, setSelection] = useState<RefactorSelection | null>(null);
  // 匯入的重構卡帶來源桌套用映射時才有值：預設勾選照它重現來源桌的建卡與玩家選擇
  const [applied, setApplied] = useState<RefactorApplied | null>(null);
  const [origin, setOrigin] = useState<"ai" | "import" | null>(null);
  const [detail, setDetail] = useState(false);
  // 取消後仍組出的半成品：中止的呼叫不會留下任何痕跡（產物沒 push、也不列失敗），
  // 面板自己說出來才看得見缺件——標題改「已取消」、主按鈕換成「不要」。
  const [cancelled, setCancelled] = useState(false);
  // 二選一對話框（refactor-mode-split）：recommend null＝初判失敗（不偽造證據、直接展開兩選項
  // 預設介面優先）；expanded＝玩家按了「自己選」看得到兩張選項卡。
  const [modeAsk, setModeAsk] = useState<RefactorModeAsk | null>(null);
  // pool 呼叫失敗的條目名單（2026-08-12 B 拍板）：顯示在結果視窗頂部紅字段。
  const [failures, setFailures] = useState<{ name: string; reason: string }[]>([]);
  const [busy, setBusy] = useState(false);
  // 重構卡 PNG 附的角色圖：圖在後端暫存單槽，這裡只記張數與 token。
  // 套用分兩段：前端排隊等回合（waiting，素材還在槽裡、歸這裡管）→ 已送後端（submitted，
  // 後端一進來就取走；成功＝用掉，拒套＝放回槽、又歸這裡管）。卸載時 idle 直接釋放；
  // waiting／submitted 交給套用那條路收尾，沒送出或被拒套就釋放。
  const [assetCount, setAssetCount] = useState(0);
  const heldToken = useRef<{ world: string; token: string } | null>(null);
  const applyPhase = useRef<"idle" | "waiting" | "submitted">("idle");
  // 開檔請求世代：開新檔、關結果卡、換桌都讓還在路上的舊開檔請求失效
  const openSeq = useRef(0);
  const inputRef = useRef<HTMLInputElement>(null);
  // 非 null＝AI 盤點／展開跑中，modal 顯示 text；cancelling 只管取消鈕的 disabled，不影響迴圈判斷。
  const [progress, setProgress] = useState<{ text: string; cancelling: boolean; tail: string } | null>(null);
  // 迴圈裡讀取的取消旗標——用 ref 而非 state：async 迴圈裡的閉包看不到後續 setState，只有 ref.current 每次都讀最新值。
  const cancelRef = useRef(false);
  // 進度世代：每開一段新進度（初判、主體）加一，晚到的取消失敗只認發起時那一段
  const progressGen = useRef(0);
  // 不排隊的路徑（清回原卡後刷新）自己判斷「還在原桌、元件還在」
  const worldRef = useRef(world);
  worldRef.current = world;
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      openSeq.current += 1;
      if (applyPhase.current === "idle") releaseHeldAssets();
    };
  }, []);
  useEffect(() => {
    openSeq.current += 1;
  }, [world]);

  function releaseToken(held: { world: string; token: string } | null) {
    if (held) void invoke("refactor_card_release", { worldId: held.world, token: held.token }).catch(() => {});
  }

  function releaseHeldAssets() {
    const held = heldToken.current;
    heldToken.current = null;
    releaseToken(held);
  }

  // 匯出這桌先前套用過的重構產物（apply() 落檔），重玩同一張卡不必再燒 AI 額度重新展開。
  async function exportSavedRefactorOutcome(withImages = false) {
    setStatusMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName, withImages),
        filters: refactorCardFilters(withImages),
      });
      if (!path) return;
      const bytes = await invoke<number>("refactor_export_saved", { worldId: world, path, withImages });
      setStatusMessage(exportedMessage(bytes));
      await revealItemInDir(path);
    } catch (reason) {
      setStatusMessage(
        String(reason).includes("refactor-export-none") ? t("refactorExportNone") : String(reason),
      );
    }
  }

  // 重構的桌況閘門（AI 重構與套用重構卡產物共用）〔作者裁決 2026-10-03、2026-10-11〕：已遊玩擋（請開新桌）、
  // 重構過但沒有匯入原檔擋；重構過、未遊玩的確認後清回剛匯入原卡的狀態，永不拿重構後的資料再重構。未重構的
  // 桌照現狀。分兩段：先查桌況（擋下的當場提示），清回留給呼叫端在該做的時候做。`stale` 為真＝這次請求已作廢。
  type RerunStatus = "fresh" | "played" | "no_source" | "ready";
  // 回 false＝擋下或已作廢。
  // 查詢失敗在這裡就報到狀態列（兩個呼叫端都在呼叫前清過狀態列、拿到 false 直接收手，只報一次）。
  async function checkRerun(stale: () => boolean): Promise<RerunStatus | false> {
    let rerun: RerunStatus;
    try {
      rerun = await invoke<RerunStatus>("refactor_rerun_status", { worldId: world });
    } catch (reason) {
      if (!stale()) setStatusMessage(String(reason));
      return false;
    }
    if (stale()) return false;
    if (rerun === "played" || rerun === "no_source") {
      await showMessage(t(rerun === "played" ? "refactorRerunPlayed" : "refactorRerunNoSource"), {
        title: t("refactorBtn"),
        kind: "warning",
        okLabel: t("dialogAck"),
      });
      return false;
    }
    return rerun;
  }

  // 重構過、未遊玩：確認後先存草稿，清回剛匯入原卡的狀態並刷新畫面。回 true＝已清回、可以往下走。
  // 清回一旦成功，刷新只看桌別（後端已換掉整桌，畫面一定要跟上）；`stale` 只決定刷新後要不要接著往下走。
  async function resetForRerun(stale: () => boolean): Promise<boolean> {
    const origin = world;
    const sameTable = () => mounted.current && worldRef.current === origin;
    const confirmed = await confirm(t("refactorRerunWarnBody"), {
      title: t("refactorBtn"),
      kind: "warning",
      okLabel: t("refactorRerunOk"),
      cancelLabel: t("dialogCancel"),
    });
    if (!confirmed || stale()) return false;
    // 清回會換掉每一條：草稿先存起來（清回失敗時改動也保得住），之後的開著的既有草稿無從定基
    if (!(await flushDraft()) || stale()) return false;
    try {
      await invoke("refactor_reset_to_source", { worldId: world });
    } catch (reason) {
      if (!stale()) setStatusMessage(String(reason));
      return false;
    }
    if (!sameTable()) return false;
    await refreshAfterApply(sameTable, null);
    return !stale();
  }

  // 讀卡片介面判四態；讀不到、讀不出原卡、DRM／雲端載入器卡都在這裡報到狀態列並回 null（擋下）。
  async function routableCardState(stale: () => boolean): Promise<"supported" | "none" | null> {
    let cards: CardInterface[];
    try {
      cards = await invoke<CardInterface[]>("card_interfaces", { worldId: world });
    } catch (reason) {
      // 讀不到卡片介面不能當成「無介面卡」直跑角色線
      if (!stale()) setStatusMessage(String(reason));
      return null;
    }
    if (stale()) return null;
    const state = detectRefactorCardState(cards);
    if (state === "unreadable") {
      const name = cards.find((card) => card.unsupported === "unreadable")?.character_name.trim();
      setStatusMessage(name ? t("refactorCardUnreadable", { name }) : t("refactorCardUnreadableUnnamed"));
      return null;
    }
    if (state === "unsupported") {
      setStatusMessage(t("refactorUnsupportedCard"));
      return null;
    }
    return state;
  }

  // AI 卡重構：盤點出六區塊小抄（PERSONS／INTERFACE／ENTRIES／SPLITS／GROUPS／FIELDS）→本地
  // 零呼叫組裝（refactor_assemble_local：carry 照搬＋split 零呼叫路由＋clean 人物組卡）→剩餘
  // AI 呼叫全並行（人物佇列＋absorb＋group＋statusbar＋interface，上限 4、無序列鏈）→組產物。
  // knownFields 是 survey.fields 固定一份，所有呼叫共用，不再沿呼叫鏈累積。
  // 入口：四態偵測分流（refactor-mode-split）。supported 卡先跑初判再彈二選一；
  // none 是唯一免問的路（直跑角色線）；unreadable（原卡讀不出）與 unsupported（DRM／雲端載入器）擋下不跑。
  async function runAiRefactor() {
    if (progress) return;
    const origin = world;
    const stale = () => !mounted.current || worldRef.current !== origin;
    setStatusMessage("");
    const rerun = await checkRerun(stale);
    if (rerun === false) return;
    // 重構過的桌先清回再判：清回用的是收據保存的原檔，目前桌檔讀不出不代表清回後也讀不出
    if (rerun === "ready" && !(await resetForRerun(stale))) return;
    setStatusMessage("");
    const state = await routableCardState(stale);
    if (state === null) return;
    if (state === "none") {
      await startRefactorRun("characters");
      return;
    }
    // supported：初判帶全卡只出兩行；取消走既有 refactor_abort 路。
    cancelRef.current = false;
    setCancelled(false);
    progressGen.current += 1;
    setProgress({ text: t("refactorProbing"), cancelling: false, tail: "" });
    try {
      let probeTail = "";
      const channel = new Channel<string>();
      channel.onmessage = (delta: string) => {
        probeTail = (probeTail + delta).slice(-2000);
        setProgress((current) =>
          current && { ...current, tail: probeTail.split("\n").slice(-4).join("\n") },
        );
      };
      const probe = await invoke<RefactorRecommendOutcome>("refactor_recommend", {
        worldId: world,
        onDelta: channel,
      });
      setProgress(null);
      if (stale()) return;
      const recommend: RefactorMode = probe.recommend === "characters" ? "characters" : "interface";
      setModeAsk({
        recommend,
        evidence: probe.evidence,
        expanded: false,
        picked: recommend,
        ticket: probe.run_id ? { runId: probe.run_id, fingerprint: probe.fingerprint } : null,
        failure: null,
      });
    } catch (reason) {
      setProgress(null);
      if (stale()) return;
      const kind = probeFailureKind(reason);
      if (kind === "aborted") return;
      // 白名單碼（持續性設定／前置障礙）：不開二選一，原因直接報（2026-10-12 作者裁決）
      if (kind === "not_sent") {
        setStatusMessage(stripTransportPrefix(String(reason)));
        return;
      }
      // 送出後失敗／解析不出＝不偽造證據：無判官句、展開兩選項、預設介面優先（2026-08-14 拍板），框內附原因
      setModeAsk({
        recommend: null,
        evidence: "",
        expanded: true,
        picked: "interface",
        ticket: null,
        failure: { kind, raw: String(reason) },
      });
    }
  }

  // 玩家選定玩法後的重構主體（none 卡直接以 characters 進來、無 resume 憑證）。
  async function startRefactorRun(mode: RefactorMode, ticket: RefactorRunTicket | null = null) {
    cancelRef.current = false;
    setCancelled(false);
    progressGen.current += 1;
    setProgress({ text: t("refactorSurveying"), cancelling: false, tail: "" });
    try {
      // 共用 tail：所有呼叫（survey＋展開）的 Channel onDelta 都 append 進同一個 buffer，
      // 任一路增量＝活著訊號，不因並行而互相蓋掉彼此的畫面。
      let tailBuffer = "";
      const appendTail = (delta: string) => {
        tailBuffer = (tailBuffer + delta).slice(-2000);
        setProgress((current) =>
          current && { ...current, tail: tailBuffer.split("\n").slice(-4).join("\n") },
        );
      };
      const makeOnDelta = () => {
        const channel = new Channel<string>();
        channel.onmessage = appendTail;
        return channel;
      };

      // 世界書現讀：重新重構剛清回原卡時，外層傳進來的條目清單還是清回前的
      const entries = await invoke<WorldbookEntry[]>("read_worldbook", { worldId: world });
      const survey = await invoke<RefactorSurveyOutcome>("refactor_survey", {
        worldId: world,
        mode,
        runId: ticket?.runId ?? null,
        fingerprint: ticket?.fingerprint ?? null,
        onDelta: makeOnDelta(),
      });
      // 本地零呼叫組裝：carry／split 各路由／clean 人物，毫秒級、不算進並行呼叫額度。
      const local = await invoke<RefactorLocalAssembly>("refactor_assemble_local", { worldId: world, survey });
      const { local: localPersons, queue } = buildRefactorPersonPlan(
        survey,
        entries,
        local.clean_person_names,
      );

      const absorbUids = survey.verdicts
        .filter((verdict) => verdict.action === "absorb")
        .map((verdict) => verdict.uid);
      // statusbar 段依來源 uid 分組：同一條原始條目的多個 statusbar span 合成一次呼叫。
      const statusbarByUid = new Map<string, string[]>();
      for (const route of survey.splits) {
        if (route.route !== "statusbar") continue;
        const uid = route.span.split("#")[0];
        statusbarByUid.set(uid, [...(statusbarByUid.get(uid) ?? []), route.span]);
      }

      // 全部呼叫進同一個 pool，上限 4 有界並行；不再有「重寫→介面」序列鏈。
      type RefactorTask =
        | { kind: "person"; item: RefactorPersonQueueItem }
        | { kind: "absorb"; uid: string }
        | { kind: "group"; group: RefactorSplitGroup }
        | { kind: "statusbar"; uid: string; spans: string[] }
        | { kind: "interface"; uid: string };
      // 角色優先＝介面產物一律不建：interface／statusbar 呼叫整個不發（refactor-mode-split；
      // 這些條目與段落的下落改由 mode-aware 稽核記入 dropped，包 3）。
      const buildInterfaces = mode !== "characters";
      // 外框候選先不展開：等定義條目展開完、比對骨架容器後才決定當外框還是照常展開（refactor-frame.ts）
      const frameCandidates = buildInterfaces ? (survey.frame_candidates ?? []) : [];
      const pool: RefactorTask[] = [
        ...queue.map((item): RefactorTask => ({ kind: "person", item })),
        ...absorbUids.map((uid): RefactorTask => ({ kind: "absorb", uid })),
        ...survey.groups.map((group): RefactorTask => ({ kind: "group", group })),
        ...(buildInterfaces
          ? [...statusbarByUid.entries()].map(
              ([uid, spans]): RefactorTask => ({ kind: "statusbar", uid, spans }),
            )
          : []),
        ...(buildInterfaces
          ? survey.interface_uids
              .filter((uid) => !frameCandidates.some((candidate) => candidate.uid === uid))
              .map((uid): RefactorTask => ({ kind: "interface", uid }))
          : []),
      ];
      const totalSteps = pool.length + frameCandidates.length;

      const characters: RefactorCharacter[] = [...local.characters, ...localPersons];
      const refactorEntries: RefactorNewEntry[] = [...local.entries];
      // 淘汰／未收編／稽核有東西＝不是「無事可做」：純介面卡選 characters 時產物只剩 rule 5
      // 淘汰清單，也要開結果視窗——玩家看得到可放回，套用後 mode 才落地、介面 fallback 才停。
      const localNotes = local.dropped.length + local.unabsorbed.length + local.audit.length;
      if (
        totalSteps === 0 &&
        characters.length === 0 &&
        refactorEntries.length === 0 &&
        localNotes === 0
      ) {
        setProgress(null);
        setStatusMessage(t("refactorNothingToDo"));
        return;
      }

      const interfaces: RefactorInterface[] = [];
      // reason 帶原始錯誤文字（去重顯示在結果視窗）：玩家看得到「模型呼叫失敗」這類可修正原因。
      const failedTitles: { name: string; reason: string }[] = [];
      // 介面展開成功的條目名稱：合併衝突時整組記成失敗，來源不被消耗
      const interfaceNames: string[] = [];
      // 介面合併衝突時要保留的來源 uid；其餘失敗、取消、沒跑到的任務由 succeeded 推回來
      const conflictUids: string[] = [];
      // 成功完成的任務：沒在這裡的任務（失敗、取消時中斷、取消後沒發出）所用的來源一律保留
      const succeeded = new Set<RefactorTask>();
      const knownFields = survey.fields; // 命名唯一權威，全呼叫共用同一份、不累積。
      let done = 0;
      const bumpDone = () => {
        done++;
        setProgress(
          (current) =>
            current && { ...current, text: t("refactorParallelStep", { done, total: totalSteps }) },
        );
      };

      setProgress(
        (current) =>
          current && { ...current, text: t("refactorParallelStep", { done, total: totalSteps }) },
      );

      const run = async (task: RefactorTask): Promise<void> => {
        const name =
          task.kind === "person"
            ? task.item.name
            : task.kind === "group"
              ? task.group.title
              : sourceEntryTitle(entries, task.uid);
        const fail = (reason: string) => {
          failedTitles.push({ name, reason });
        };
        try {
          if (task.kind === "person") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorPersonExpandOutcome>("refactor_expand_person", {
                  worldId: world,
                  name: task.item.name,
                  uids: task.item.uids,
                  isPlayer: task.item.is_player,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.character) {
              characters.push(result.character);
              succeeded.add(task);
            }
            else fail("");
          } else if (task.kind === "absorb") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorRewriteOutcome>("refactor_absorb_entry", {
                  worldId: world,
                  entryUid: task.uid,
                  knownFields,
                  mode,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.entry) {
              refactorEntries.push(result.entry);
              succeeded.add(task);
            }
            else fail("");
          } else if (task.kind === "group") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorRewriteOutcome>("refactor_split_group", {
                  worldId: world,
                  groupId: task.group.id,
                  title: task.group.title,
                  kind: task.group.kind,
                  spans: task.group.spans,
                  knownFields,
                  mode,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.entry) {
              refactorEntries.push(result.entry);
              succeeded.add(task);
            }
            else fail("");
          } else if (task.kind === "statusbar") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorExpandOutcome>("refactor_expand_spans", {
                  worldId: world,
                  entryUid: task.uid,
                  spans: task.spans,
                  knownFields,
                  mode,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.interface) {
              interfaces.push(result.interface);
              interfaceNames.push(name);
              succeeded.add(task);
            } else fail(result.error ?? "");
          } else {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorExpandOutcome>("refactor_expand", {
                  worldId: world,
                  entryUid: task.uid,
                  // 兩種介面都產骨架與回報規矩；playable 只決定骨架規格的開頭段
                  kind: survey.playable_interface_uids.includes(task.uid)
                    ? "interface_shell"
                    : "interface_statusbar",
                  knownFields,
                  mode,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.interface) {
              interfaces.push(result.interface);
              interfaceNames.push(name);
              succeeded.add(task);
            } else fail(result.error ?? "");
          }
        } catch (reason) {
          if (!String(reason).includes("refactor-aborted")) {
            fail(String(reason));
          }
        } finally {
          bumpDone();
        }
      };

      // chain 恆空：survey 同一 run 已建快取，warmed=true 跳過首發獨跑，pool 直接全並行開跑。
      await runRefactorCalls({
        chain: [],
        pool: [...pool],
        limit: REFACTOR_PARALLEL_LIMIT,
        isCancelled: () => cancelRef.current,
        run,
        warmed: true,
      });

      // 第二階段：外框候選對上定義骨架的容器才當外框（零呼叫），其餘照一般介面條目展開。取消時沒走到
      // 這裡的候選全部保留來源。
      let frames: RefactorFrameCandidate[] = [];
      const resolvedFrameUids = new Set<string>();
      if (frameCandidates.length > 0 && !cancelRef.current) {
        const confirmed = confirmFrames(
          frameCandidates,
          interfaces.map((candidate) => candidate.shell ?? ""),
        );
        frames = confirmed.frames;
        for (const frame of frames) {
          resolvedFrameUids.add(frame.uid);
          bumpDone();
        }
        const second = confirmed.expand.map(
          (candidate): RefactorTask => ({ kind: "interface", uid: candidate.uid }),
        );
        for (const task of second) resolvedFrameUids.add(refactorTaskUids(task)[0]);
        pool.push(...second);
        await runRefactorCalls({
          chain: [],
          pool: second,
          limit: REFACTOR_PARALLEL_LIMIT,
          isCancelled: () => cancelRef.current,
          run,
          warmed: true,
        });
      }
      const unresolvedFrameUids = frameCandidates
        .map((candidate) => candidate.uid)
        .filter((uid) => !resolvedFrameUids.has(uid));

      setProgress(null);
      if (
        characters.length > 0 ||
        interfaces.length > 0 ||
        refactorEntries.length > 0 ||
        localNotes > 0
      ) {
        // 多條介面產物互相衝突（同一欄位兩份值不同、骨架對不上欄位）：整組不套，參與的條目都記成失敗
        // 容器角色只算介面候選用到的來源條目
        const interfaceUids = new Set(interfaces.flatMap((candidate) => candidate.source_uids));
        const containerRoles = Object.fromEntries(
          entries
            .filter((entry) => interfaceUids.has(String(entry.uid)))
            .map((entry) => [String(entry.uid), sourceContainerRoles(entry.content)]),
        );
        const merged = mergeRefactorInterfaces(interfaces, frames, containerRoles);
        if (merged.conflict !== null) {
          for (const name of interfaceNames) {
            failedTitles.push({ name, reason: t("refactorInterfaceConflict", { path: merged.conflict }) });
          }
          for (const candidate of interfaces) conflictUids.push(...candidate.source_uids);
        }
        // 外框組不進骨架（或整組衝突沒有骨架可組）：來源保留、記成失敗
        for (const uid of merged.failedFrames) {
          conflictUids.push(uid);
          if (merged.conflict === null) {
            failedTitles.push({ name: sourceEntryTitle(entries, uid), reason: t("refactorFrameUnmatched") });
          }
        }
        conflictUids.push(...unresolvedFrameUids);
        const nextOutcome = assembleRefactorOutcome({
          characters,
          interfaces: merged.conflict === null ? interfaces : [],
          frames: merged.conflict === null ? frames : [],
          containerRoles,
          entries: refactorEntries,
          dropped: local.dropped,
          unabsorbed: local.unabsorbed,
          audit: local.audit,
          mode,
          sourceFingerprints: survey.source_fingerprints,
          mirrors: survey.mirrors,
          stageCards: survey.stage_cards,
          preserveSourceUids: unfinishedSourceUids(
            pool.map((task) => ({ uids: refactorTaskUids(task), succeeded: succeeded.has(task) })),
            conflictUids,
          ),
        });
        setOutcome(nextOutcome);
        setSelection(defaultRefactorSelection(nextOutcome));
        setApplied(null);
        setOrigin("ai");
        setDetail(false);
        setCancelled(cancelRef.current);
      }
      setFailures(failedTitles);
    } catch (reason) {
      setProgress(null);
      if (String(reason).includes("refactor-aborted")) return;
      // MODE 回聲核對不過：判官跑錯玩法整份拒收（後端固定字串），換成玩家看得懂的一句
      if (String(reason).includes("refactor-mode-mismatch")) {
        setStatusMessage(t("refactorModeMismatch"));
        return;
      }
      setStatusMessage(String(reason));
    }
  }

  // 取消：擋「還沒發的下一條」＋後端 abort 在途呼叫（refactor_abort，包 2 交付）——已經在燒
  // 的那幾條立刻中止，中止錯誤走 sentinel "refactor-aborted" 靜默略過，不列入失敗。
  // abort 失敗（只會是 IPC 層）：還原取消鈕讓玩家能再按，原因報狀態列；cancelRef 保持 true，不再發下一條。
  // 只在發起取消的那一桌、那一段進度還在時才還原，不能解除別桌或下一段的取消鎖。
  function cancelAiRefactor() {
    cancelRef.current = true;
    setProgress((current) => current && { ...current, cancelling: true });
    const origin = world;
    const gen = progressGen.current;
    invoke("refactor_abort", { worldId: origin }).catch((reason) => {
      if (!mounted.current || worldRef.current !== origin || progressGen.current !== gen) return;
      setProgress((current) => current && { ...current, cancelling: false });
      setStatusMessage(String(reason));
    });
  }

  // 匯入重構卡（也是零額度測試入口）：.json 與 PNG 都以原始 bytes 交後端同一個入口
  // （大小上限、封套與整包驗證、角色圖暫存），封套回來前端再驗一次。每個 await 之後
  // 核對請求世代：開了別的檔、關了結果卡或換了桌，舊回覆作廢並釋放它佔的槽。
  async function pickRefactorOutcome(file: File) {
    setStatusMessage("");
    const openedFor = world;
    const seq = ++openSeq.current;
    const stale = () => seq !== openSeq.current || !mounted.current || worldRef.current !== openedFor;
    let opened: RefactorCardOpenResult | null = null;
    try {
      // 桌況閘門（拖入與按鈕選檔兩個入口都走這裡，與 AI 重構同一套）：已遊玩／沒有原檔先擋；
      // 重構過的桌要等卡檔驗過、真的有東西可套，才確認並清回
      const rerun = await checkRerun(stale);
      if (rerun === false) return;
      const bytes = new Uint8Array(await file.arrayBuffer());
      if (stale()) return;
      opened = await invoke<RefactorCardOpenResult>("refactor_card_open", bytes, {
        headers: { "tt-world-id": openedFor },
      });
      const token = opened.token ? { world: openedFor, token: opened.token } : null;
      if (stale()) {
        releaseToken(token);
        return;
      }
      const card: RefactorCard = parseRefactorCardValue(opened.card);
      if (rerun === "ready" && !(await resetForRerun(stale))) {
        releaseToken(token);
        return;
      }
      // 舞台卡名照這一桌（清回原卡之後）過濾：重構卡可能來自別桌，對不到的不列、不預勾
      const stageCards = card.outcome.stage_cards?.length
        ? await invoke<string[]>("refactor_stage_cards", {
            worldId: openedFor,
            names: card.outcome.stage_cards,
          })
        : [];
      if (stale()) {
        releaseToken(token);
        return;
      }
      const outcome: RefactorOutcome = { ...card.outcome, stage_cards: stageCards };
      if (stageCards.length === 0) delete outcome.stage_cards;
      if (heldToken.current?.token !== token?.token) releaseHeldAssets();
      heldToken.current = token;
      setAssetCount(opened?.assets.length ?? 0);
      setOutcome(outcome);
      setSelection(defaultRefactorSelection(outcome, card.applied));
      setApplied(card.applied);
      setOrigin("import");
      setDetail(false);
    } catch (reason) {
      // 前端複驗沒過：後端已暫存的圖一併釋放
      if (opened?.token) releaseToken({ world: openedFor, token: opened.token });
      if (stale()) return;
      const code = reason instanceof Error ? reason.message : "";
      setStatusMessage(
        code === REFACTOR_IMPORT_INVALID
          ? t("refactorImportInvalid")
          : code === REFACTOR_IMPORT_NEWER
            ? t("refactorImportNewer")
            : String(reason),
      );
    }
  }

  function closeRefactor() {
    openSeq.current += 1;
    if (applyPhase.current === "idle") releaseHeldAssets();
    setAssetCount(0);
    setOutcome(null);
    setSelection(null);
    setApplied(null);
    setOrigin(null);
    setDetail(false);
    setFailures([]);
    setCancelled(false);
  }

  // 已淘汰清單的「放回」：零後端行為，走既有 entries 勾選路徑——套用時跟其他世界書條目一視同仁。
  function restoreDroppedItem(index: number) {
    if (!outcome || !selection) return;
    const result = restoreDropped(outcome, selection, index);
    setOutcome(result.outcome);
    setSelection(result.selection);
  }

  // 回合進行中會排在它後面（後端持整桌獨占）；排隊期間換桌或關掉世界設定，套用照樣在原桌完成，
  // 但 live() 為 false 就不關結果卡、不刷新、不跳訊息
  async function applyRefactor(nextSelection: RefactorSelection) {
    // 一項都沒勾＝不動桌子〔作者裁決 2026-10-27〕：摘要與展開兩個入口都經過這裡
    if (!outcome || busy || selectionIsEmpty(nextSelection)) return;
    setStatusMessage("");
    setBusy(true);
    applyPhase.current = "waiting";
    const held = heldToken.current;
    const assetToken = held?.token ?? null;
    await runQueued(async ({ live, backend }) => {
      try {
        const summary = await backend(() => {
          applyPhase.current = "submitted";
          return invoke<RefactorApplySummary>("refactor_apply", {
            worldId: world,
            outcome,
            selection: nextSelection,
            recordReceipt: origin !== "ai",
            assetToken,
          });
        });
        // 後端已用掉素材
        if (heldToken.current?.token === assetToken) heldToken.current = null;
        applyPhase.current = "idle";
        if (!live()) return;
        closeRefactor();
        await refreshAfterApply(live, summary.entry_changes ?? null);
        if (!live()) return;
        await showMessage(refactorApplyMessage(summary), {
          title: t("refactorBtn"),
          okLabel: t("dialogAck"),
        });
      } catch (reason) {
        const gone = assetToken !== null && String(reason).includes("refactor_assets_gone");
        // 角色圖已不在暫存（開了別張卡）：token 作廢；這張結果卡套不完整了，清掉讓玩家重新開檔
        if (gone && heldToken.current?.token === assetToken) heldToken.current = null;
        applyPhase.current = "idle";
        if (live() && gone) closeRefactor();
        // 後端判已遊玩（開著結果卡時桌上又有了回合）：換成同一則開新桌提醒
        if (live() && String(reason).includes(REFACTOR_PLAYED)) {
          setStatusMessage("");
          await showMessage(t("refactorRerunPlayed"), {
            title: t("refactorBtn"),
            kind: "warning",
            okLabel: t("dialogAck"),
          });
        } else if (live() && String(reason).includes(REFACTOR_EMPTY_SELECTION)) {
          setStatusMessage(t("refactorEmptySelection"));
        } else if (live()) setStatusMessage(String(reason));
      } finally {
        // 按鈕鎖一律放開（卸載後 React 不會套用），換桌後留著的面板不能永遠鎖死
        setBusy(false);
      }
    });
    // 沒送到後端（等回合時換桌或卸載）或後端拒套放回：元件已卸載就沒人會再關這張卡，這裡釋放
    applyPhase.current = "idle";
    if (!mounted.current && heldToken.current === held) releaseHeldAssets();
  }

  // 匯出結果卡上這份還沒套用（或剛套用完）的產物，供之後用「匯入重構卡」讀回重玩。
  async function exportRefactorOutcome() {
    if (!outcome || busy) return;
    setStatusMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName, false),
        filters: refactorCardFilters(false),
      });
      if (!path) return;
      const bytes = await invoke<number>("refactor_export_outcome", { worldId: world, outcome, path });
      setStatusMessage(exportedMessage(bytes));
      await revealItemInDir(path);
    } catch (reason) {
      setStatusMessage(String(reason));
    }
  }

  return {
    outcome,
    selection,
    applied,
    assetCount,
    origin,
    detail,
    cancelled,
    modeAsk,
    failures,
    busy,
    waitingForTurn,
    progress,
    inputRef,
    setSelection,
    setDetail,
    setModeAsk,
    runAiRefactor,
    startRefactorRun,
    cancelAiRefactor,
    pickRefactorOutcome,
    closeRefactor,
    restoreDroppedItem,
    applyRefactor,
    exportRefactorOutcome,
    exportSavedRefactorOutcome,
  };
}

export type RefactorWorkflowController = ReturnType<typeof useRefactorWorkflow>;
