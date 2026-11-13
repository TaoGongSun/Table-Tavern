import { useRef, useState } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../../../i18n";
import { RULE_BOOKS, bookAdded, bookEntries, rulesCopy, type RuleBook } from "./rule-books";

type ListedEntry = { uid: number; title: string; content: string };

const plainName = (book: RuleBook) => rulesCopy()[book.id].name;

/** 一本內建書：白話名＋系統代稱＋右側鈕，按 ⓘ 才展開說明與署名。桌設定與指南共用。 */
export function RuleBookRow({
  book,
  added = false,
  otherAdded = false,
  onAdd,
  onRemove,
  onSwitch,
}: {
  book: RuleBook;
  added?: boolean;
  /** 這桌已經有另一本內建書 */
  otherAdded?: boolean;
  onAdd?: () => void;
  onRemove?: () => void;
  onSwitch?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const c = rulesCopy();
  const text = c[book.id];
  const button = added && onRemove
    ? { label: c.remove, disabled: false, run: onRemove }
    : otherAdded && onSwitch
      ? { label: c.switch, disabled: false, run: onSwitch }
      : { label: added ? c.added : onAdd ? c.add : c.inSettings, disabled: added || !onAdd, run: onAdd };
  return (
    <li className="rule-book">
      <div className="rule-book-head">
        <button type="button" className="rule-book-name" aria-expanded={open} title={c.more} onClick={() => setOpen(!open)}>
          {text.name} <small>{book.name}</small> <span aria-hidden="true">ⓘ</span>
        </button>
        <button type="button" className="btn btn-sm" disabled={button.disabled} onClick={button.run}>
          {button.label}
        </button>
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

type Remove = (uids: number[]) => Promise<boolean>;

/** 確認後只刪標題＋內文都沒動過的條目；改過的留下並在確認框講明。 */
async function removeBook(book: RuleBook, entries: readonly ListedEntry[], remove: Remove) {
  const c = rulesCopy();
  const { owned, kept } = bookEntries(book, entries);
  const text = c.removeConfirm.replace("{name}", plainName(book)).replace("{n}", String(owned.length));
  const message = kept > 0 ? `${text}\n${c.removeKept.replace("{kept}", String(kept))}` : text;
  const accepted = await confirm(message, { title: c.removeTitle, kind: "warning", okLabel: c.remove, cancelLabel: t("dialogCancel") });
  if (accepted && owned.length > 0) await remove(owned);
}

/** 一次確認：講明舊書刪幾條，有改過的才補一句會留下。先刪沒改過的，刪不掉就不加新書（免得兩套並存）。回傳有沒有換。 */
async function replaceBook(
  current: RuleBook,
  next: RuleBook,
  entries: readonly ListedEntry[],
  remove: Remove,
  add: (bookJson: string, label: string) => Promise<void>,
) {
  const c = rulesCopy();
  const { owned, kept } = bookEntries(current, entries);
  const text = c.switchConfirm
    .replace("{old}", plainName(current))
    .replace("{next}", plainName(next))
    .replace("{n}", String(owned.length));
  const message = kept > 0 ? `${text}\n${c.removeKept.replace("{kept}", String(kept))}` : text;
  const accepted = await confirm(message, { title: c.switchOpen, kind: "warning", okLabel: c.switch, cancelLabel: t("dialogCancel") });
  if (!accepted || (owned.length > 0 && !(await remove(owned)))) return false;
  await add(next.json, next.name);
  return true;
}

/** 桌設定：加了的那本常駐，其餘收在一顆展開鈕後面（沒加書時鈕就是小標），免得五本把世界設定擠到畫面外。
 *  指南（catalog）永遠全列。同時間只跑一個加入／移除／換書。 */
export function RuleBookList({
  entries = [],
  onAddRulesBook,
  onRemoveEntries,
  catalog = false,
}: {
  entries?: readonly ListedEntry[];
  onAddRulesBook?: (bookJson: string, label: string) => Promise<void>;
  onRemoveEntries?: Remove;
  catalog?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const busy = useRef(false);
  const once = (run: () => Promise<unknown>) => () => {
    if (busy.current) return;
    busy.current = true;
    void run().finally(() => (busy.current = false));
  };
  const c = rulesCopy();
  const current = RULE_BOOKS.find((book) => bookAdded(book, entries));
  const row = (book: RuleBook) => (
    <RuleBookRow
      key={book.id}
      book={book}
      added={book === current}
      otherAdded={Boolean(current && book !== current)}
      onAdd={onAddRulesBook && once(() => onAddRulesBook(book.json, book.name).then(() => setOpen(false)))}
      onRemove={onRemoveEntries && book === current ? once(() => removeBook(book, entries, onRemoveEntries)) : undefined}
      onSwitch={
        onAddRulesBook && onRemoveEntries && current && book !== current
          ? once(async () => (await replaceBook(current, book, entries, onRemoveEntries, onAddRulesBook)) && setOpen(false))
          : undefined
      }
    />
  );
  if (catalog) return <ul className="rule-books">{RULE_BOOKS.map(row)}</ul>;
  return (
    <>
      {current && <ul className="rule-books">{row(current)}</ul>}
      <div className="rule-books-more">
        <button type="button" className="rule-books-switch" aria-expanded={open} onClick={() => setOpen(!open)}>
          <span aria-hidden="true">{open ? "▾" : "▸"}</span> {current ? c.switchOpen : c.heading}
        </button>
        {open && <ul className="rule-books">{RULE_BOOKS.filter((book) => book !== current).map(row)}</ul>}
      </div>
    </>
  );
}
