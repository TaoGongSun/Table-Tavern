import { type ReactNode, useState } from "react";
import { t } from "../../i18n";
import { type StateNode } from "../../shared/contracts/backend-contracts";
import { CombatSection, RefusedBadge } from "./CombatSection";
import { type RefusedMarks } from "./useTableStateController";

const TRACKING_LABELS = {
  goals: "statePanelGoals",
  promises: "statePanelPromises",
  clues: "statePanelClues",
} as const;
const RESERVED = [
  "characters",
  "status",
  "items",
  "tracking",
  "goals",
  "promises",
  "clues",
  "hp",
  "conditions",
  "combat",
];
const branch = (node: StateNode | undefined): Record<string, StateNode> =>
  typeof node === "object" && node !== null ? node : {};

type PanelProps = {
  tree: Record<string, StateNode>;
  renderLeaf: (path: string[], label: string) => ReactNode;
  onAdd?: (parentPath: string[], name: string) => Promise<void>;
  onRemove?: (path: string[]) => Promise<void>;
  onRemoveItem?: (path: string[]) => Promise<void>;
  /** 這一幕剛由 GM 收場（顯示「戰鬥結束」一行） */
  combatEnded?: boolean;
  refused?: RefusedMarks;
  /** 整串改一個清單欄（存檔走 set_panel_list） */
  onSetList?: (path: string[], items: string[]) => void;
  onSetOrder?: (order: string[]) => void;
  onSetTurn?: (name: string) => void;
};

function AddField({
  path,
  node,
  onAdd,
}: {
  path: string[];
  node: Record<string, StateNode>;
  onAdd: PanelProps["onAdd"];
}) {
  const [name, setName] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const trimmed = name?.trim() ?? "";
  const invalid = RESERVED.includes(trimmed) || Object.hasOwn(node, trimmed);
  if (name === null)
    return (
      <button type="button" onClick={() => setName("")}>
        {t("statePanelAddField")}
      </button>
    );
  return (
    <form
      className="state-panel-add"
      onSubmit={async (event) => {
        event.preventDefault();
        if (!trimmed || invalid || busy || !onAdd) return;
        setBusy(true);
        try {
          await onAdd(path, trimmed);
          setName(null);
        } catch {
          /* controller 顯示後端錯誤，保留欄名供修正 */
        } finally {
          setBusy(false);
        }
      }}
    >
      <input
        autoFocus
        aria-label={t("statePanelFieldName")}
        value={name}
        disabled={busy}
        onChange={(event) => setName(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape" && !busy) setName(null);
        }}
      />
      <button type="submit" disabled={!trimmed || invalid || busy || !onAdd}>
        {t("statePanelAddField")}
      </button>
      <button type="button" disabled={busy} onClick={() => setName(null)}>
        {t("dialogCancel")}
      </button>
      {invalid && <span role="alert">{t("statePanelInvalidName")}</span>}
    </form>
  );
}

function RemoveField({
  path,
  onRemove,
  label = t("statePanelRemoveField"),
}: {
  path: string[];
  onRemove: PanelProps["onRemove"];
  label?: string;
}) {
  const [busy, setBusy] = useState(false);
  return (
    <button
      type="button"
      disabled={busy || !onRemove}
      aria-label={label + " " + path[path.length - 1]}
      onPointerDown={(event) => event.preventDefault()}
      onClick={async () => {
        if (!onRemove || busy) return;
        setBusy(true);
        try {
          await onRemove(path);
        } catch {
          /* controller 顯示錯誤，欄位保留 */
        } finally {
          setBusy(false);
        }
      }}
    >
      ×
    </button>
  );
}

/** 清單欄（持有物、追蹤三欄、狀態效果）：逐項 chip 可改可刪；「編輯」或點拒收標記打開整串編輯，一行一項，
 *  空清單也能補，存檔整串走 set_panel_list。角色與敵人共用這一份。 */
