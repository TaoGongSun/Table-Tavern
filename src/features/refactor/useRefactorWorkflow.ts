import { useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t } from "../../i18n";
import {
  assembleRefactorOutcome,
  buildRefactorPersonPlan,
  defaultRefactorSelection,
  parseRefactorOutcome,
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
} from "./refactor-review";
import { REFACTOR_PARALLEL_LIMIT, runRefactorCalls, withRateLimitRetry } from "./refactor-run";
import {
  detectRefactorTristate,
  type RefactorMode,
  type RefactorRecommendOutcome,
  type RefactorRunTicket,
} from "./refactor-mode";
import type { CardInterface } from "../card-interface/interface-card";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";

export interface RefactorModeAsk {
  recommend: RefactorMode | null;
  evidence: string;
  expanded: boolean;
  picked: RefactorMode;
  /** 第二段 resume 憑證；null＝初判失敗或非 claude lane，第二段直接重送全卡。 */
  ticket: RefactorRunTicket | null;
}

interface UseRefactorWorkflowOptions {
  world: string;
  worldName: string;
  entries: WorldbookEntry[];
  setStatusMessage: (message: string) => void;
  refreshAfterApply: () => Promise<void>;
}

// 重構卡存檔對話框預設檔名：桌名可能含檔名非法字元，一律代換成 -；空桌名就不接前綴，只用在地化字尾
function refactorCardFileName(tableName: string): string {
  const safe = tableName.replace(/[\\/:*?"<>|\x00-\x1f\x7f]/g, "-");
  return `${safe ? `${safe}-` : ""}${t("refactorExportFileName")}.json`;
}

function refactorApplyMessage(summary: RefactorApplySummary) {
  return [
    summary.new_characters > 0 && t("refactorApplyDoneCharacters", { n: summary.new_characters }),
    summary.player_assigned && t("refactorApplyDonePlayer"),
    summary.new_entries > 0 && t("refactorApplyDoneEntries", { n: summary.new_entries }),
    summary.deleted_entries > 0 && t("refactorApplyDoneDeleted", { n: summary.deleted_entries }),
    summary.interface_applied && t("refactorApplyDoneInterface"),
    summary.mechanisms_applied > 0 && t("refactorApplyDoneMechanisms", { n: summary.mechanisms_applied }),
  ]
    .filter(Boolean)
    .join("・");
}

export function useRefactorWorkflow({
  world,
  worldName,
  entries,
  setStatusMessage,
  refreshAfterApply,
}: UseRefactorWorkflowOptions) {
  // AI 卡重構：結果卡（產物讀進來後的人審／套用）與下面的「盤點→展開」進度是兩段獨立狀態，
  // 交會點是 setRefactorOutcome——AI 兩階段跑完、或選檔路徑讀完 JSON，都寫進同一份結果卡。
  const [outcome, setOutcome] = useState<RefactorOutcome | null>(null);
  const [selection, setSelection] = useState<RefactorSelection | null>(null);
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
  const inputRef = useRef<HTMLInputElement>(null);
  // 非 null＝AI 盤點／展開跑中，modal 顯示 text；cancelling 只管取消鈕的 disabled，不影響迴圈判斷。
  const [progress, setProgress] = useState<{ text: string; cancelling: boolean; tail: string } | null>(null);
  // 迴圈裡讀取的取消旗標——用 ref 而非 state：async 迴圈裡的閉包看不到後續 setState，只有 ref.current 每次都讀最新值。
  const cancelRef = useRef(false);

  // 匯出這桌先前套用過的重構產物（apply() 落檔），重玩同一張卡不必再燒 AI 額度重新展開。
  async function exportSavedRefactorOutcome() {
    setStatusMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName),
        filters: [{ name: t("refactorOutcomeJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("refactor_export_saved", { worldId: world, path });
      await revealItemInDir(path);
    } catch (reason) {
      setStatusMessage(
        String(reason).includes("refactor-export-none") ? t("refactorExportNone") : String(reason),
      );
    }
  }

  // AI 卡重構：盤點出六區塊小抄（PERSONS／INTERFACE／ENTRIES／SPLITS／GROUPS／FIELDS）→本地
  // 零呼叫組裝（refactor_assemble_local：carry 照搬＋split 零呼叫路由＋clean 人物組卡）→剩餘
  // AI 呼叫全並行（人物佇列＋absorb＋group＋statusbar＋interface，上限 4、無序列鏈）→組產物。
  // knownFields 是 survey.fields 固定一份，所有呼叫共用，不再沿呼叫鏈累積。
  // 入口：三態偵測分流（refactor-mode-split）。supported 卡先跑初判再彈二選一；
  // none 是唯一免問的路（直跑角色線）；unsupported（DRM／雲端載入器）擋下不跑。
  async function runAiRefactor() {
    if (progress) return;
    if (await invoke<boolean>("refactor_outcome_exists", { worldId: world })) {
      const rerun = await confirm(t("refactorRerunWarnBody"), {
        title: t("refactorBtn"),
        kind: "warning",
      });
      if (!rerun) return;
    }
    setStatusMessage("");
    const cards = await invoke<CardInterface[]>("card_interfaces", { worldId: world }).catch(
      () => [] as CardInterface[],
    );
    const tristate = detectRefactorTristate(cards);
    if (tristate === "unsupported") {
      setStatusMessage(t("refactorUnsupportedCard"));
      return;
    }
    if (tristate === "none") {
      await startRefactorRun("characters");
      return;
    }
    // supported：初判帶全卡只出兩行；取消走既有 refactor_abort 路。
    cancelRef.current = false;
    setCancelled(false);
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
      const recommend: RefactorMode = probe.recommend === "characters" ? "characters" : "interface";
      setModeAsk({
        recommend,
        evidence: probe.evidence,
        expanded: false,
        picked: recommend,
        ticket: probe.run_id ? { runId: probe.run_id, fingerprint: probe.fingerprint } : null,
      });
    } catch (reason) {
      setProgress(null);
      if (String(reason).includes("refactor-aborted")) return;
      // 初判失敗＝不偽造證據：no 判官句、直接展開兩選項、預設介面優先（2026-08-14 拍板）
      setModeAsk({ recommend: null, evidence: "", expanded: true, picked: "interface", ticket: null });
    }
  }

  // 玩家選定玩法後的重構主體（none 卡直接以 characters 進來、無 resume 憑證）。
  async function startRefactorRun(mode: RefactorMode, ticket: RefactorRunTicket | null = null) {
    cancelRef.current = false;
    setCancelled(false);
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
          ? survey.interface_uids.map((uid): RefactorTask => ({ kind: "interface", uid }))
          : []),
      ];
      const totalSteps = pool.length;

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
            if (result.character) characters.push(result.character);
            else failedTitles.push({ name, reason: "" });
          } else if (task.kind === "absorb") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorRewriteOutcome>("refactor_absorb_entry", {
                  worldId: world,
                  entryUid: task.uid,
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.entry) refactorEntries.push(result.entry);
            else failedTitles.push({ name, reason: "" });
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
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.entry) refactorEntries.push(result.entry);
            else failedTitles.push({ name, reason: "" });
          } else if (task.kind === "statusbar") {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorExpandOutcome>("refactor_expand_spans", {
                  worldId: world,
                  entryUid: task.uid,
                  spans: task.spans,
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.interface) interfaces.push(result.interface);
            else failedTitles.push({ name, reason: "" });
          } else {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorExpandOutcome>("refactor_expand", {
                  worldId: world,
                  entryUid: task.uid,
                  kind: survey.playable_interface_uids.includes(task.uid)
                    ? "interface_shell"
                    : "interface",
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => cancelRef.current,
            );
            if (result.interface) interfaces.push(result.interface);
            else failedTitles.push({ name, reason: "" });
          }
        } catch (reason) {
          if (!String(reason).includes("refactor-aborted")) {
            failedTitles.push({ name, reason: String(reason).slice(0, 200) });
          }
        } finally {
          bumpDone();
        }
      };

      // chain 恆空：survey 同一 run 已建快取，warmed=true 跳過首發獨跑，pool 直接全並行開跑。
      await runRefactorCalls({
        chain: [],
        pool,
        limit: REFACTOR_PARALLEL_LIMIT,
        isCancelled: () => cancelRef.current,
        run,
        warmed: true,
      });

      setProgress(null);
      if (
        characters.length > 0 ||
        interfaces.length > 0 ||
        refactorEntries.length > 0 ||
        localNotes > 0
      ) {
        const nextOutcome = assembleRefactorOutcome({
          characters,
          interfaces,
          entries: refactorEntries,
          dropped: local.dropped,
          unabsorbed: local.unabsorbed,
          audit: local.audit,
          mode,
        });
        setOutcome(nextOutcome);
        setSelection(defaultRefactorSelection(nextOutcome));
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
  function cancelAiRefactor() {
    cancelRef.current = true;
    setProgress((current) => current && { ...current, cancelling: true });
    void invoke("refactor_abort", { worldId: world });
  }

  // AI 卡重構：零額度測試用入口——直接餵一份產物 JSON，跳過真 AI 呼叫，驗證人審／套用路徑用。
  async function pickRefactorOutcome(file: File) {
    setStatusMessage("");
    try {
      const nextOutcome = parseRefactorOutcome(await file.text());
      setOutcome(nextOutcome);
      setSelection(defaultRefactorSelection(nextOutcome));
      setOrigin("import");
      setDetail(false);
    } catch (reason) {
      const invalid = reason instanceof Error && reason.message === REFACTOR_IMPORT_INVALID;
      setStatusMessage(invalid ? t("refactorImportInvalid") : String(reason));
    }
  }

  function closeRefactor() {
    setOutcome(null);
    setSelection(null);
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

  async function applyRefactor(nextSelection: RefactorSelection) {
    if (!outcome || busy) return;
    setStatusMessage("");
    setBusy(true);
    try {
      const summary = await invoke<RefactorApplySummary>("refactor_apply", {
        worldId: world,
        outcome,
        selection: nextSelection,
        recordReceipt: origin !== "ai",
      });
      closeRefactor();
      await refreshAfterApply();
      await showMessage(refactorApplyMessage(summary), { title: t("refactorBtn") });
    } catch (reason) {
      setStatusMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }

  // 匯出結果卡上這份還沒套用（或剛套用完）的產物，供之後用「匯入重構卡」讀回重玩。
  async function exportRefactorOutcome() {
    if (!outcome || busy) return;
    setStatusMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName),
        filters: [{ name: t("refactorOutcomeJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("refactor_export_outcome", { outcome, path });
      await revealItemInDir(path);
    } catch (reason) {
      setStatusMessage(String(reason));
    }
  }

  return {
    outcome,
    selection,
    origin,
    detail,
    cancelled,
    modeAsk,
    failures,
    busy,
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
