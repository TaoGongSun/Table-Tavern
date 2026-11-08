import { useState } from "react";
import { Dialog } from "../../shared/ui/Dialog";
import { diceCopy } from "./dice-ui";
import { rulesCopy } from "./rules/rule-books";
import { RuleBookList } from "./rules/RuleBookRow";

export function RulesGuide({
  compact = false,
  entries,
  onAddRulesBook,
  onRemoveEntries,
}: {
  compact?: boolean;
  /** 從桌設定開才有：指南裡的加入鈕與已加入狀態跟選書入口同步 */
  entries?: readonly { uid: number; title: string; content: string }[];
  onAddRulesBook?: (bookJson: string, label: string) => Promise<void>;
  onRemoveEntries?: (uids: number[]) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const c = diceCopy();
  const r = rulesCopy();
  return (
    <>
      <button
        type="button"
        className="btn btn-sm"
        title={c.guide}
        aria-label={c.guide}
        onClick={() => setOpen(true)}
      >
        {compact ? "?" : c.guide}
      </button>
      {open && (
        <Dialog title={c.guide} className="dice-guide" closeButton onDismiss={() => setOpen(false)}>
          <p>{c.intro}</p>
          <p>{c.soloGuide}</p>
          <h3>{r.addable}</h3>
          <RuleBookList entries={entries} onAddRulesBook={onAddRulesBook} onRemoveEntries={onRemoveEntries} />
          <p>{r.others}</p>
          <small className="rule-trademark">{r.trademark}</small>
        </Dialog>
      )}
    </>
  );
}