function PanelList({
  items,
  path,
  label,
  mark,
  renderLeaf,
  onRemoveItem,
  onSetList,
}: {
  items: Record<string, StateNode>;
  path: string[];
  label: string;
  mark: RefusedMarks[string] | undefined;
  renderLeaf: PanelProps["renderLeaf"];
  onRemoveItem: PanelProps["onRemoveItem"];
  onSetList: PanelProps["onSetList"];
}) {
  const keys = Object.keys(items).sort((a, b) => Number(a) - Number(b));
  const [draft, setDraft] = useState<string | null>(null);
  const open = () =>
    setDraft(keys.map((key) => (typeof items[key] === "string" ? items[key] : "")).join("\n"));
  return (
    <section className="state-panel-list" aria-label={label}>
      <h5>
        {label}
        {onSetList && draft === null && (
          <button type="button" className="state-panel-edit" onClick={open}>
            {t("statePanelEditList")}
          </button>
        )}
        <RefusedBadge mark={mark} onClick={onSetList && open} />
      </h5>
      {draft !== null && onSetList ? (
        <form
          className="state-panel-list-edit"
          onSubmit={(event) => {
            event.preventDefault();
            onSetList(path, draft.split("\n").map((line) => line.trim()).filter(Boolean));
            setDraft(null);
          }}
        >
          <textarea
            autoFocus
            aria-label={label}
            rows={Math.max(2, keys.length + 1)}
            value={draft}
            onChange={(event) => setDraft(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") setDraft(null);
            }}
          />
          <button type="submit">{t("statePanelSaveList")}</button>
          <button type="button" onClick={() => setDraft(null)}>
            {t("dialogCancel")}
          </button>
        </form>
      ) : (
        <div className="state-panel-chips">
          {keys.map((key) => (
            <div className="state-panel-chip" key={key}>
              {renderLeaf([...path, key], `${label} ${Number(key) + 1}`)}
              <RemoveField
                path={[...path, key]}
                onRemove={onRemoveItem}
                label={`${t("statePanelRemoveItem")} ${items[key]}`}
              />
            </div>
          ))}
          {keys.length === 0 && <span>{t("stateEmptyValue")}</span>}
        </div>
      )}
    </section>
  );
}

/** 預設桌的固定鍵只在顯示時翻譯，所有寫入仍保留原始路徑。 */
export function DefaultStatePanel({
  tree,
  renderLeaf,
  onAdd,
  onRemove,
  onRemoveItem,
  combatEnded = false,
  refused = {},
  onSetList,
  onSetOrder,
  onSetTurn,
}: PanelProps) {
  function list(node: StateNode | undefined, path: string[], label: string) {
    return (
      <PanelList
        key={path.join(".")}
        items={branch(node)}
        path={path}
        label={label}
        mark={refused[path.join(".")]}
        renderLeaf={renderLeaf}
        onRemoveItem={onRemoveItem}
        onSetList={onSetList}
      />
    );
  }
  function custom(node: Record<string, StateNode>, path: string[]) {
    return (
      <>
        {Object.entries(node)
          .filter(([key]) => !RESERVED.includes(key))
          .map(([key]) => (
            <div className="state-panel-custom" key={key}>
              {renderLeaf([...path, key], key)}
              <RemoveField path={[...path, key]} onRemove={onRemove} />
            </div>
          ))}
        <AddField path={path} node={node} onAdd={onAdd} />
      </>
    );
  }
  const tracking = branch(tree.tracking);
  return (
    <div className="state-default-panel">
      <CombatSection
        tree={tree}
        ended={combatEnded}
        refused={refused}
        renderLeaf={renderLeaf}
        renderList={list}
        onSetOrder={onSetOrder}
        onSetTurn={onSetTurn}
      />
      <section aria-label={t("statePanelCharacters")}>
        <h3>{t("statePanelCharacters")}</h3>
        {Object.entries(branch(tree.characters)).map(([name, node]) => {
          const cell = branch(node),
            path = ["characters", name];
          return (
            <section className="state-panel-cell" key={name} aria-label={name}>
              <h4>{name}</h4>
              {renderLeaf([...path, "status"], t("statePanelStatus"))}
              {typeof cell.hp === "string" && renderLeaf([...path, "hp"], t("statePanelHp"))}
              {list(cell.items, [...path, "items"], t("statePanelItems"))}
              {typeof cell.conditions === "object" &&
                list(cell.conditions, [...path, "conditions"], t("statePanelConditions"))}
              {custom(cell, path)}
            </section>
          );
        })}
      </section>
      <section
        className="state-panel-cell"
        aria-label={t("statePanelTracking")}
      >
        <h3>{t("statePanelTracking")}</h3>
        <div className="state-panel-tracking">
          {(["goals", "promises", "clues"] as const).map((key) => (
            <div key={key}>
              {list(tracking[key], ["tracking", key], t(TRACKING_LABELS[key]))}
            </div>
          ))}
        </div>
        {custom(tracking, ["tracking"])}
      </section>
    </div>
  );
}
