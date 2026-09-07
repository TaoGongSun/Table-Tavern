import { type FormEvent, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type { Visibility, WorldbookEntry } from "../../shared/contracts/backend-contracts";
import type { CharacterMeta } from "../characters/card-model";
import { t } from "../../i18n";
import { EMPTY_LEDGER, type Ledger, type LedgerEntry, type WorldbookDraft } from "./worldbook-model";

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
  const [entries, setEntries] = useState<WorldbookEntry[]>([]);
  const [ledger, setLedger] = useState<Ledger>(EMPTY_LEDGER);
  const [characters, setCharacters] = useState<CharacterMeta[]>([]);
  const [message, setMessage] = useState("");
  const [draft, setDraft] = useState<WorldbookDraft | null>(null);
  // 條目表單開啟當下的快照，用來判斷「有沒有改過」（未儲存提示）
  const [draftOrigin, setDraftOrigin] = useState("");

  async function refreshCast() {
    try {
      const cast = await invoke<CharacterMeta[]>("list_characters", { worldId: world });
      setCharacters(cast.filter((character) => !character.archived));
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  useEffect(() => {
    setMessage("");
    setEntries([]);
    setLedger(EMPTY_LEDGER);
    setCharacters([]);
    setDraft(null);
    invoke<WorldbookEntry[]>("read_worldbook", { worldId: world })
      .then(setEntries)
      .catch((reason) => setMessage(String(reason)));
    // 帳本掛掉不該擋住世界書編輯：失敗就當空，不彈錯誤。
    invoke<Ledger>("mechanism_ledger", { worldId: world })
      .then(setLedger)
      .catch(() => setLedger(EMPTY_LEDGER));
    void refreshCast();
  }, [world]);

  const draftDirty = draft !== null && JSON.stringify(draft) !== draftOrigin;
  // 既有條目改到一半離開時會自動存，不算未儲存；只有還沒存過的新條目要提醒
  const newEntryDirty = draftDirty && draft?.uid === null;

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

  // 表單切到「指定角色」時沿用原本的直接重抓語意；這條原本沒有錯誤訊息處理。
  function refreshCharactersForVisibility() {
    void invoke<CharacterMeta[]>("list_characters", { worldId: world }).then((cast) =>
      setCharacters(cast.filter((character) => !character.archived)),
    );
  }

  // 帳本的「照原文送模型」開關＝重用既有 upsert_worldbook_entry 反轉該條目的 disabled；
  // 找不到該 uid 就跳過（條目已被刪，不是這裡的錯）。
  async function toggleLedgerEntry(ledgerEntry: LedgerEntry) {
    const target = entries.find((entry) => entry.uid === ledgerEntry.uid);
    if (!target || target.locked) return;
    setMessage("");
    try {
      await invoke<number>("upsert_worldbook_entry", {
        worldId: world,
        entry: { ...target, disabled: !target.disabled },
      });
      await refreshWorldbook();
      await refreshLedger();
    } catch (reason) {
      setMessage(String(reason));
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
      setMessage(String(reason));
      return false;
    }
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
    setMessage(autoSaved ? t("worldbookEntrySaved") : "");
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

  async function saveEntry(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft) return;
    setMessage("");
    if (!(await persistDraft(draft))) return;
    setDraft(null);
    setMessage(t("worldbookEntrySaved"));
  }

  async function deleteEntry(entry: WorldbookEntry) {
    setMessage("");
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
      });
      if (!accepted) return;
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
    if (!draft || draft.uid === null) return;
    setMessage("");
    try {
      const meta = await invoke<CharacterMeta>("worldbook_entry_to_character", {
        worldId: world,
        uid: draft.uid,
        color: convertColor,
        asPlayer: false,
      });
      setDraft(null);
      await refreshWorldbook();
      setMessage(t("convertEntryDone", { name: meta.name }));
      await onEntryConverted();
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  async function flushExistingDraftForLeave() {
    if (!draft || !draftDirty || draft.uid === null) return true;
    if (!(await persistDraft(draft))) return false;
    setDraft(null);
    return true;
  }

  return {
    entries,
    ledger,
    characters,
    message,
    draft,
    draftOrigin,
    draftDirty,
    newEntryDirty,
    setMessage,
    setDraft,
    refreshWorldbook,
    refreshLedger,
    refreshCast,
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
  };
}

export type WorldbookEditorController = ReturnType<typeof useWorldbookEditor>;
