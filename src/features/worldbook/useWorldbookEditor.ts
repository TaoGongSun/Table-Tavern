import { type FormEvent, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type { WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../characters/card-model";
import { t } from "../../i18n";
import { backendCode } from "../../shared/ui/backend-text";
import {
  draftFromEntry,
  parseKeys,
  draftVisibility,
  EMPTY_LEDGER,
  mergePendingRebase,
  pruneDraftCharacters,
  rebaseDraft,
  type DraftState,
  type Ledger,
  type LedgerEntry,
  type PendingRebase,
  type EntryChanges,
  type WorldbookDraft,
} from "./worldbook-model";

const isDirty = (state: DraftState) =>
  state.draft !== null && JSON.stringify(state.draft) !== state.origin;
const CLOSED = { draft: null, origin: "" } as const;

interface UseWorldbookEditorOptions {
  world: string;
  convertColor: string;
  onEntryConverted: () => Promise<void>;
}

export function useWorldbookEditor({
  world,
  convertColor,
  onEntryConverted,
}: UseWorldbookEditorOptions) {
  const [entries, setEntriesState] = useState<WorldbookEntry[]>([]);
  // 最新條目清單：await 之後的寫入（帳本開關）從這裡取，不用當次 render 的舊清單
  const entriesRef = useRef<WorldbookEntry[]>([]);
  function setEntries(next: WorldbookEntry[]) {
    entriesRef.current = next;
    setEntriesState(next);
  }
  const [ledger, setLedger] = useState<Ledger>(EMPTY_LEDGER);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const [message, setMessage] = useState("");
  // 開著的條目草稿＋開啟當下的原樣快照（判斷「有沒有改過」）＋開啟序號。ref 是唯一真相、每次改都同步
  // 鏡像進 state：修剪、定基與玩家打字都在同一份上算，await 之後的入口讀 ref 拿得到最新值，不吃舊閉包
  const draftRef = useRef<DraftState>({ draft: null, origin: "", seq: 0 });
  const [draftState, setDraftState] = useState<DraftState>(draftRef.current);
  function updateDraft(change: (current: DraftState) => DraftState) {
    const next = change(draftRef.current);
    if (next === draftRef.current) return;
    draftRef.current = next;
    setDraftState(next);
  }
  const { draft, origin: draftOrigin, seq: draftSeq } = draftState;
  // 撤銷匯入或匯入合併報來、還沒套到草稿上的改動；之後任一次成功且最新的條目重讀套用後才清
  const pendingRebase = useRef<PendingRebase | null>(null);

  // 三種重讀各自編號：換桌或後發的重讀已送出時，晚到的舊回應丟掉
  const castRead = useRef(0);
  const entriesRead = useRef(0);
  const ledgerRead = useRef(0);

  // 舊請求晚到的成功與失敗都丟掉（失敗不往外拋），不蓋掉新的名單或訊息
  async function readCast() {
    const ticket = ++castRead.current;
    let cast: CharacterMeta[];
    try {
      cast = await invoke<CharacterMeta[]>("list_characters", { worldId: world });
    } catch (reason) {
      if (ticket === castRead.current) throw reason;
      return;
    }
    if (ticket === castRead.current) {
      setCharacters(cast.filter((character) => !character.archived));
    }
  }

  async function refreshCast() {
    try {
      await readCast();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  // 最新一次條目重讀：等定基的入口跟著它等，不另發（另發會被之後的重讀一再頂掉）
  const readInFlight = useRef<Promise<void> | null>(null);

  function refreshWorldbook() {
    const read = readWorldbook();
    readInFlight.current = read;
    const clear = () => {
      if (readInFlight.current === read) readInFlight.current = null;
    };
    read.then(clear, clear);
    return read;
  }

  async function readWorldbook() {
    const ticket = ++entriesRead.current;
    let next: WorldbookEntry[];
    try {
      next = await invoke<WorldbookEntry[]>("read_worldbook", { worldId: world });
    } catch (reason) {
      if (ticket === entriesRead.current) throw reason;
      return;
    }
    if (ticket !== entriesRead.current) return;
    setEntries(next);
    const pending = pendingRebase.current;
    if (pending) {
      pendingRebase.current = null;
      updateDraft((current) => rebaseDraft(current, pending, next, t("listSeparator")));
    }
  }

  async function refreshLedger() {
    const ticket = ++ledgerRead.current;
    let next = EMPTY_LEDGER;
    try {
      next = await invoke<Ledger>("mechanism_ledger", { worldId: world });
    } catch {
      // 帳本掛掉不該擋住世界書編輯：失敗就當空，不彈錯誤。
    }
    if (ticket === ledgerRead.current) setLedger(next);
  }

  useEffect(() => {
    setMessage("");
    setEntries([]);
    setLedger(EMPTY_LEDGER);
    setCharacters([]);
    pendingRebase.current = null;
    updateDraft((current) => ({ ...current, ...CLOSED }));
    refreshWorldbook().catch((reason) => setMessage(String(reason)));
    void refreshLedger();
    void refreshCast();
  }, [world]);

  const draftDirty = isDirty(draftState);
  // 既有條目改到一半離開時會自動存，不算未儲存；只有還沒存過的新條目要提醒
  const newEntryDirty = draftDirty && draft?.uid === null;

  // 表單切到「指定角色」時沿用原本的直接重抓語意；這條原本沒有錯誤訊息處理。
  function refreshCharactersForVisibility() {
    void readCast().catch(() => {});
  }

  /** 世界設定的通知：確定已刪的角色從開著的草稿與它的原樣快照拿掉（不製造新的未儲存，玩家其餘改動照留）；
   * 撤銷匯入與匯入合併的條目改動排進待定基，等條目重讀成功時套用；撤銷時讀不到世界書（null）就直接關掉既有條目草稿
   * （無從判斷，罕見不嚴防）。再重讀條目、勾選名單與帳本。後端存條目時另有防線，這裡只求畫面一致。 */
  async function applyWorldChange(removed: string[], entryChanges: (EntryChanges | null)[]) {
    if (removed.length > 0) {
      updateDraft((current) => {
        if (current.draft === null) return current;
        const origin =
          current.origin === ""
            ? current.origin
            : JSON.stringify(
                pruneDraftCharacters(JSON.parse(current.origin) as WorldbookDraft, removed),
              );
        return { ...current, draft: pruneDraftCharacters(current.draft, removed), origin };
      });
    }
    for (const changes of entryChanges) {
      if (changes === null) {
        updateDraft((current) =>
          current.draft !== null && current.draft.uid !== null ? { ...current, ...CLOSED } : current,
        );
      } else {
        pendingRebase.current = mergePendingRebase(pendingRebase.current, changes);
      }
    }
    await Promise.all([
      refreshWorldbook().catch((reason) => setMessage(String(reason))),
      refreshCast(),
      refreshLedger(),
    ]);
  }

  /** 有待定基就跟著進行中的重讀（沒有就發一次）等到它套用完；某次重讀被較新的頂掉不算，重讀失敗回 false。 */
  async function awaitPendingRebase() {
    while (pendingRebase.current) {
      try {
        await (readInFlight.current ?? refreshWorldbook());
      } catch (reason) {
        if (!pendingRebase.current) break;
        setMessage(String(reason));
        return false;
      }
    }
    return true;
  }

  /** 寫回世界書之前：write＝手動儲存、轉成角色卡，一律等定基；leave＝離開／切條目的自動存，
   * 草稿乾淨（本來就不寫）就不等。呼叫端之後一律從 draftRef 取當下草稿再決定寫不寫。 */
  async function settleRebase(mode: "write" | "leave") {
    const current = draftRef.current;
    if (
      mode === "leave" &&
      (current.draft === null || current.draft.uid === null || !isDirty(current))
    ) {
      return true;
    }
    return await awaitPendingRebase();
  }

  // 帳本的「照原文送模型」開關＝重用既有 upsert_worldbook_entry 反轉該條目的 disabled；
  // 找不到該 uid 就跳過（條目已被刪，不是這裡的錯）。
  async function toggleLedgerEntry(ledgerEntry: LedgerEntry) {
    setMessage("");
    // 撤銷／匯入重讀的空窗裡清單可能還是舊的：先等定基與進行中的重讀，再從最新清單取整條
    if (!(await awaitPendingRebase())) return;
    await readInFlight.current?.catch(() => {});
    const target = entriesRef.current.find((entry) => entry.uid === ledgerEntry.uid);
    if (!target || target.locked) return;
    try {
      await invoke<number>("upsert_worldbook_entry", {
        worldId: world,
        entry: { ...target, disabled: !target.disabled },
        create: false,
      });
      // 同一條的草稿若開著，enabled 跟上開關，免得自動存蓋回
      pendingRebase.current = mergePendingRebase(pendingRebase.current, {
        changed: [{ uid: target.uid, groups: ["enabled"] }],
        deleted: [],
      });
      await refreshWorldbook();
      await refreshLedger();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  // 條目表單按取消＝丟資料，先問過（自動存只走切換編輯對象那條路）
  async function confirmDiscardDraft() {
    if (!isDirty(draftRef.current)) return true;
    return await confirm(t("unsavedLeaveConfirm", { n: 1 }), {
      title: t("unsavedLeaveTitle"),
      kind: "warning",
      okLabel: t("unsavedLeaveOk"),
      cancelLabel: t("unsavedLeaveCancel"),
    });
  }

  /** 把表單寫回世界書：saved＝寫好了；gone＝既有條目已不在（後端拒寫），這份草稿已關（closeOnGone 為
   * false 時留著）；failed＝原因留在清單訊息列（表單不關） */
  async function persistDraft(
    source: DraftState,
    { closeOnGone = true }: { closeOnGone?: boolean } = {},
  ): Promise<"saved" | "gone" | "failed"> {
    const form = source.draft;
    if (!form) return "saved";
    const visibility = draftVisibility(form);
    const entry: WorldbookEntry = {
      uid: form.uid ?? Number.MAX_SAFE_INTEGER,
      title: form.title.trim(),
      keys: parseKeys(form.keys),
      content: form.content,
      constant: form.constant,
      order: form.order,
      disabled: !form.enabled,
      locked: false,
      visibility,
    };
    try {
      await invoke<number>("upsert_worldbook_entry", {
        worldId: world,
        entry,
        create: form.uid === null,
      });
      await refreshWorldbook();
      return "saved";
    } catch (reason) {
      setMessage(String(reason));
      if (backendCode(reason) !== "worldbook_entry_gone") return "failed";
      // 只關送出的那份：等待途中新開的草稿不動
      if (closeOnGone) {
        updateDraft((current) => (current.seq === source.seq ? { ...current, ...CLOSED } : current));
      }
      await refreshWorldbook().catch(() => {});
      return "gone";
    }
  }

  // 換編輯對象＝把手上這條存起來就走（條目本來就是即時寫檔，多問一次只是擋路）。
  // 還沒存過的新條目例外：直接存會把半成品留在清單上，照舊問。
  async function openDraft(next: WorldbookDraft) {
    let autoSaved = false;
    let gone = false;
    const current = draftRef.current;
    if (current.draft && isDirty(current)) {
      if (current.draft.uid === null) {
        if (!(await confirmDiscardDraft())) return;
      } else {
        if (!(await settleRebase("leave"))) return;
        const latest = draftRef.current;
        if (latest.draft && latest.draft.uid !== null && isDirty(latest)) {
          const result = await persistDraft(latest);
          if (result === "failed") return;
          autoSaved = result === "saved";
          gone = result === "gone";
        }
      }
    }
    // 自動存回「已被刪除」：留著那則訊息
    if (!gone) setMessage(autoSaved ? t("worldbookEntrySaved") : "");
    updateDraft((state) => ({ draft: next, origin: JSON.stringify(next), seq: state.seq + 1 }));
  }

  async function closeDraft() {
    if (await confirmDiscardDraft()) updateDraft((current) => ({ ...current, ...CLOSED }));
  }

  /** 表單改字 */
  function setDraft(next: WorldbookDraft) {
    updateDraft((current) => ({ ...current, draft: next }));
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
    void openDraft(draftFromEntry(entry, t("listSeparator")));
  }

  async function saveEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draftRef.current.draft) return;
    setMessage("");
    if (!(await settleRebase("write"))) return;
    const latest = draftRef.current;
    if (!latest.draft) return;
    // 玩家親手按存檔被拒（條目已不在）：留著表單，文字才複製得走〔作者裁決 2026-10-29〕；切換、離開照舊關
    const result = await persistDraft(latest, { closeOnGone: false });
    if (result !== "saved") return;
    updateDraft((current) => (current.seq === latest.seq ? { ...current, ...CLOSED } : current));
    setMessage(t("worldbookEntrySaved"));
  }

  async function deleteEntry(entry: WorldbookEntry) {
    setMessage("");
    try {
      const accepted = await confirm(
        t("worldbookDeleteConfirm", { title: entry.title || String(entry.uid) }),
        {
          title: t("worldbookDeleteTitle"),
          kind: "warning",
          okLabel: t("dialogDelete"),
          cancelLabel: t("dialogCancel"),
        },
      );
      if (!accepted) return;
      await invoke("delete_worldbook_entry", { worldId: world, uid: entry.uid });
      await refreshWorldbook();
      updateDraft((current) =>
        current.draft?.uid === entry.uid ? { ...current, ...CLOSED } : current,
      );
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function reorderEntries(ordered: WorldbookEntry[]) {
    setMessage("");
    const previous = entries;
    setEntries(ordered);
    try {
      await invoke("reorder_worldbook_entries", {
        worldId: world,
        uids: ordered.map((entry) => entry.uid),
      });
    } catch (reason) {
      setEntries(previous);
      setMessage(String(reason));
    }
  }

  // 去重上線前重複匯入過的桌，用這顆自己收拾：同內容只留排最前面那條
  async function dedupeWorldbook() {
    setMessage("");
    try {
      const accepted = await confirm(t("worldbookDedupeConfirm"), {
        title: t("worldbookDedupe"),
        kind: "warning",
        okLabel: t("worldbookDedupe"),
        cancelLabel: t("dialogCancel"),
      });
      if (!accepted) return;
      // 去重會改留下那條的可見度：開著的既有條目草稿先存起來並關掉，不留過期草稿
      if (!(await flushAndCloseExistingDraft())) return;
      // 去重只刪東西，別觸發匯入後的選 GM／改桌名
      const removed = await invoke<number>("dedupe_worldbook", { worldId: world });
      if (removed > 0) await refreshWorldbook();
      setMessage(removed > 0 ? t("worldbookDedupeDone", { n: removed }) : t("worldbookDedupeNone"));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function exportWorldbook() {
    setMessage("");
    try {
      const path = await saveDialog({
        defaultPath: "worldbook.json",
        filters: [{ name: t("worldbookJson"), extensions: ["json"] }],
      });
      if (!path) return;
      await invoke("export_worldbook", { worldId: world, path });
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  // 手動「轉成角色卡」一律轉一般卡——玩家卡另有從頭建立的入口，AI 卡重構的勾選畫面也能指定
  // 玩家卡（要點 4），這顆按鈕不再問「要不要轉成玩家卡」。
  async function convertEntryToCharacter() {
    if (!draftRef.current.draft || draftRef.current.draft.uid === null) return;
    setMessage("");
    if (!(await settleRebase("write"))) return;
    const latest = draftRef.current;
    if (!latest.draft || latest.draft.uid === null) return;
    try {
      const meta = await invoke<CharacterMeta>("worldbook_entry_to_character", {
        worldId: world,
        uid: latest.draft.uid,
        color: convertColor,
        asPlayer: false,
      });
      updateDraft((current) => (current.seq === latest.seq ? { ...current, ...CLOSED } : current));
      await refreshWorldbook();
      setMessage(t("convertEntryDone", { name: meta.name }));
      await onEntryConverted();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function flushExistingDraftForLeave() {
    const current = draftRef.current;
    if (!current.draft || current.draft.uid === null || !isDirty(current)) return true;
    if (!(await settleRebase("leave"))) return false;
    const latest = draftRef.current;
    if (!latest.draft || latest.draft.uid === null || !isDirty(latest)) return true;
    const result = await persistDraft(latest);
    if (result === "failed") return false;
    updateDraft((state) => (state.seq === latest.seq ? { ...state, ...CLOSED } : state));
    return true;
  }

  /** 整批改寫世界書之前（清回、去重）：開著的既有條目草稿存起來再關掉（乾淨的也關），存不起來回 false；
   * 新條目草稿不動 */
  async function flushAndCloseExistingDraft() {
    if (!(await flushExistingDraftForLeave())) return false;
    updateDraft((current) =>
      current.draft !== null && current.draft.uid !== null ? { ...current, ...CLOSED } : current,
    );
    return true;
  }

  /** 重構套用失敗後的補救（刷新前）：開著的既有條目草稿有改動就存，存成才關；條目已被部分套用刪掉（gone）或
   * 存不起來都留著不關、回 false，未存文字不丟。乾淨的照 flushAndCloseExistingDraft 關掉。新條目草稿不動 */
  async function flushExistingDraftForRecovery() {
    const current = draftRef.current;
    if (current.draft && current.draft.uid !== null && isDirty(current)) {
      if (!(await settleRebase("leave"))) return false;
      const latest = draftRef.current;
      if (latest.draft && latest.draft.uid !== null && isDirty(latest)) {
        if ((await persistDraft(latest, { closeOnGone: false })) !== "saved") return false;
        updateDraft((state) => (state.seq === latest.seq ? { ...state, ...CLOSED } : state));
        return true;
      }
    }
    return flushAndCloseExistingDraft();
  }

  return {
    entries,
    ledger,
    characters,
    message,
    draft,
    draftOrigin,
    draftSeq,
    draftDirty,
    newEntryDirty,
    setMessage,
    setDraft,
    refreshWorldbook,
    refreshLedger,
    refreshCast,
    applyWorldChange,
    refreshCharactersForVisibility,
    toggleLedgerEntry,
    addEntry,
    editEntry,
    closeDraft,
    saveEntry,
    deleteEntry,
    reorderEntries,
    dedupeWorldbook,
    exportWorldbook,
    convertEntryToCharacter,
    flushExistingDraftForLeave,
    flushAndCloseExistingDraft,
    flushExistingDraftForRecovery,
  };
}

export type WorldbookEditorController = ReturnType<typeof useWorldbookEditor>;
