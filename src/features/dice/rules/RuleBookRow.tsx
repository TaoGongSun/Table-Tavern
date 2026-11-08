import { useState } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../../../i18n";
import { RULE_BOOKS, bookAdded, bookEntries, rulesCopy, type RuleBook } from "./rule-books";

/** 一本內建書：名稱＋副標＋加入／移除鈕，按 ⓘ 才展開三句說明與署名。桌設定與指南共用；
 *  沒有 onAdd（指南開在不能加的地方）就停用鈕並指路到桌設定。 */
export function RuleBookRow({
  book,
  added = false,
  onAdd,
  onRemove,
}: {
  book: RuleBook;
  added?: boolean;
  onAdd?: () => void;
  onRemove?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const c = rulesCopy();
  const text = c[book.id];
  return (
    <li className="rule-book">
      <div className="rule-book-head">
        <button
          type="button"
          className="rule-book-name"
          aria-expanded={open}
          title={c.more}
          onClick={() => setOpen(!open)}
        >
          {book.name} <span aria-hidden="true">ⓘ</span>
        </button>
        {added && onRemove ? (
          <button type="button" className="btn btn-sm" onClick={onRemove}>
            {c.remove}
          </button>
        ) : (
          <button type="button" className="btn btn-sm" disabled={added || !onAdd} onClick={onAdd}>
            {added ? c.added : onAdd ? c.add : c.inSettings}
          </button>
        )}
      </div>
      <p className="rule-book-sub">{text.subtitle}</p>
      {open && (
        <>
          <p className="rule-book-details">{text.details}</p>
          <small className="rule-book-credit">{book.credit}</small>
        </>
      )}
    </li>
  );
}

type ListedEntry = { uid: number; title: string; content: string };

/** 確認後只刪標題＋內文都沒動過的條目；改過的留下並在確認框講明。 */
async function removeBook(book: RuleBook, entries: readonly ListedEntry[], remove: (uids: number[]) => Promise<void>) {
  const c = rulesCopy();
  const { owned, kept } = bookEntries(book, entries);
  const text = c.removeConfirm.replace("{name}", book.name).replace("{n}", String(owned.length));
  const accepted = await confirm(kept > 0 ? `${text}\n${c.removeKept.replace("{kept}", String(kept))}` : text, {
    title: c.removeTitle,
    kind: "warning",
    okLabel: c.remove,
    cancelLabel: t("dialogCancel"),
  });
  if (accepted && owned.length > 0) await remove(owned);
}

/** 全部內建書：「已加入」看這桌世界書，onAddRulesBook 沒給就只能指路到桌設定（側欄開的指南）。 */
export function RuleBookList({
  entries = [],
  onAddRulesBook,
  onRemoveEntries,
}: {
  entries?: readonly ListedEntry[];
  onAddRulesBook?: (bookJson: string, label: string) => Promise<void>;
  onRemoveEntries?: (uids: number[]) => Promise<void>;
}) {
  return (
    <ul className="rule-books">
      {RULE_BOOKS.map((book) => (
        <RuleBookRow
          key={book.id}
          book={book}
          added={bookAdded(book, entries)}
          onAdd={onAddRulesBook && (() => void onAddRulesBook(book.json, book.name))}
          onRemove={onRemoveEntries && (() => void removeBook(book, entries, onRemoveEntries))}
        />
      ))}
    </ul>
  );
}
