import { type ReactNode, useState } from "react";
import { t } from "../../i18n";
import { type StateNode } from "../../shared/contracts/backend-contracts";

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
];
const branch = (node: StateNode | undefined): Record<string, StateNode> =>
  typeof node === "object" && node !== null ? node : {};

type PanelProps = {
  tree: Record<string, StateNode>;
  renderLeaf: (path: string[], label: string) => ReactNode;
  onAdd?: (parentPath: string[], name: string) => Promise<void>;
  onRemove?: (path: string[]) => Promise<void>;
  onRemoveItem?: (path: string[]) => Promise<void>;
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

/** 預設桌的固定鍵只在顯示時翻譯，所有寫入仍保留原始路徑。 */
export function DefaultStatePanel({
  tree,
  renderLeaf,
  onAdd,
  onRemove,
  onRemoveItem,
}: PanelProps) {
  function list(node: StateNode | undefined, path: string[], label: string) {
    const items = branch(node);
    return (
      <section className="state-panel-list" aria-label={label}>
        <h5>{label}</h5>
        <div className="state-panel-chips">
          {Object.keys(items)
            .sort((a, b) => Number(a) - Number(b))
            .map((key) => (
              <div className="state-panel-chip" key={key}>
                {renderLeaf([...path, key], `${label} ${Number(key) + 1}`)}
                <RemoveField
                  path={[...path, key]}
                  onRemove={onRemoveItem}
                  label={`${t("statePanelRemoveItem")} ${items[key]}`}
                />
              </div>
            ))}
          {Object.keys(items).length === 0 && (
            <span>{t("stateEmptyValue")}</span>
          )}
        </div>
      </section>
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
      <section aria-label={t("statePanelCharacters")}>
        <h3>{t("statePanelCharacters")}</h3>
        {Object.entries(branch(tree.characters)).map(([name, node]) => {
          const cell = branch(node),
            path = ["characters", name];
          return (
            <section className="state-panel-cell" key={name} aria-label={name}>
              <h4>{name}</h4>
              {renderLeaf([...path, "status"], t("statePanelStatus"))}
              {list(cell.items, [...path, "items"], t("statePanelItems"))}
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
