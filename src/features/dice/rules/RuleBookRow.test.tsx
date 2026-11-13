// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";

const ask = vi.hoisted(() => vi.fn(async (_message: string) => true));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: ask }));

import { RULE_BOOKS, bookEntries, rulesCopy } from "./rule-books";
import { RuleBookList } from "./RuleBookRow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let host: HTMLDivElement;
afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  ask.mockReset();
  ask.mockResolvedValue(true);
});

it("confirms once, then removes untouched entries before adding the other book", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const [current, next] = RULE_BOOKS;
  const parsed = JSON.parse(current.json) as { entries: Record<string, { comment: string; content: string }> };
  const entries = Object.values(parsed.entries).map((entry, uid) => ({ uid, title: entry.comment, content: entry.content }));
  entries[0] = { ...entries[0], content: "沃恩改過的檢定規則" };
  const order: string[] = [];
  let removable = false;
  const onRemove = vi.fn(async (_uids: number[]) => {
    if (removable) order.push("remove");
    return removable;
  });
  const onAdd = vi.fn(async (_json: string, _label: string) => {
    order.push("add");
  });
  const render = () =>
    root.render(<RuleBookList entries={entries} onAddRulesBook={onAdd} onRemoveEntries={onRemove} />);
  await act(async () => render());
  const c = rulesCopy();
  const open = [...host.querySelectorAll("button")].find((button) => button.textContent?.endsWith(c.switchOpen))!;
  await act(async () => (open as HTMLButtonElement).click());
  const swap = [...host.querySelectorAll("button")].find((button) => button.textContent === c.switch)!;
  ask.mockResolvedValueOnce(false);
  await act(async () => (swap as HTMLButtonElement).click());
  expect(onRemove).not.toHaveBeenCalled();
  expect(onAdd).not.toHaveBeenCalled();
  // 舊書刪不掉就不加新書，免得兩套並存
  await act(async () => (swap as HTMLButtonElement).click());
  await vi.waitFor(() => expect(onRemove).toHaveBeenCalledOnce());
  expect(onAdd).not.toHaveBeenCalled();
  onRemove.mockClear();
  removable = true;
  await act(async () => (swap as HTMLButtonElement).click());
  await vi.waitFor(() => expect(onAdd).toHaveBeenCalledOnce());
  const { owned, kept } = bookEntries(current, entries);
  expect(order).toEqual(owned.length > 0 ? ["remove", "add"] : ["add"]);
  expect(onRemove.mock.calls[0][0]).toEqual(owned);
  const message = String(ask.mock.calls[ask.mock.calls.length - 1][0]);
  expect(message).toContain(String(owned.length));
  expect(message).toContain(c.removeKept.replace("{kept}", String(kept)));
  expect(onAdd.mock.calls[0][1]).toBe(next.name);
});
