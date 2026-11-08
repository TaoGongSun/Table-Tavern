// 狀態列／狀態樹 controller：這桌的平欄、樹、跳動記號、分支指認與「正在編輯哪一格」。
// 所有權從 App() 搬過來，行為與依賴陣列照舊；畫面（stateLeafRow／stateTreeNodes）仍在 App。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type BranchBinding, type StateNode, type WorldState } from "../../shared/contracts/backend-contracts";
import { parseWithFormat, prefixFor, stripPrefix } from "./state-format";

// 路徑指到的葉子值；中途撞到分支或缺節點都當空字串（面板只讀，取不到就是沒東西可改）
export function treeValueAt(tree: Record<string, StateNode>, path: string[]): string {
  let node: StateNode | undefined = tree[path[0]];
  for (const key of path.slice(1)) {
    if (typeof node !== "object" || node === null) return "";
    node = node[key];
  }
  return typeof node === "string" ? node : "";
}

function isPanelListItem(path: string[]): boolean {
  return /^\d+$/.test(path[path.length - 1] ?? "") && (
    (path.length === 4 && path[0] === "characters" && ["items", "conditions"].includes(path[2])) ||
    (path.length === 3 && path[0] === "tracking" && ["goals", "promises", "clues"].includes(path[1])) ||
    (path.length === 3 && path[0] === "combat" && path[1] === "order") ||
    (path.length === 5 && path[0] === "combat" && path[1] === "foes" && path[3] === "conditions")
  );
}

export type RefusedMarks = NonNullable<WorldState["state"]["refused"]>;
export type RefusedMark = RefusedMarks[string];

