// 牌桌狀態列：平欄摘要（時間｜地點｜在場）是收合時唯一的一行，本身就是開關；
// 展開後是可點擊編輯的欄位與可折疊的狀態樹。開關記在 localStorage，跨桌沿用。
import { Fragment, useMemo, useState } from "react";
import { t } from "../../i18n";
import { BranchBinding, StateNode } from "../../shared/contracts/backend-contracts";
import { CharacterCard, CharacterMeta } from "../characters/card-model";
import { EditingStateField, treeValueAt } from "./useTableStateController";
import { IconExpand } from "../../shared/ui/icons";

const STATE_BAR_OPEN_KEY = "state_bar_open";

// 值裡的字面 {{user}} 只在顯示時換成玩家名（模型上下文與存檔仍是原文，後端注入前才代換）；
// 大小寫不分、容許中間空白（{{ user }}），其他巨集不動
const USER_MACRO = /\{\{\s*user\s*\}\}/gi;
function displayUserMacro(value: string, playerName: string): string {
  return value.replace(USER_MACRO, playerName);
}

interface StateBarProps {
  /** 平欄（time／place／present 與卡自訂欄） */
  fields: Record<string, string>;
  tree: Record<string, StateNode>;
  jumps: Record<string, string>;
  bindings: BranchBinding[];
  editing: EditingStateField | null;
  onBeginEdit: (path: string[], tree: boolean, value: string) => void;
  onChangeEditValue: (next: string) => void;
  onSave: (path: string[], tree: boolean, value: string) => void;
  onCancelEdit: () => void;
  onMarkCounter: (path: string[]) => void;
  onBind: (characterId: string, path: string[] | null) => void;
  /** 玩家卡：分支預設展開哪一支靠它的指認，欄位值裡的 {{user}} 也用它的名字 */
  player: CharacterCard | null;
  /** 這桌的角色總數：一個都沒有就不掛分支指認下拉 */
  castCount: number;
  cast: CharacterMeta[];
}

