// 狀態列／狀態樹 controller：這桌的平欄、樹、跳動記號、分支指認與「正在編輯哪一格」。
// 所有權從 App() 搬過來，行為與依賴陣列照舊；畫面（stateLeafRow／stateTreeNodes）仍在 App。
import { useCallback, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { type BranchBinding, type StateNode, type WorldState } from "../backend-contracts";

// 路徑指到的葉子值；中途撞到分支或缺節點都當空字串（面板只讀，取不到就是沒東西可改）
export function treeValueAt(tree: Record<string, StateNode>, path: string[]): string {
  let node: StateNode | undefined = tree[path[0]];
  for (const key of path.slice(1)) {
    if (typeof node !== "object" || node === null) return "";
    node = node[key];
  }
  return typeof node === "string" ? node : "";
}

// 綁定清單載入失敗就當空陣列——面板本來就能在沒有綁定資料時正常運作，不因此整個掛掉
export async function loadBranchBindings(worldId: string): Promise<BranchBinding[]> {
  try {
    return await invoke<BranchBinding[]>("branch_bindings", { worldId });
  } catch {
    return [];
  }
}

/** 編輯中的欄位：path 是樹裡的完整路徑，平欄則是長度 1 的路徑（tree=false，走舊的單層存檔） */
export interface EditingStateField {
  path: string[];
  tree: boolean;
  value: string;
}

export interface TableStateController {
  /** 平欄（time／place／present 與卡自訂欄） */
  fields: Record<string, string>;
  tree: Record<string, StateNode>;
  /** 全量桌的跳動警示：路徑（點分）→ 顯示標記 */
  jumps: Record<string, string>;
  /** 每條分支目前綁給哪個角色 */
  bindings: BranchBinding[];
  editing: EditingStateField | null;
  /** 換桌：把 read_state 的結果與已讀好的綁定清單一次同步塞進來（不得在裡面 await） */
  hydrate: (state: WorldState["state"] | undefined, bindings: BranchBinding[]) => void;
  /** 重讀這桌的狀態與綁定 */
  refresh: () => Promise<void>;
  save: (path: string[], tree: boolean, value: string) => Promise<void>;
  /** 玩家點跳動記號：把該欄永久標成計數器 */
  markCounter: (path: string[]) => Promise<void>;
  /** 指認／解除分支給角色 */
  bind: (characterId: string, path: string[] | null) => Promise<void>;
  beginEdit: (path: string[], tree: boolean, value: string) => void;
  changeEditValue: (next: string) => void;
  /** Esc 取消：連同「這次別存」的旗標一起立起來 */
  cancelEdit: () => void;
  /** 單純收掉編輯框（換桌用），不動取消旗標 */
  clearEdit: () => void;
}

export function useTableStateController(input: {
  worldId: string;
  onError: (message: string) => void;
}): TableStateController {
  const { worldId, onError } = input;
  const [tableState, setTableState] = useState<Record<string, string>>({});
  const [tableTree, setTableTree] = useState<Record<string, StateNode>>({});
  const [tableJumps, setTableJumps] = useState<Record<string, string>>({});
  // 分支指認清單：每條狀態樹分支目前綁給哪個角色，換桌／讀狀態一起重載
  const [branchBindings, setBranchBindings] = useState<BranchBinding[]>([]);
  const [editingStateField, setEditingStateField] = useState<EditingStateField | null>(null);
  // Enter 送出會接著觸發 blur，Esc 也會先失焦；旗標避免重複送出或把取消誤存。
  const stateFieldSaveBusy = useRef(false);
  const stateFieldEditCancelled = useRef(false);

  const hydrate = useCallback((state: WorldState["state"] | undefined, bindings: BranchBinding[]) => {
    setTableState(state?.table ?? {});
    setTableTree(state?.tree ?? {});
    setTableJumps(state?.jumps ?? {});
    setBranchBindings(bindings);
  }, []);

  // 重讀是兩趟非同步，中途換桌的話上一桌的結果會晚一步蓋掉新桌的畫面；
  // 回來時比對現在還是不是同一桌，不是就整份丟掉
  const currentWorldId = useRef(worldId);
  currentWorldId.current = worldId;

  const refresh = useCallback(async () => {
    const state = await invoke<WorldState>("read_state", { worldId });
    const bindings = await loadBranchBindings(worldId);
    if (currentWorldId.current !== worldId) return;
    setTableState(state.state?.table ?? {});
    setTableTree(state.state?.tree ?? {});
    setTableJumps(state.state?.jumps ?? {});
    setBranchBindings(bindings);
  }, [worldId]);

  // 儲存前關掉輸入框，讓失敗時不會卡在一個可能已過期的欄位值上。
  const save = useCallback(
    async (path: string[], tree: boolean, value: string) => {
      if (stateFieldSaveBusy.current || stateFieldEditCancelled.current) return;
      setEditingStateField(null);
      if (value === (tree ? treeValueAt(tableTree, path) : (tableState[path[0]] ?? ""))) return;
      stateFieldSaveBusy.current = true;
      onError("");
      try {
        if (tree) await invoke("set_state_path", { worldId, path, value });
        else await invoke("set_table_state", { worldId, fields: { [path[0]]: value } });
        await refresh();
      } catch (reason) {
        onError(String(reason));
      } finally {
        stateFieldSaveBusy.current = false;
      }
    },
    [tableTree, tableState, worldId, onError, refresh],
  );

  // 玩家點跳動記號：把該欄永久標成計數器，之後全量桌跳動比對不再對它示警
  const markCounter = useCallback(
    async (path: string[]) => {
      onError("");
      try {
        await invoke("mark_state_counter", { worldId, path });
        await refresh();
      } catch (reason) {
        onError(String(reason));
      }
    },
    [worldId, onError, refresh],
  );

  // 指認／解除分支給角色；成功後重載綁定清單，失敗照面板既有規矩交給 onError
  const bind = useCallback(
    async (characterId: string, path: string[] | null) => {
      try {
        await invoke("set_branch_binding", { worldId, characterId, path });
        setBranchBindings(await loadBranchBindings(worldId));
      } catch (reason) {
        onError(String(reason));
      }
    },
    [worldId, onError],
  );

  const beginEdit = useCallback((path: string[], tree: boolean, value: string) => {
    stateFieldEditCancelled.current = false;
    setEditingStateField({ path, tree, value });
  }, []);

  const changeEditValue = useCallback((next: string) => {
    setEditingStateField((previous) => (previous ? { ...previous, value: next } : previous));
  }, []);

  const cancelEdit = useCallback(() => {
    stateFieldEditCancelled.current = true;
    setEditingStateField(null);
  }, []);

  const clearEdit = useCallback(() => setEditingStateField(null), []);

  return useMemo(
    () => ({
      fields: tableState,
      tree: tableTree,
      jumps: tableJumps,
      bindings: branchBindings,
      editing: editingStateField,
      hydrate,
      refresh,
      save,
      markCounter,
      bind,
      beginEdit,
      changeEditValue,
      cancelEdit,
      clearEdit,
    }),
    [
      tableState,
      tableTree,
      tableJumps,
      branchBindings,
      editingStateField,
      hydrate,
      refresh,
      save,
      markCounter,
      bind,
      beginEdit,
      changeEditValue,
      cancelEdit,
      clearEdit,
    ],
  );
}
