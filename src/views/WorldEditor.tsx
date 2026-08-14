import { FormEvent, Fragment, useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { confirm, message as showMessage, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { t, type MsgKey } from "../i18n";
import {
  assembleRefactorOutcome,
  buildRefactorPersonPlan,
  defaultRefactorSelection,
  parseRefactorOutcome,
  refactorSummaryCounts,
  REFACTOR_IMPORT_INVALID,
  restoreDropped,
  setPlayerIndex,
  sourceEntryTitle,
  sourceEntryTitles,
  toggleIndex,
  unselectCharacter,
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
} from "../refactor-review";
import { REFACTOR_PARALLEL_LIMIT, runRefactorCalls, withRateLimitRetry } from "../refactor-run";
import { detectRefactorTristate, type RefactorMode, type RefactorRecommendOutcome, type RefactorRunTicket } from "../refactor-mode";
import { type CardInterface } from "../interface-card";
import { Visibility, WorldbookEntry } from "../backend-contracts";
import { CharacterMeta } from "../card-model";
import { useDragReorder } from "../drag-reorder";

interface WorldbookDraft {
  uid: number | null;
  title: string;
  keys: string;
  content: string;
  constant: boolean;
  enabled: boolean;
  order: number;
  visibility: Visibility["type"];
  characters: string[];
}

// 機制帳本：世界書分頁「哪些條目被本地機制接管／跳過」面板，對應 mechanism.rs 的 Ledger。
type RecordKind = "rejected" | "clamped" | "error" | "absorbed" | "skipped" | "jump";

interface LedgerEntry {
  uid: number;
  title: string;
  kind: RecordKind;
  detail: string;
  sent: boolean;
}

interface Ledger {
  entries: LedgerEntry[];
  rejected: number;
  clamped: number;
  errors: number;
  jumps: number;
}

const EMPTY_LEDGER: Ledger = { entries: [], rejected: 0, clamped: 0, errors: 0, jumps: 0 };

// 重構卡存檔對話框預設檔名：桌名可能含檔名非法字元，一律代換成 -；空桌名就不接前綴，只用在地化字尾
function refactorCardFileName(tableName: string): string {
  const safe = tableName.replace(/[\\/:*?"<>|\x00-\x1f\x7f]/g, "-");
  return `${safe ? `${safe}-` : ""}${t("refactorExportFileName")}.json`;
}

// 淘汰理由 rule／稽核 kind 都是後端固定枚舉，本地寫死對照 i18n 鍵；查不到就退第一種，不讓畫面空白。
const REFACTOR_DROPPED_RULE_KEYS: Record<number, MsgKey> = {
  1: "refactorDroppedRule1",
  2: "refactorDroppedRule2",
  3: "refactorDroppedRule3",
  4: "refactorDroppedRule4",
  5: "refactorDroppedRule5",
};
const REFACTOR_AUDIT_KIND_KEYS: Record<string, MsgKey> = {
  coverage: "refactorAuditKindCoverage",
  mechanism: "refactorAuditKindMechanism",
  split: "refactorAuditKindSplit",
  drop_rule: "refactorAuditKindDropRule",
  excused: "refactorAuditKindExcused",
};

// 世界書 v1：一份只進 GM 上下文的 world.md（NewPlan §7.0）
export function WorldEditor({
  world,
  worldName,
  onBack,
  leaveGuard,
  convertColor,
  onEntryConverted,
  onRefactorApplied,
}: {
  world: string;
  worldName: string;
  onBack: () => void;
  /** 側欄要離開世界設定時先問過這裡（未儲存確認與返回鈕同一條） */
  leaveGuard: { current: (() => Promise<boolean>) | null };
  convertColor: string;
  onEntryConverted: () => Promise<void>;
  /** AI 卡重構套用成功後：角色清單／卡片介面／桌面狀態都可能變了，交回 App 層重載 */
  onRefactorApplied: () => Promise<void>;
}) {
  const [text, setText] = useState<string | null>(null);
  const [savedText, setSavedText] = useState("");
  const [message, setMessage] = useState("");
  const [entries, setEntries] = useState<WorldbookEntry[]>([]);
  const [ledger, setLedger] = useState<Ledger>(EMPTY_LEDGER);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const [worldbookMessage, setWorldbookMessage] = useState("");
  const [draft, setDraft] = useState<WorldbookDraft | null>(null);
  // 條目表單開啟當下的快照，用來判斷「有沒有改過」（未儲存提示）
  const [draftOrigin, setDraftOrigin] = useState("");
  const draftFormRef = useRef<HTMLFormElement>(null);
  const entryDrag = useDragReorder(
    entries,
    (entry) => String(entry.uid),
    (ordered) => void reorderEntries(ordered),
  );
  // AI 卡重構：結果卡（產物讀進來後的人審／套用）與下面的「盤點→展開」進度是兩段獨立狀態，
  // 交會點是 setRefactorOutcome——AI 兩階段跑完、或選檔路徑讀完 JSON，都寫進同一份結果卡。
  const [refactorOutcome, setRefactorOutcome] = useState<RefactorOutcome | null>(null);
  const [refactorSelection, setRefactorSelection] = useState<RefactorSelection | null>(null);
  const [refactorOrigin, setRefactorOrigin] = useState<"ai" | "import" | null>(null);
  const [refactorDetail, setRefactorDetail] = useState(false);
  // 取消後仍組出的半成品：中止的呼叫不會留下任何痕跡（產物沒 push、也不列失敗），
  // 面板自己說出來才看得見缺件——標題改「已取消」、主按鈕換成「不要」。
  const [refactorCancelled, setRefactorCancelled] = useState(false);
  // 二選一對話框（refactor-mode-split）：recommend null＝初判失敗（不偽造證據、直接展開兩選項
  // 預設介面優先）；expanded＝玩家按了「自己選」看得到兩張選項卡。
  const [refactorModeAsk, setRefactorModeAsk] = useState<{
    recommend: RefactorMode | null;
    evidence: string;
    expanded: boolean;
    picked: RefactorMode;
    /** 第二段 resume 憑證；null＝初判失敗或非 claude lane，第二段直接重送全卡。 */
    ticket: RefactorRunTicket | null;
  } | null>(null);
  // pool 呼叫失敗的條目名單（2026-08-12 B 拍板）：顯示在結果視窗頂部紅字段——以前塞頁面
  // 角落的一行狀態文字，被結果 modal 蓋住玩家看不到。
  const [refactorFailures, setRefactorFailures] = useState<{ name: string; reason: string }[]>([]);
  const [refactorBusy, setRefactorBusy] = useState(false);
  const refactorInputRef = useRef<HTMLInputElement>(null);
  // 非 null＝AI 盤點／展開跑中，modal 顯示 text；cancelling 只管取消鈕的 disabled，不影響迴圈判斷。
  const [refactorProgress, setRefactorProgress] = useState<{ text: string; cancelling: boolean; tail: string } | null>(null);
  // 迴圈裡讀取的取消旗標——用 ref 而非 state：async 迴圈裡的閉包看不到後續 setState，只有 ref.current 每次都讀最新值。
  const refactorCancelRef = useRef(false);

  async function refreshCast() {
    try {
      const cast = await invoke<CharacterMeta[]>("list_characters", { worldId: world });
      setCharacters(cast.filter((character) => !character.archived));
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  useEffect(() => {
    setMessage("");
    setWorldbookMessage("");
    setText(null);
    setEntries([]);
    setLedger(EMPTY_LEDGER);
    setCharacters([]);
    setDraft(null);
    invoke<string>("read_world_md", { worldId: world })
      .then((value) => {
        setText(value);
        setSavedText(value);
      })
      .catch((reason) => setMessage(String(reason)));
    invoke<WorldbookEntry[]>("read_worldbook", { worldId: world })
      .then(setEntries)
      .catch((reason) => setWorldbookMessage(String(reason)));
    // 帳本掛掉不該擋住世界書編輯：失敗就當空，不彈錯誤。
    invoke<Ledger>("mechanism_ledger", { worldId: world })
      .then(setLedger)
      .catch(() => setLedger(EMPTY_LEDGER));
    void refreshCast();
  }, [world]);

  // 新增的空白表單排在清單底部，展開時可能在畫面外，捲到看得見
  // （不用 smooth：長清單的平滑捲動會被後續 render 打斷，停在半路）
  useEffect(() => {
    draftFormRef.current?.scrollIntoView({ block: "nearest" });
  }, [draftOrigin]);

  if (text === null) return message ? <p role="alert">{message}</p> : null;

  const draftDirty = draft !== null && JSON.stringify(draft) !== draftOrigin;
  // 既有條目改到一半離開時會自動存，不算未儲存；只有還沒存過的新條目要提醒
  const newEntryDirty = draftDirty && draft?.uid === null;
  const unsavedCount = (text !== savedText ? 1 : 0) + (newEntryDirty ? 1 : 0);

  async function saveWorldSettings(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    try {
      await invoke("write_world_md", { worldId: world, content: text });
      setSavedText(text ?? "");
      setMessage(t("saved"));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function confirmLeave() {
    // 開著的既有條目照換編輯對象那套：先存起來再走，存不起來就別走
    if (draft && draftDirty && draft.uid !== null) {
      if (!(await persistDraft(draft))) return false;
      setDraft(null);
    }
    if (unsavedCount === 0) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: unsavedCount }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }
  // 側欄切走時走的是同一條確認；每次 render 掛上，閉包才拿得到最新的 unsavedCount
  leaveGuard.current = confirmLeave;

  async function handleBack() {
    if (await confirmLeave()) onBack();
  }

  async function refreshWorldbook() {
    setEntries(await invoke<WorldbookEntry[]>("read_worldbook", { worldId: world }));
  }

  async function refreshLedger() {
    try {
      setLedger(await invoke<Ledger>("mechanism_ledger", { worldId: world }));
    } catch {
      setLedger(EMPTY_LEDGER);
    }
  }

  // 帳本的「照原文送模型」開關＝重用既有 upsert_worldbook_entry 反轉該條目的 disabled；
  // 找不到該 uid 就跳過（條目已被刪，不是這裡的錯）。
  async function toggleLedgerEntry(ledgerEntry: LedgerEntry) {
    const target = entries.find((entry) => entry.uid === ledgerEntry.uid);
    if (!target || target.locked) return;
    setWorldbookMessage("");
    try {
      await invoke<number>("upsert_worldbook_entry", {
        worldId: world,
        entry: { ...target, disabled: !target.disabled },
      });
      await refreshWorldbook();
      await refreshLedger();
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 條目表單按取消＝丟資料，先問過（自動存只走切換編輯對象那條路）
  async function confirmDiscardDraft() {
    if (!draftDirty) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: 1 }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
    });
  }

  // 換編輯對象＝把手上這條存起來就走（條目本來就是即時寫檔，多問一次只是擋路）。
  // 還沒存過的新條目例外：直接存會把半成品留在清單上，照舊問。
  async function openDraft(next: WorldbookDraft) {
    let autoSaved = false;
    if (draft && draftDirty) {
      if (draft.uid === null) {
        if (!(await confirmDiscardDraft())) return;
      } else {
        if (!(await persistDraft(draft))) return;
        autoSaved = true;
      }
    }
    setWorldbookMessage(autoSaved ? t("worldbookEntrySaved") : "");
    setDraft(next);
    setDraftOrigin(JSON.stringify(next));
  }

  async function closeDraft() {
    if (await confirmDiscardDraft()) setDraft(null);
  }

  function addEntry() {
    void openDraft({
      uid: null,
      title: "",
      keys: "",
      content: "",
      constant: false,
      enabled: true,
      order: 100,
      visibility: "gm",
      characters: [],
    });
  }

  function editEntry(entry: WorldbookEntry) {
    void openDraft({
      uid: entry.uid,
      title: entry.title,
      keys: entry.keys.join("、"),
      content: entry.content,
      constant: entry.constant,
      enabled: !entry.disabled,
      order: entry.order,
      visibility: entry.visibility.type,
      characters: entry.visibility.type === "characters" ? entry.visibility.characters : [],
    });
  }

  /** 把表單寫回世界書；失敗時把原因留在清單訊息列並回傳 false（表單不關） */
  async function persistDraft(source: WorldbookDraft) {
    const visibility: Visibility =
      source.visibility === "characters"
        ? {
            type: "characters",
            characters: source.characters.filter((id) =>
              characters.some((character) => character.id === id),
            ),
          }
        : { type: source.visibility };
    const entry: WorldbookEntry = {
      uid: source.uid ?? Number.MAX_SAFE_INTEGER,
      title: source.title.trim(),
      keys: source.keys
        .split(/[,、]/)
        .map((key) => key.trim())
        .filter(Boolean),
      content: source.content,
      constant: source.constant,
      order: source.order,
      disabled: !source.enabled,
      locked: false,
      visibility,
    };
    try {
      await invoke<number>("upsert_worldbook_entry", { worldId: world, entry });
      await refreshWorldbook();
      return true;
    } catch (reason) {
      setWorldbookMessage(String(reason));
      return false;
    }
  }

  async function saveEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft) return;
    setWorldbookMessage("");
    if (!(await persistDraft(draft))) return;
    setDraft(null);
    setWorldbookMessage(t("worldbookEntrySaved"));
  }

  async function deleteEntry(entry: WorldbookEntry) {
    setWorldbookMessage("");
    try {
      const accepted = await confirm(
        t("worldbookDeleteConfirm", { title: entry.title || String(entry.uid) }),
        { title: t("worldbookDeleteTitle"), kind: "warning" },
      );
      if (!accepted) return;
      await invoke("delete_worldbook_entry", { worldId: world, uid: entry.uid });
      await refreshWorldbook();
      if (draft?.uid === entry.uid) setDraft(null);
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function reorderEntries(ordered: WorldbookEntry[]) {
    setWorldbookMessage("");
    const previous = entries;
    setEntries(ordered);
    try {
      await invoke("reorder_worldbook_entries", {
        worldId: world,
        uids: ordered.map((entry) => entry.uid),
      });
    } catch (reason) {
      setEntries(previous);
      setWorldbookMessage(String(reason));
    }
  }

  // 去重上線前重複匯入過的桌，用這顆自己收拾：同內容只留排最前面那條
  async function dedupeWorldbook() {
    setWorldbookMessage("");
    try {
      const accepted = await confirm(t("worldbookDedupeConfirm"), {
        title: t("worldbookDedupe"),
        kind: "warning",
      });
      if (!accepted) return;
      // 去重只刪東西，別觸發匯入後的選 GM／改桌名
      const removed = await invoke<number>("dedupe_worldbook", { worldId: world });
      if (removed > 0) await refreshWorldbook();
      setWorldbookMessage(
        removed > 0 ? t("worldbookDedupeDone", { n: removed }) : t("worldbookDedupeNone"),
      );
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  async function exportWorldbook() {
    setWorldbookMessage("");
    try {
      const path = await saveDialog({
        defaultPath: "worldbook.json",
        filters: [{ name: t("worldbookJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("export_worldbook", { worldId: world, path });
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 匯出這桌先前套用過的重構產物（apply() 落檔），重玩同一張卡不必再燒 AI 額度重新展開。
  async function exportSavedRefactorOutcome() {
    setWorldbookMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName),
        filters: [{ name: t("refactorOutcomeJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("refactor_export_saved", { worldId: world, path });
      await revealItemInDir(path);
    } catch (reason) {
      setWorldbookMessage(
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
    if (refactorProgress) return;
    if (await invoke<boolean>("refactor_outcome_exists", { worldId: world })) {
      const rerun = await confirm(t("refactorRerunWarnBody"), {
        title: t("refactorBtn"),
        kind: "warning",
      });
      if (!rerun) return;
    }
    setWorldbookMessage("");
    const cards = await invoke<CardInterface[]>("card_interfaces", { worldId: world }).catch(
      () => [] as CardInterface[],
    );
    const tristate = detectRefactorTristate(cards);
    if (tristate === "unsupported") {
      setWorldbookMessage(t("refactorUnsupportedCard"));
      return;
    }
    if (tristate === "none") {
      await startRefactorRun("characters");
      return;
    }
    // supported：初判帶全卡只出兩行；取消走既有 refactor_abort 路。
    refactorCancelRef.current = false;
    setRefactorCancelled(false);
    setRefactorProgress({ text: t("refactorProbing"), cancelling: false, tail: "" });
    try {
      let probeTail = "";
      const channel = new Channel<string>();
      channel.onmessage = (delta: string) => {
        probeTail = (probeTail + delta).slice(-2000);
        setRefactorProgress((current) =>
          current && { ...current, tail: probeTail.split("\n").slice(-4).join("\n") },
        );
      };
      const probe = await invoke<RefactorRecommendOutcome>("refactor_recommend", {
        worldId: world,
        onDelta: channel,
      });
      setRefactorProgress(null);
      const recommend: RefactorMode = probe.recommend === "characters" ? "characters" : "interface";
      setRefactorModeAsk({
        recommend,
        evidence: probe.evidence,
        expanded: false,
        picked: recommend,
        ticket: probe.run_id ? { runId: probe.run_id, fingerprint: probe.fingerprint } : null,
      });
    } catch (reason) {
      setRefactorProgress(null);
      if (String(reason).includes("refactor-aborted")) return;
      // 初判失敗＝不偽造證據：no 判官句、直接展開兩選項、預設介面優先（2026-08-14 拍板）
      setRefactorModeAsk({ recommend: null, evidence: "", expanded: true, picked: "interface", ticket: null });
    }
  }

  // 玩家選定玩法後的重構主體（none 卡直接以 characters 進來、無 resume 憑證）。
  async function startRefactorRun(mode: RefactorMode, ticket: RefactorRunTicket | null = null) {
    refactorCancelRef.current = false;
    setRefactorCancelled(false);
    setRefactorProgress({ text: t("refactorSurveying"), cancelling: false, tail: "" });
    try {
      // 共用 tail：所有呼叫（survey＋展開）的 Channel onDelta 都 append 進同一個 buffer，
      // 任一路增量＝活著訊號，不因並行而互相蓋掉彼此的畫面。
      let tailBuffer = "";
      const appendTail = (delta: string) => {
        tailBuffer = (tailBuffer + delta).slice(-2000);
        setRefactorProgress((current) =>
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
      const { local: localPersons, queue } = buildRefactorPersonPlan(survey, entries, local.clean_person_names);

      const absorbUids = survey.verdicts.filter((verdict) => verdict.action === "absorb").map((verdict) => verdict.uid);
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
          ? [...statusbarByUid.entries()].map(([uid, spans]): RefactorTask => ({ kind: "statusbar", uid, spans }))
          : []),
        ...(buildInterfaces ? survey.interface_uids.map((uid): RefactorTask => ({ kind: "interface", uid })) : []),
      ];
      const totalSteps = pool.length;

      const characters: RefactorCharacter[] = [...local.characters, ...localPersons];
      const refactorEntries: RefactorNewEntry[] = [...local.entries];
      // 淘汰／未收編／稽核有東西＝不是「無事可做」：純介面卡選 characters 時產物只剩 rule 5
      // 淘汰清單，也要開結果視窗——玩家看得到可放回，套用後 mode 才落地、介面 fallback 才停。
      const localNotes = local.dropped.length + local.unabsorbed.length + local.audit.length;
      if (totalSteps === 0 && characters.length === 0 && refactorEntries.length === 0 && localNotes === 0) {
        setRefactorProgress(null);
        setWorldbookMessage(t("refactorNothingToDo"));
        return;
      }

      const interfaces: RefactorInterface[] = [];
      // reason 帶原始錯誤文字（去重顯示在結果視窗）：玩家看得到「模型呼叫失敗」這類可修正原因。
      const failedTitles: { name: string; reason: string }[] = [];
      const knownFields = survey.fields; // 命名唯一權威，全呼叫共用同一份、不累積。
      let done = 0;
      const bumpDone = () => {
        done++;
        setRefactorProgress((current) => current && { ...current, text: t("refactorParallelStep", { done, total: totalSteps }) });
      };

      setRefactorProgress((current) => current && { ...current, text: t("refactorParallelStep", { done, total: totalSteps }) });

      const run = async (task: RefactorTask): Promise<void> => {
        const name = task.kind === "person" ? task.item.name : task.kind === "group" ? task.group.title : sourceEntryTitle(entries, task.uid);
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
              () => refactorCancelRef.current,
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
              () => refactorCancelRef.current,
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
              () => refactorCancelRef.current,
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
              () => refactorCancelRef.current,
            );
            if (result.interface) interfaces.push(result.interface);
            else failedTitles.push({ name, reason: "" });
          } else {
            const result = await withRateLimitRetry(
              () =>
                invoke<RefactorExpandOutcome>("refactor_expand", {
                  worldId: world,
                  entryUid: task.uid,
                  kind: survey.playable_interface_uids.includes(task.uid) ? "interface_shell" : "interface",
                  knownFields,
                  onDelta: makeOnDelta(),
                }),
              () => refactorCancelRef.current,
            );
            if (result.interface) interfaces.push(result.interface);
            else failedTitles.push({ name, reason: "" });
          }
        } catch (reason) {
          if (!String(reason).includes("refactor-aborted")) failedTitles.push({ name, reason: String(reason).slice(0, 200) });
        } finally {
          bumpDone();
        }
      };

      // chain 恆空：survey 同一 run 已建快取，warmed=true 跳過首發獨跑，pool 直接全並行開跑。
      await runRefactorCalls({
        chain: [],
        pool,
        limit: REFACTOR_PARALLEL_LIMIT,
        isCancelled: () => refactorCancelRef.current,
        run,
        warmed: true,
      });

      setRefactorProgress(null);
      if (characters.length > 0 || interfaces.length > 0 || refactorEntries.length > 0 || localNotes > 0) {
        const outcome = assembleRefactorOutcome({
          characters,
          interfaces,
          entries: refactorEntries,
          dropped: local.dropped,
          unabsorbed: local.unabsorbed,
          audit: local.audit,
          mode,
        });
        setRefactorOutcome(outcome);
        setRefactorSelection(defaultRefactorSelection(outcome));
        setRefactorOrigin("ai");
        setRefactorDetail(false);
        setRefactorCancelled(refactorCancelRef.current);
      }
      setRefactorFailures(failedTitles);
    } catch (reason) {
      setRefactorProgress(null);
      if (String(reason).includes("refactor-aborted")) return;
      // MODE 回聲核對不過：判官跑錯玩法整份拒收（後端固定字串），換成玩家看得懂的一句
      if (String(reason).includes("refactor-mode-mismatch")) {
        setWorldbookMessage(t("refactorModeMismatch"));
        return;
      }
      setWorldbookMessage(String(reason));
    }
  }

  // 取消：擋「還沒發的下一條」＋後端 abort 在途呼叫（refactor_abort，包 2 交付）——已經在燒
  // 的那幾條立刻中止，中止錯誤走 sentinel "refactor-aborted" 靜默略過，不列入失敗。
  function cancelAiRefactor() {
    refactorCancelRef.current = true;
    setRefactorProgress((current) => current && { ...current, cancelling: true });
    void invoke("refactor_abort", { worldId: world });
  }

  // AI 卡重構：零額度測試用入口——直接餵一份產物 JSON，跳過真 AI 呼叫，驗證人審／套用路徑用。
  async function pickRefactorOutcome(file: File) {
    setWorldbookMessage("");
    try {
      const outcome = parseRefactorOutcome(await file.text());
      setRefactorOutcome(outcome);
      setRefactorSelection(defaultRefactorSelection(outcome));
      setRefactorOrigin("import");
      setRefactorDetail(false);
    } catch (reason) {
      const invalid = reason instanceof Error && reason.message === REFACTOR_IMPORT_INVALID;
      setWorldbookMessage(invalid ? t("refactorImportInvalid") : String(reason));
    }
  }

  function closeRefactor() {
    setRefactorOutcome(null);
    setRefactorSelection(null);
    setRefactorOrigin(null);
    setRefactorDetail(false);
    setRefactorFailures([]);
    setRefactorCancelled(false);
  }

  // 已淘汰清單的「放回」：零後端行為，走既有 entries 勾選路徑——套用時跟其他世界書條目一視同仁。
  function restoreDroppedItem(index: number) {
    if (!refactorOutcome || !refactorSelection) return;
    const result = restoreDropped(refactorOutcome, refactorSelection, index);
    setRefactorOutcome(result.outcome);
    setRefactorSelection(result.selection);
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

  async function applyRefactor(selection: RefactorSelection) {
    if (!refactorOutcome || refactorBusy) return;
    setWorldbookMessage("");
    setRefactorBusy(true);
    try {
      const summary = await invoke<RefactorApplySummary>("refactor_apply", {
        worldId: world,
        outcome: refactorOutcome,
        selection,
        recordReceipt: refactorOrigin !== "ai",
      });
      closeRefactor();
      await refreshWorldbook();
      await refreshLedger();
      await refreshCast();
      await onRefactorApplied();
      await showMessage(refactorApplyMessage(summary), { title: t("refactorBtn") });
    } catch (reason) {
      setWorldbookMessage(String(reason));
    } finally {
      setRefactorBusy(false);
    }
  }

  // 匯出結果卡上這份還沒套用（或剛套用完）的產物，供之後用「匯入重構卡」讀回重玩。
  async function exportRefactorOutcome() {
    if (!refactorOutcome || refactorBusy) return;
    setWorldbookMessage("");
    try {
      const path = await saveDialog({
        defaultPath: refactorCardFileName(worldName),
        filters: [{ name: t("refactorOutcomeJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("refactor_export_outcome", { outcome: refactorOutcome, path });
      await revealItemInDir(path);
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 手動「轉成角色卡」一律轉一般卡——玩家卡另有從頭建立的入口，AI 卡重構的勾選畫面也能指定
  // 玩家卡（要點 4），這顆按鈕不再問「要不要轉成玩家卡」。
  async function convertEntryToCharacter() {
    if (!draft || draft.uid === null) return;
    setWorldbookMessage("");
    try {
      const meta = await invoke<CharacterMeta>("worldbook_entry_to_character", {
        worldId: world,
        uid: draft.uid,
        color: convertColor,
        asPlayer: false,
      });
      setDraft(null);
      await refreshWorldbook();
      setWorldbookMessage(t("convertEntryDone", { name: meta.name }));
      await onEntryConverted();
    } catch (reason) {
      setWorldbookMessage(String(reason));
    }
  }

  // 條目表單就地展開：編輯取代原本那一列、新增排在清單底部（2026-07-30 使用者回饋——
  // 表單固定在頂端時，點下方條目的編輯完全看不出反應）。按鈕照全 app 慣例置頂。
  const entryForm = draft && (
    <form ref={draftFormRef} className="settings-form worldbook-form" onSubmit={saveEntry}>
      <div className="row">
        <button type="submit">{t("worldbookSaveEntry")}</button>
        <button type="button" onClick={() => void closeDraft()}>
          {t("worldbookCancel")}
        </button>
        {draft.uid !== null && (
          <button type="button" onClick={() => void convertEntryToCharacter()}>
            {t("convertEntryToCard")}
          </button>
        )}
      </div>
      <label>
        {t("worldbookEntryTitle")}
        <input
          value={draft.title}
          onChange={(event) => setDraft({ ...draft, title: event.currentTarget.value })}
        />
      </label>
      <label>
        {t("worldbookKeys")}
        <input
          value={draft.keys}
          placeholder={t("worldbookKeysHint")}
          onChange={(event) => setDraft({ ...draft, keys: event.currentTarget.value })}
        />
      </label>
      <label>
        {t("worldbookContent")}
        <textarea
          rows={7}
          value={draft.content}
          onChange={(event) => setDraft({ ...draft, content: event.currentTarget.value })}
        />
      </label>
      <label className="inline">
        <input
          type="checkbox"
          checked={draft.constant}
          onChange={(event) => setDraft({ ...draft, constant: event.currentTarget.checked })}
        />
        {t("worldbookConstantLabel")}
      </label>
      <label className="inline">
        <input
          type="checkbox"
          checked={draft.enabled}
          onChange={(event) => setDraft({ ...draft, enabled: event.currentTarget.checked })}
        />
        {t("worldbookEnabled")}
      </label>
      <fieldset className="worldbook-visibility">
        <legend>{t("worldbookVisibility")}</legend>
        {(["gm", "public", "characters"] as const).map((visibility) => (
          <label className="inline" key={visibility}>
            <input
              type="radio"
              name="worldbook-visibility"
              value={visibility}
              checked={draft.visibility === visibility}
              onChange={() => {
                setDraft({ ...draft, visibility });
                // 點「指定角色」當下重抓在場角色：畫面開著時可能剛從隱藏區還原角色
                if (visibility === "characters") {
                  void invoke<CharacterMeta[]>("list_characters", { worldId: world }).then((cast) =>
                    setCharacters(cast.filter((character) => !character.archived)),
                  );
                }
              }}
            />
            {visibility === "gm"
              ? t("worldbookVisibilityGm")
              : visibility === "public"
                ? t("worldbookVisibilityPublic")
                : t("worldbookVisibilityCharacters")}
          </label>
        ))}
      </fieldset>
      {draft.visibility === "characters" && (
        <fieldset className="worldbook-characters">
          <legend>{t("worldbookChooseCharacters")}</legend>
          {characters.length === 0 ? (
            <span>{t("worldbookNoCharacters")}</span>
          ) : (
            characters.map((character) => (
              <label className="inline" key={character.id}>
                <input
                  type="checkbox"
                  checked={draft.characters.includes(character.id)}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      characters: event.currentTarget.checked
                        ? [...draft.characters, character.id]
                        : draft.characters.filter((id) => id !== character.id),
                    })
                  }
                />
                {character.name}
              </label>
            ))
          )}
        </fieldset>
      )}
    </form>
  );

  // 結果卡摘要行只列有產物的區：「拆出 N 個角色」「介面」「收編 N 條規則」以「・」串接
  const refactorCounts = refactorOutcome ? refactorSummaryCounts(refactorOutcome) : null;
  const refactorSummaryParts = refactorCounts
    ? [
        refactorCounts.characters > 0 && t("refactorSummaryCharacters", { n: refactorCounts.characters }),
        refactorCounts.hasInterface && t("refactorSummaryInterface"),
        refactorCounts.entries > 0 && t("refactorSummaryEntries", { n: refactorCounts.entries }),
        refactorCounts.mechanisms > 0 && t("refactorSummaryMechanisms", { n: refactorCounts.mechanisms }),
      ].filter((part): part is string => Boolean(part))
    : [];

  return (
    <>
      <form onSubmit={saveWorldSettings} className="settings-form">
        {/* 按鈕列放文字框上方：長文編輯時儲存／返回固定在最顯眼處（2026-07-24 使用者回饋） */}
        <div className="row">
          <button type="submit">{t("saveWorld")}</button>
          <button type="button" onClick={() => void handleBack()}>
            {t("backToNow")}
          </button>
          {message && <span>{message}</span>}
          {unsavedCount > 0 && (
            <span className="unsaved-hint" role="status">
              {t("unsavedChanges", { n: unsavedCount })}
            </span>
          )}
        </div>
        <textarea
          rows={6}
          aria-label={t("worldAria")}
          value={text}
          onChange={(e) => setText(e.currentTarget.value)}
        />
      </form>

      <section className="worldbook-section" aria-labelledby="worldbook-title">
        <h3 id="worldbook-title">{t("worldbookTitle")}</h3>
        <div className="worldbook-actions">
          <button type="button" onClick={addEntry}>
            {t("worldbookAddEntry")}
          </button>
          <button type="button" onClick={() => void dedupeWorldbook()}>
            {t("worldbookDedupe")}
          </button>
          <button type="button" onClick={() => void exportWorldbook()}>
            {t("worldbookExport")}
          </button>
          <button
            type="button"
            className="ai-gen-btn"
            title={t("refactorBtnHint")}
            disabled={refactorProgress !== null}
            onClick={() => void runAiRefactor()}
          >
            ✨ {t("refactorBtn")}
          </button>
          <button
            type="button"
            title={t("refactorImportBtnHint")}
            disabled={refactorProgress !== null}
            onClick={() => refactorInputRef.current?.click()}
          >
            {t("refactorImportBtn")}
          </button>
          <button
            type="button"
            disabled={refactorProgress !== null}
            onClick={() => void exportSavedRefactorOutcome()}
          >
            {t("refactorExportSavedBtn")}
          </button>
          <input
            ref={refactorInputRef}
            type="file"
            accept=".json,application/json"
            hidden
            onChange={(e) => {
              const file = e.currentTarget.files?.[0];
              e.currentTarget.value = "";
              if (file) void pickRefactorOutcome(file);
            }}
          />
        </div>
        {/* 操作回饋緊貼按鈕列：重構擋下訊息之類的結果放列表底部的話，條目多的桌要捲到底
            才看得到，點了像沒反應。 */}
        {worldbookMessage && <p role="status">{worldbookMessage}</p>}

        {/* 標準流程零必看：只有真的有東西被接管／跳過，或有記帳次數時才出現這塊。 */}
        {(ledger.entries.length > 0 ||
          ledger.rejected > 0 ||
          ledger.clamped > 0 ||
          ledger.errors > 0 ||
          ledger.jumps > 0) && (
          <details className="mechanism-ledger">
            <summary>{t("ledgerTitle")}</summary>
            {ledger.entries.length > 0 && (
              <div className="mechanism-ledger-list">
                {ledger.entries.map((entry) => (
                  <div className="mechanism-ledger-row" key={entry.uid}>
                    <div className="mechanism-ledger-summary">
                      <strong>{entry.title}</strong>
                      <span className="worldbook-badge">
                        {entry.kind === "absorbed" ? t("ledgerAbsorbed") : t("ledgerSkipped")}
                      </span>
                      <span className="mechanism-ledger-detail">{entry.detail}</span>
                    </div>
                    {!entries.find((worldbookEntry) => worldbookEntry.uid === entry.uid)?.locked && (
                    <label className="mechanism-ledger-toggle">
                      <input
                        type="checkbox"
                        checked={entry.sent}
                        onChange={() => void toggleLedgerEntry(entry)}
                      />
                      {t("ledgerSendRaw")}
                    </label>
                    )}
                  </div>
                ))}
              </div>
            )}
            {(ledger.rejected > 0 || ledger.clamped > 0 || ledger.errors > 0 || ledger.jumps > 0) && (
              <p className="mechanism-ledger-stats">
                {[
                  ledger.rejected > 0 && t("ledgerStatsRejected", { n: ledger.rejected }),
                  ledger.clamped > 0 && t("ledgerStatsClamped", { n: ledger.clamped }),
                  ledger.errors > 0 && t("ledgerStatsErrors", { n: ledger.errors }),
                  ledger.jumps > 0 && t("ledgerStatsJumps", { n: ledger.jumps }),
                ]
                  .filter(Boolean)
                  .join("　")}
              </p>
            )}
          </details>
        )}

        {entries.length === 0 ? (
          <p className="worldbook-empty">{t("worldbookEmpty")}</p>
        ) : (
          <div className="worldbook-list">
            {entryDrag.order.map((entry) =>
              draft && draft.uid === entry.uid ? (
                <Fragment key={entry.uid}>{entryForm}</Fragment>
              ) : (
              <div
                className={`worldbook-row${entry.disabled ? " worldbook-row-disabled" : ""}${
                  entryDrag.draggingKey === String(entry.uid) ? " row-dragging" : ""
                }`}
                key={entry.uid}
                title={t("dragToReorder")}
                {...entryDrag.rowProps(entry)}
              >
                <div className="worldbook-summary">
                  {(() => {
                    const head = (
                      <>
                        <strong>{entry.title || entry.uid}</strong>
                        <span>{entry.keys.join("、") || t("worldbookNoKeys")}</span>
                        <div className="worldbook-badges">
                          {entry.constant && (
                            <span className="worldbook-badge">{t("worldbookConstant")}</span>
                          )}
                          {/* 可見範圍＝資訊邊界：全 app 統一的虛線琥珀機密記號 */}
                          <span className="worldbook-badge worldbook-badge-visibility">
                            {entry.visibility.type === "gm"
                              ? t("worldbookVisibilityGm")
                              : entry.visibility.type === "public"
                                ? t("worldbookVisibilityPublic")
                                : t("worldbookCharacterCount", {
                                    n: entry.visibility.characters.length,
                                  })}
                          </span>
                          {entry.disabled && (
                            <span className="worldbook-badge">{t("worldbookDisabled")}</span>
                          )}
                          {entry.locked && <span className="worldbook-badge">🔒 {t("worldbookLocked")}</span>}
                        </div>
                      </>
                    );
                    // 鎖定條目不能編輯，但說明文的家就在世界書：標題列可展開唯讀全文。
                    return entry.locked ? (
                      <details className="worldbook-locked-view">
                        <summary>{head}</summary>
                        <div className="worldbook-locked-content">{entry.content}</div>
                      </details>
                    ) : (
                      head
                    );
                  })()}
                </div>
                {!entry.locked && <div className="worldbook-row-actions">
                  <button type="button" onClick={() => editEntry(entry)}>
                    {t("editBtn")}
                  </button>
                  <button type="button" onClick={() => void deleteEntry(entry)}>
                    {t("worldbookDelete")}
                  </button>
                </div>
                }
              </div>
              ),
            )}
          </div>
        )}
        {draft && draft.uid === null && entryForm}
      </section>

      {refactorProgress && (
        <div className="modal-overlay">
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("refactorBtn")}>
            <h2>{t("refactorBtn")}</h2>
            <p role="status">{refactorProgress.text}</p>
            {refactorProgress.tail && <pre className="refactor-stream-tail">{refactorProgress.tail}</pre>}
            <div className="ai-gen-footer">
              <button type="button" disabled={refactorProgress.cancelling} onClick={cancelAiRefactor}>
                {t("refactorCancel")}
              </button>
            </div>
          </div>
        </div>
      )}

      {refactorModeAsk && (
        // 二選一（refactor-mode-split，2026-08-14 拍板文案）：一鍵照建議＋展開自己選兩層都有。
        // 取消＝整個不跑，反悔路是重按重構鈕重跑（原卡 PNG 留檔）。
        <div className="modal-overlay">
          <div className="modal" role="dialog" aria-modal="true" aria-label={t("refactorModeTitle")}>
            <h2>{t("refactorModeTitle")}</h2>
            {refactorModeAsk.recommend !== null && (
              <p>
                {refactorModeAsk.recommend === "interface"
                  ? t("refactorModeSuggestInterface", { evidence: refactorModeAsk.evidence })
                  : t("refactorModeSuggestCharacters", { evidence: refactorModeAsk.evidence })}
              </p>
            )}
            {refactorModeAsk.expanded && (
              <div className="refactor-mode-options">
                {(["interface", "characters"] as const).map((option) => (
                  <label key={option} className="refactor-mode-option">
                    <input
                      type="radio"
                      name="refactor-mode"
                      checked={refactorModeAsk.picked === option}
                      onChange={() => setRefactorModeAsk({ ...refactorModeAsk, picked: option })}
                    />
                    <span>
                      <strong>{option === "interface" ? t("refactorModeOptInterface") : t("refactorModeOptCharacters")}</strong>
                      <br />
                      {option === "interface" ? t("refactorModeOptInterfaceDesc") : t("refactorModeOptCharactersDesc")}
                    </span>
                  </label>
                ))}
              </div>
            )}
            <div className="ai-gen-footer">
              <button type="button" onClick={() => setRefactorModeAsk(null)}>
                {t("refactorCancel")}
              </button>
              {!refactorModeAsk.expanded && (
                <button
                  type="button"
                  onClick={() => setRefactorModeAsk({ ...refactorModeAsk, expanded: true })}
                >
                  {t("refactorModeChoose")}
                </button>
              )}
              <button
                type="button"
                className="ai-gen-submit"
                onClick={() => {
                  const { picked, ticket } = refactorModeAsk;
                  setRefactorModeAsk(null);
                  void startRefactorRun(picked, ticket);
                }}
              >
                {refactorModeAsk.expanded ? t("refactorModeGo") : t("refactorModeGoRecommended")}
              </button>
            </div>
          </div>
        </div>
      )}

      {refactorOutcome && refactorSelection && (
        // 點視窗外不關閉（2026-08-12 拍板）：誤觸一下整份重構結果就丟了，關閉只走「不要」鍵
        <div className="modal-overlay">
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label={t("refactorResultTitle")}
          >
            <h2>
              {refactorCancelled
                ? t("refactorResultCancelledTitle")
                : refactorFailures.length > 0
                  ? t("refactorResultPartialTitle")
                  : t("refactorResultTitle")}
            </h2>
            {refactorCancelled && (
              <p className="usage-bad" role="alert">
                {t("refactorCancelledNotice")}
              </p>
            )}
            {refactorFailures.length > 0 && (
              <p className="usage-bad" role="alert">
                {t("refactorPartialFailed", { n: refactorFailures.length, names: refactorFailures.map((f) => f.name).join("、") })}
                {[...new Set(refactorFailures.map((f) => f.reason).filter(Boolean))].map((reason) => (
                  <span key={reason} className="refactor-fail-reason">
                    {t("refactorFailReason", { reason })}
                  </span>
                ))}
              </p>
            )}
            {!refactorDetail ? (
              <>
                {refactorSummaryParts.length > 0 && <p>{refactorSummaryParts.join("・")}</p>}
                <div className="ai-gen-footer">
                  {/* 取消造成的半成品：主按鈕換成「不要」，套用降級成次要鈕（2026-08-14 拍板）。 */}
                  <button
                    type="button"
                    className={refactorCancelled ? "ai-gen-submit" : undefined}
                    disabled={refactorBusy}
                    onClick={closeRefactor}
                  >
                    {t("refactorDismiss")}
                  </button>
                  <button type="button" disabled={refactorBusy} onClick={() => void exportRefactorOutcome()}>
                    {t("refactorExportBtn")}
                  </button>
                  <button type="button" disabled={refactorBusy} onClick={() => setRefactorDetail(true)}>
                    {t("refactorExpand")}
                  </button>
                  <button
                    type="button"
                    className={refactorCancelled ? undefined : "ai-gen-submit"}
                    disabled={refactorBusy}
                    onClick={() => void applyRefactor(defaultRefactorSelection(refactorOutcome))}
                  >
                    {t("refactorApplyAll")}
                  </button>
                </div>
              </>
            ) : (
              <>
                {refactorOutcome.characters.length > 0 && (
                  <section>
                    <h3>{t("refactorSectionCharacters")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.characters.map((character, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <details>
                            <summary>
                              <label className="inline" onClick={(event) => event.stopPropagation()}>
                            <input
                              type="checkbox"
                              checked={refactorSelection.character_indices.includes(index)}
                              onClick={(event) => event.stopPropagation()}
                              onChange={(event) => {
                                const checked = event.currentTarget.checked;
                                setRefactorSelection(
                                  (selection) =>
                                    selection &&
                                    (checked
                                      ? { ...selection, character_indices: toggleIndex(selection.character_indices, index, true) }
                                      : unselectCharacter(selection, index)),
                                );
                              }}
                            />
                            {character.emoji} {character.name}
                          </label>
                            </summary>
                            <span className="refactor-source">{t("refactorSourceLabel", { titles: sourceEntryTitles(entries, character.source_uids) })}</span>
                            <p>{t("refactorCharPublic")}</p>
                            <div style={{ whiteSpace: "pre-wrap" }}>{character.public_md}</div>
                            <p>{t("refactorCharPrivate")}</p>
                            <div style={{ whiteSpace: "pre-wrap" }}>{character.private_md}</div>
                          </details>
                          {/* 玩家卡只問 AI 認定是 {{user}} 的那一位：多數卡都預設好玩家是誰，
                              讓任意角色都能被選成玩家卡不符合卡的設計。 */}
                          {character.suspected_player && (
                            <label className="mechanism-ledger-toggle">
                              <input
                                type="checkbox"
                                checked={refactorSelection.player_index === index}
                                onChange={(event) =>
                                  setRefactorSelection(
                                    (selection) =>
                                      selection && setPlayerIndex(selection, event.currentTarget.checked ? index : null),
                                  )
                                }
                              />
                              {t("refactorPlayerCheckLabel")}
                            </label>
                          )}
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                {refactorOutcome.entries.length > 0 && (
                  <section>
                    <h3>{t("refactorSectionEntries")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.entries.map((entry, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <details>
                            <summary>
                              <label className="inline" onClick={(event) => event.stopPropagation()}>
                                <input
                                  type="checkbox"
                                  checked={refactorSelection.entry_indices.includes(index)}
                                  onClick={(event) => event.stopPropagation()}
                                  onChange={(event) => {
                                    const checked = event.currentTarget.checked;
                                    setRefactorSelection((selection) => selection && {
                                      ...selection,
                                      entry_indices: toggleIndex(selection.entry_indices, index, checked),
                                    });
                                  }}
                                />
                                {entry.title}
                              </label>
                              <span className="worldbook-badge">
                                {entry.kind === "setting" ? t("refactorEntryKindSetting") : t("refactorEntryKindMechanism")}
                              </span>
                              {entry.kind === "mechanism" && (Object.keys(entry.rules ?? {}).length > 0 || (entry.triggers?.length ?? 0) > 0) && (
                                <span className="worldbook-badge">🔒 {t("worldbookLocked")}</span>
                              )}
                            </summary>
                            <span className="refactor-source">{t("refactorSourceLabel", { titles: sourceEntryTitles(entries, entry.source_uids) })}</span>
                            <div style={{ whiteSpace: "pre-wrap" }}>{entry.content}</div>
                          </details>
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                {refactorOutcome.interface && (
                  <section>
                    <h3>{t("refactorSectionInterface")}</h3>
                    <div className="mechanism-ledger-list">
                      <details>
                        <summary>
                        <label className="inline" onClick={(event) => event.stopPropagation()}>
                        <input
                          type="checkbox"
                          checked={refactorSelection.apply_interface}
                          onClick={(event) => event.stopPropagation()}
                          onChange={(event) => {
                            const checked = event.currentTarget.checked;
                            setRefactorSelection((selection) => selection && { ...selection, apply_interface: checked });
                          }}
                        />
                        {t("refactorSummaryInterface")}
                      </label>
                        </summary>
                        <span className="refactor-source">
                          {t("refactorSourceLabel", { titles: sourceEntryTitles(entries, refactorOutcome.interface.source_uids) })}
                        </span>
                        <div>{t("refactorInterfaceFields", { names: typeof refactorOutcome.interface.state_fields === "object" && refactorOutcome.interface.state_fields !== null && !Array.isArray(refactorOutcome.interface.state_fields) ? Object.keys(refactorOutcome.interface.state_fields).join("、") : "" })}</div>
                      </details>
                    </div>
                  </section>
                )}
                {refactorOutcome.mechanisms.length > 0 && (
                  <section>
                    <h3>{t("refactorSectionMechanisms")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.mechanisms.map((mechanism, index) => (
                        <label className="inline" key={index}>
                          <input
                            type="checkbox"
                            checked={refactorSelection.mechanism_indices.includes(index)}
                            onChange={(event) => {
                              const checked = event.currentTarget.checked;
                              setRefactorSelection(
                                (selection) =>
                                  selection && {
                                    ...selection,
                                    mechanism_indices: toggleIndex(selection.mechanism_indices, index, checked),
                                  },
                              );
                            }}
                          />
                          {sourceEntryTitle(entries, mechanism.source_uid)}
                        </label>
                      ))}
                    </div>
                  </section>
                )}
                {/* 已淘汰：判官整條／半條丟棄的內容快照，預設收起——玩家想確認才展開，救回來就是
                    普通世界書條目，走下面既有的套用路徑，沒有新的後端行為。 */}
                {refactorOutcome.dropped.length > 0 && (
                  <details className="mechanism-ledger">
                    <summary>{t("refactorDroppedSection", { n: refactorOutcome.dropped.length })}</summary>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.dropped.map((item, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <details>
                            <summary>
                              {item.title}{" "}
                              <span className="worldbook-badge">
                                {t(REFACTOR_DROPPED_RULE_KEYS[item.rule] ?? "refactorDroppedRule1")}
                              </span>
                            </summary>
                            <div style={{ whiteSpace: "pre-wrap" }}>{item.content}</div>
                          </details>
                          <button type="button" onClick={() => restoreDroppedItem(index)}>
                            {t("refactorDroppedRestore")}
                          </button>
                        </div>
                      ))}
                    </div>
                  </details>
                )}
                {/* 未接管機制：純資訊，原文已經照搬進 GM 規則條目——不會遺失，只是還沒有系統畫面。 */}
                {refactorOutcome.unabsorbed.length > 0 && (
                  <section>
                    <h3>{t("refactorUnabsorbedSection", { n: refactorOutcome.unabsorbed.length })}</h3>
                    <p className="mechanism-ledger-detail">{t("refactorUnabsorbedHint")}</p>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.unabsorbed.map((item, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <div className="mechanism-ledger-summary">
                            <strong>{item.title}</strong>
                            <span className="mechanism-ledger-detail">{item.note}</span>
                            <span className="refactor-source">{item.span || item.uid}</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                {/* 稽核：機械檢查抓到的紅字，純資訊不影響套用——detail 是後端已經寫好的繁中一句。 */}
                {refactorOutcome.audit.length > 0 && (
                  <section>
                    <h3>{t("refactorAuditSection")}</h3>
                    <div className="mechanism-ledger-list">
                      {refactorOutcome.audit.map((item, index) => (
                        <div className="mechanism-ledger-row" key={index}>
                          <div className="mechanism-ledger-summary">
                            <span className="worldbook-badge">
                              {t(REFACTOR_AUDIT_KIND_KEYS[item.kind] ?? "refactorAuditKindCoverage")}
                            </span>
                            <span className="refactor-source">{item.span || item.uid}</span>
                            <span className={item.kind === "excused" ? "mechanism-ledger-detail" : "usage-bad"}>
                              {item.detail}
                            </span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                )}
                <div className="ai-gen-footer">
                  <button type="button" disabled={refactorBusy} onClick={() => setRefactorDetail(false)}>
                    {t("settingsBack")}
                  </button>
                  <button
                    type="button"
                    className="ai-gen-submit"
                    disabled={refactorBusy}
                    onClick={() => void applyRefactor(refactorSelection)}
                  >
                    {t("refactorApplyBtn")}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </>
  );
}