/** 主欄的狀態列：平欄摘要＋可折疊的狀態樹，每一格點著就能改 */
export function StateBar({
  fields,
  tree,
  jumps,
  bindings,
  editing,
  onBeginEdit,
  onChangeEditValue,
  onSave,
  onCancelEdit,
  onMarkCounter,
  onBind,
  player,
  castCount,
  cast,
}: StateBarProps) {
  const [stateBarOpen, setStateBarOpen] = useState(
    () => localStorage.getItem(STATE_BAR_OPEN_KEY) === "true",
  );

  const stateFields = [
    { key: "time", label: t("stateFieldTime") },
    { key: "place", label: t("stateFieldPlace") },
    { key: "present", label: t("stateFieldPresent") },
    ...Object.keys(fields)
      .filter((key) => !["time", "place", "present"].includes(key))
      .map((key) => ({ key, label: key })),
  ];
  // 收合摘要用短標籤：時間、地點照欄位名；在場用自帶冒號的短標（「在場：」），展開區才用全名
  const summaryFields = [
    { key: "time", label: `${t("stateFieldTime")} ` },
    { key: "place", label: `${t("stateFieldPlace")} ` },
    { key: "present", label: t("stateSummaryPresent") },
  ];
  const stateValue = (key: string) => fields[key] || t("stateEmptyValue");

  // 表單交給瀏覽器處理 Enter，中文輸入法選字時不會提前送出。
  function stateFieldForm(path: string[], isTree: boolean, label: string) {
    const value = editing?.value ?? "";
    return (
      <form
        className="state-bar-field-form"
        onSubmit={(event) => {
          event.preventDefault();
          onSave(path, isTree, value);
        }}
      >
        <input
          className="state-bar-input"
          autoFocus
          value={value}
          aria-label={label}
          onChange={(event) => {
            const next = event.currentTarget.value;
            onChangeEditValue(next);
          }}
          onBlur={() => onSave(path, isTree, value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              onCancelEdit();
            }
          }}
        />
      </form>
    );
  }

  // 一列點著就能改的欄位：平欄與樹葉子共用，差別只在存回哪裡
  function stateLeafRow(path: string[], isTree: boolean, label: string) {
    const isEditing =
      editing?.tree === isTree &&
      editing.path.length === path.length &&
      editing.path.every((segment, index) => segment === path[index]);
    const value = isTree ? treeValueAt(tree, path) : (fields[path[0]] ?? "");
    const jumpMark = jumps[path.join(".")];
    return (
      <div className="state-bar-field" key={path.join("\0")}>
        <span className="state-bar-label">{label}</span>
        {isEditing ? (
          stateFieldForm(path, isTree, label)
        ) : (
          <div className="state-bar-value-row">
            <button
              className="state-bar-value"
              type="button"
              title={t("stateEditHint")}
              onClick={() => onBeginEdit(path, isTree, value)}
            >
              {value ? displayUserMacro(value, player?.name || t("playerLabel")) : t("stateEmptyValue")}
            </button>
            {jumpMark && (
              <button
                className="state-bar-jump"
                type="button"
                title={t("stateJumpHint")}
                onClick={() => onMarkCounter(path)}
              >
                {"⚠ " + jumpMark}
              </button>
            )}
          </div>
        )}
      </div>
    );
  }

  // 玩家卡目前指認到的分支路徑，含所有祖先（自己與上面每一層都要預設展開），沒指認就是空集合
  const openBranchPaths = useMemo(() => {
    const bound = player && bindings.find((b) => b.characterId === player.id);
    const set = new Set<string>();
    if (bound) {
      for (let depth = 1; depth <= bound.path.length; depth += 1) {
        set.add(bound.path.slice(0, depth).join("/"));
      }
    }
    return set;
  }, [player, bindings]);

  // 樹狀折疊：分支一層層收起來，預設展開第一層與玩家自己那支；summary 上附分支指認下拉
  function stateTreeNodes(nodes: Record<string, StateNode>, path: string[], depth: number) {
    return Object.entries(nodes).map(([key, node]) => {
      const childPath = [...path, key];
      if (typeof node === "string") return stateLeafRow(childPath, true, key);
      const bound = bindings.find(
        (binding) =>
          binding.path.length === childPath.length &&
          binding.path.every((segment, index) => segment === childPath[index]),
      );
      // 清單出身的分支（鍵全是數字索引，如「氣味標記者：0→利格魯德」的名冊）不是誰的狀態包，
      // 掛指認下拉會被讀成「挑誰出場」；只有名字鍵的物件分支才提供指認。
      const isList = Object.keys(node).every((childKey) => /^\d+$/.test(childKey));
      return (
        <details
          className="state-tree-branch"
          key={key}
          open={depth === 0 || openBranchPaths.has(childPath.join("/"))}
        >
          <summary>
            {key}
            {castCount > 0 && !isList && (
              <select
                className="state-tree-bind"
                aria-label={t("stateBranchBindAria")}
                title={t("stateBranchBindHint")}
                value={bound?.characterId ?? ""}
                onClick={(event) => event.stopPropagation()}
                onPointerDown={(event) => event.stopPropagation()}
                onChange={(event) => {
                  const nextId = event.currentTarget.value;
                  if (nextId) onBind(nextId, childPath);
                  else if (bound) onBind(bound.characterId, null);
                }}
              >
                <option value="">{t("stateBranchUnbound")}</option>
                {cast.map((character) => (
                  <option key={character.id} value={character.id}>
                    {character.name}
                  </option>
                ))}
              </select>
            )}
          </summary>
          <div className="state-tree-children">{stateTreeNodes(node, childPath, depth + 1)}</div>
        </details>
      );
    });
  }

  return (
    <details
      className="state-bar"
      open={stateBarOpen}
      onToggle={(event) => {
        const next = event.currentTarget.open;
        setStateBarOpen(next);
        localStorage.setItem(STATE_BAR_OPEN_KEY, String(next));
      }}
    >
      {/* 單行摘要：三段各自截短、整行不換行，展開與否都留著當開關 */}
      <summary>
        <IconExpand className="state-bar-chevron" />
        {summaryFields.map(({ key, label }, index) => (
          <Fragment key={key}>
            {index > 0 && <span className="state-bar-pipe">｜</span>}
            <span className="state-bar-seg" title={`${label}${stateValue(key)}`}>
              <b>{label}</b>
              {stateValue(key)}
            </span>
          </Fragment>
        ))}
      </summary>
      <div className="state-bar-fields">
        {stateFields.map(({ key, label }) => stateLeafRow([key], false, label))}
        {stateTreeNodes(tree, [], 0)}
      </div>
    </details>
  );
}