// 索引只用來定位畫面上的項目；後端一律接收清單父路徑與整串字串陣列。
function panelListAfterEdit(tree: Record<string, StateNode>, path: string[], value: string): string[] {
  let node: StateNode = tree;
  for (const key of path.slice(0, -1)) {
    if (typeof node === "string") return [];
    node = node[key] ?? {};
  }
  if (typeof node === "string") return [];
  return Object.entries(node).sort(([a], [b]) => Number(a) - Number(b))
    .flatMap(([key, item]) => {
      if (key === path[path.length - 1]) return value === "" ? [] : [value];
      return typeof item === "string" ? [item] : [];
    });
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
  defaultPanel: boolean;
  diceEnabled: boolean;
  addPanelField: (parentPath: string[], name: string) => Promise<void>;
  removePanelField: (path: string[]) => Promise<void>;
  removePanelItem: (path: string[]) => Promise<void>;
  /** 不經編輯框直接寫一格（點名字設輪到誰） */
  setLeaf: (path: string[], value: string) => Promise<void>;
  /** 預設桌整串改清單（先攻順序上下移） */
  setPanelList: (path: string[], items: string[]) => Promise<void>;
  /** 上一次套用改到的欄位（點分路徑 → "-7"／"更新"） */
  changes: Record<string, string>;
  /** 戰鬥欄的拒收標記 */
  refused: RefusedMarks;
  /** 這一幕剛由 GM 收場（先攻非空→空） */
  combatEnded: boolean;
  tree: Record<string, StateNode>;
  /** 全量桌的跳動警示：路徑（點分）→ 顯示標記 */
  jumps: Record<string, string>;
  /** 數字欄的顯示模板：路徑（點分）→「第 {} 天」這類原寫法 */
  displayFormats: Record<string, string>;
  /** 鏡像欄：路徑（點分）→ 跟著哪個固定鍵（place／present）走，畫面上唯讀 */
  mirrors: Record<string, string>;
  /** 顯示前綴：路徑（點分，可含 *）→「👤 」這類開頭符號 */
  displayPrefixes: Record<string, string>;
  /** 在場名單集合的路徑（點分）：狀態列只列 present 裡的人 */
  presentScoped: string[];
  /** 每條分支目前綁給哪個角色 */
  bindings: BranchBinding[];
  editing: EditingStateField | null;
  /** 換桌：把 read_state 的結果與已讀好的綁定清單一次同步塞進來（不得在裡面 await） */
  hydrate: (
    state: WorldState["state"] | undefined,
    bindings: BranchBinding[],
    displayFormats?: Record<string, string>,
    mirrors?: Record<string, string>,
    displayPrefixes?: Record<string, string>,
    collections?: MechanismCollections,
    defaultPanel?: boolean,
    diceEnabled?: boolean,
    scene?: number,
  ) => void;
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

type MechanismCollections = NonNullable<WorldState["mechanism"]>["collections"];

function presentScopedPaths(collections: MechanismCollections): string[] {
  return Object.entries(collections ?? {})
    .filter(([, spec]) => spec.present_scoped)
    .map(([path]) => path);
}

export function useTableStateController(input: {
  worldId: string;
  onError: (message: string) => void;
  /** 手改存成功、狀態樹重讀完之後：變數模式下手改改的是逐字稿事件上的表，呼叫端要重讀事件（卡片介面才換得到新值） */
  onEdited?: () => void;
}): TableStateController {
  const { worldId, onError } = input;
  const onEditedRef = useRef(input.onEdited);
  onEditedRef.current = input.onEdited;
  const [defaultPanel, setDefaultPanel] = useState(false);
  const [diceEnabled, setDiceEnabled] = useState(false);
  const [tableState, setTableState] = useState<Record<string, string>>({});
  const [tableTree, setTableTree] = useState<Record<string, StateNode>>({});
  const [tableJumps, setTableJumps] = useState<Record<string, string>>({});
  const [tableChanges, setTableChanges] = useState<Record<string, string>>({});
  const [refused, setRefused] = useState<RefusedMarks>({});
  const [combatEnded, setCombatEnded] = useState(false);
  const [displayFormats, setDisplayFormats] = useState<Record<string, string>>({});
  const [mirrors, setMirrors] = useState<Record<string, string>>({});
  const [displayPrefixes, setDisplayPrefixes] = useState<Record<string, string>>({});
  const [presentScoped, setPresentScoped] = useState<string[]>([]);
  // 分支指認清單：每條狀態樹分支目前綁給哪個角色，換桌／讀狀態一起重載
  const [branchBindings, setBranchBindings] = useState<BranchBinding[]>([]);
  const [editingStateField, setEditingStateField] = useState<EditingStateField | null>(null);
  // Enter 送出會接著觸發 blur，Esc 也會先失焦；旗標避免重複送出或把取消誤存。
  const stateFieldSaveBusy = useRef(false);
  const stateFieldEditCancelled = useRef(false);

  // 換桌世代：hydrate 一次就換，在途的重讀回來時世代不同就不碰畫面
  const readGeneration = useRef(0);
  const hydrate = useCallback(
    (
      state: WorldState["state"] | undefined,
      bindings: BranchBinding[],
      formats?: Record<string, string>,
      mirrorPaths?: Record<string, string>,
      prefixes?: Record<string, string>,
      collections?: MechanismCollections,
      panel = false,
      dice = false,
      scene?: number,
    ) => {
      readGeneration.current += 1;
      setDefaultPanel(panel);
      setDiceEnabled(dice);
      setTableState(state?.table ?? {});
      setTableTree(state?.tree ?? {});
      setTableJumps(state?.jumps ?? {});
      setTableChanges(state?.changes ?? {});
      setRefused(state?.refused ?? {});
      setCombatEnded(scene !== undefined && state?.combat_ended === scene);
      setDisplayFormats(formats ?? {});
      setMirrors(mirrorPaths ?? {});
      setDisplayPrefixes(prefixes ?? {});
      setPresentScoped(presentScopedPaths(collections));
      setBranchBindings(bindings);
    },
    [],
  );

  // 重讀是兩趟非同步：同時只跑一趟，在途期間再被要求（卡片頻繁寫入、手改）就在這趟套用後補讀一次，
  // 回來的結果一律比在途前新，不會有晚回的舊結果蓋掉新值。換桌（worldId 變或 hydrate）換世代，
  // 舊世代的在途結果整份丟掉，新世代另起一趟、不併進舊的
  const currentWorldId = useRef(worldId);
  currentWorldId.current = worldId;
  const reading = useRef<{ worldId: string; generation: number; again: boolean; promise: Promise<void> } | null>(
    null,
  );

  const refresh = useCallback(() => {
    const running = reading.current;
    if (running && running.worldId === worldId && running.generation === readGeneration.current) {
      running.again = true;
      return running.promise;
    }
    const job = { worldId, generation: readGeneration.current, again: false, promise: Promise.resolve() };
    const current = () => currentWorldId.current === worldId && readGeneration.current === job.generation;
    job.promise = (async () => {
      try {
        do {
          job.again = false;
          const state = await invoke<WorldState>("read_state", { worldId });
          const bindings = await loadBranchBindings(worldId);
          if (!current()) return;
          setDefaultPanel(state.mechanism?.default_panel ?? false);
          setDiceEnabled(state.dice_enabled ?? false);
          setTableState(state.state?.table ?? {});
          setTableTree(state.state?.tree ?? {});
          setTableJumps(state.state?.jumps ?? {});
          setTableChanges(state.state?.changes ?? {});
          setRefused(state.state?.refused ?? {});
          setCombatEnded(state.state?.combat_ended === state.current_scene);
          setDisplayFormats(state.mechanism?.display_formats ?? {});
          setMirrors(state.mechanism?.mirrors ?? {});
          setDisplayPrefixes(state.mechanism?.display_prefixes ?? {});
          setPresentScoped(presentScopedPaths(state.mechanism?.collections));
          setBranchBindings(bindings);
        } while (job.again);
      } finally {
        if (reading.current === job) reading.current = null;
      }
    })();
    reading.current = job;
    return job.promise;
  }, [worldId]);

  // 房間 GM 落段後平欄已改：收合列以外的樹與自訂欄跟著重讀。沒開桌不聽
  useEffect(() => {
    if (!worldId) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<{ worldId: string }>("room-table-changed", (event) => {
      if (event.payload.worldId === worldId) void refresh();
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [worldId, refresh]);

  // 儲存前關掉輸入框，讓失敗時不會卡在一個可能已過期的欄位值上。
  const save = useCallback(
    async (path: string[], tree: boolean, input: string) => {
      if (stateFieldSaveBusy.current || stateFieldEditCancelled.current) return;
      const panelItem = defaultPanel && tree && isPanelListItem(path);
      // 先剝前綴、再照數字模板解析（顯示時反過來：先套模板、再接前綴）
      const value = tree && !panelItem
        ? parseWithFormat(stripPrefix(input, prefixFor(displayPrefixes, path)), displayFormats[path.join(".")])
        : input;
      setEditingStateField(null);
      if (!(panelItem && value === "") && value === (tree ? treeValueAt(tableTree, path) : (tableState[path[0]] ?? ""))) return;
      stateFieldSaveBusy.current = true;
      onError("");
      try {
        if (panelItem) await invoke("set_panel_list", {
          worldId, path: path.slice(0, -1), items: panelListAfterEdit(tableTree, path, value),
        });
        else if (tree) await invoke("set_state_path", { worldId, path, value });
        else await invoke("set_table_state", { worldId, fields: { [path[0]]: value } });
        await refresh();
        onEditedRef.current?.();
      } catch (reason) {
        onError(String(reason));
      } finally {
        stateFieldSaveBusy.current = false;
      }
    },
    [defaultPanel, tableTree, tableState, displayFormats, displayPrefixes, worldId, onError, refresh],
  );

  const removePanelItem = useCallback(async (path: string[]) => {
    stateFieldEditCancelled.current = false;
    await save(path, true, "");
  }, [save]);

  const setLeaf = useCallback(async (path: string[], value: string) => {
    stateFieldEditCancelled.current = false;
    await save(path, true, value);
  }, [save]);

  const setPanelList = useCallback(
    async (path: string[], items: string[]) => {
      onError("");
      try {
        await invoke("set_panel_list", { worldId, path, items });
        await refresh();
        onEditedRef.current?.();
      } catch (reason) {
        onError(String(reason));
      }
    },
    [worldId, onError, refresh],
  );

  const panelField = useCallback(
    async (command: string, args: Record<string, unknown>) => {
      onError("");
      try {
        await invoke(command, { worldId, ...args });
        await refresh();
        onEditedRef.current?.();
      } catch (reason) {
        onError(String(reason));
        throw reason;
      }
    },
    [worldId, onError, refresh],
  );
  const addPanelField = useCallback(
    (parentPath: string[], name: string) =>
      panelField("add_panel_field", { parentPath, name }),
    [panelField],
  );
  const removePanelField = useCallback(
    (path: string[]) => panelField("remove_panel_field", {
      parentPath: path.slice(0, -1), name: path[path.length - 1],
    }),
    [panelField],
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
      defaultPanel,
      diceEnabled,
      addPanelField,
      removePanelField,
      removePanelItem,
      setLeaf,
      setPanelList,
      changes: tableChanges,
      refused,
      combatEnded,
      fields: tableState,
      tree: tableTree,
      jumps: tableJumps,
      displayFormats,
      mirrors,
      displayPrefixes,
      presentScoped,
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
      defaultPanel,
      diceEnabled,
      addPanelField,
      removePanelField,
      removePanelItem,
      setLeaf,
      setPanelList,
      tableChanges,
      refused,
      combatEnded,
      tableState,
      tableTree,
      tableJumps,
      displayFormats,
      mirrors,
      displayPrefixes,
      presentScoped,
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
