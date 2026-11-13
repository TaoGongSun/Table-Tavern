// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
const ipc = vi.hoisted(() =>
  vi.fn(async (command: string) =>
    command === "read_state" ? { dice_enabled: true, table_kind: "chat" } : [],
  ),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke: ipc }));
const ask = vi.hoisted(() => vi.fn(async (_message: string) => true));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: ask }));
import { DiceSettings } from "./DiceSettings";
import { DiceControls } from "./DiceControls";
import { DicePicker } from "./DicePicker";
import { DiceRollText } from "./DiceRollText";
import { diceCopy, playerTurn } from "./dice-ui";
import { rulesCopy } from "./rules/rule-books";
import { RuleBookList } from "./rules/RuleBookRow";
import type { TranscriptEvent } from "../../shared/contracts/backend-contracts";
(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;
let root: Root;
let host: HTMLDivElement;
afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
});
const event = (
  kind: TranscriptEvent["kind"],
  marker?: TranscriptEvent["marker"],
): TranscriptEvent => ({
  kind,
  marker,
  ts: "now",
  speaker_id: "",
  speaker_name: "",
  text: "沃恩擲2d6+3",
});
it("keeps a disabled dice button visible and submits only the chosen formula", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const onRoll = vi.fn(async () => {});
  const render = (room: boolean) =>
    root.render(
      <DiceControls
        diceEnabled
        events={[event("narration")]}
        busy={false}
        locked={false}
        room={room}
        hasTarget
        playerName="沃恩"
        onRoll={onRoll}
      />,
    );
  await act(async () => render(true));
  expect(host.querySelector("button")?.disabled).toBe(true);
  expect(host.querySelector('[role="status"]')?.textContent).toBeTruthy();
  await act(async () => render(false));
  await act(async () => host.querySelector("button")!.click());
  expect(onRoll).toHaveBeenCalledExactlyOnceWith("2d6+3");
  expect(host.querySelector(".dice-options")).toBeNull();
});
it("opens the other suggestions and custom fields from the arrow, or the plain button without suggestions", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const onRoll = vi.fn(async () => {});
  const onExpand = vi.fn();
  const render = (options: string[]) =>
    root.render(<DicePicker copy={diceCopy()} options={options} reason="" onRoll={onRoll} onExpand={onExpand} />);
  await act(async () => render(["1d20+3", "2d6"]));
  await act(async () => host.querySelector<HTMLButtonElement>(".dice-more")!.click());
  expect(onExpand).toHaveBeenCalledOnce();
  const option = host.querySelector<HTMLButtonElement>(".dice-option")!;
  expect(option.textContent).toContain("2d6");
  await act(async () => option.click());
  expect(onRoll).toHaveBeenCalledExactlyOnceWith("2d6");
  await act(async () => render([]));
  expect(host.querySelector(".dice-more")).toBeNull();
  await act(async () => host.querySelector("button")!.click());
  const [custom] = [...host.querySelectorAll<HTMLButtonElement>(".dice-custom button")];
  await act(async () => custom.click());
  expect(onRoll).toHaveBeenLastCalledWith("1d20");
});
it("falls back to the plain button when nothing is left for the player, and ticks rolled options", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const onRoll = vi.fn(async () => {});
  await act(async () =>
    root.render(<DicePicker copy={diceCopy()} options={["2d6", "1d20"]} primary={null} rolled={["2d6"]} reason="" onRoll={onRoll} />),
  );
  expect(host.querySelector(".dice-more")).toBeNull();
  await act(async () => host.querySelector("button")!.click());
  const [first] = [...host.querySelectorAll<HTMLButtonElement>(".dice-option")];
  expect(first.textContent?.startsWith("✓")).toBe(true);
  await act(async () => first.click());
  expect(onRoll).toHaveBeenCalledExactlyOnceWith("2d6");
});
it("bolds the total after the last equals sign", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root.render(<DiceRollText text="沃恩 = 雷克 1d20: [7] = 7" />));
  expect(host.querySelector("strong")?.textContent).toBe("= 7");
  expect(host.textContent).toBe("沃恩 = 雷克 1d20: [7] = 7");
});
it("quiet picker keeps the reason hidden until tapped and never rolls", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const onRoll = vi.fn(async () => {});
  await act(async () =>
    root.render(<DicePicker copy={diceCopy()} options={["2d6"]} reason="等 GM 說完" quiet onRoll={onRoll} />),
  );
  expect(host.querySelector('[role="status"]')).toBeNull();
  await act(async () => host.querySelector("button")!.click());
  expect(host.querySelector('[role="status"]')?.textContent).toBe("等 GM 說完");
  expect(onRoll).not.toHaveBeenCalled();
});
it("only an unresolved explicit NPC call blocks the player turn", () => {
  const call = event("system", { type: "gm_call", name: "雷克" });
  expect(playerTurn([event("narration"), call], "沃恩")).toBe(false);
  expect(
    playerTurn([event("narration"), call, event("dialogue")], "沃恩"),
  ).toBe(true);
  expect(
    playerTurn(
      [event("narration"), event("system", { type: "gm_call", name: "沃恩" })],
      "沃恩",
    ),
  ).toBe(true);
});

it("table settings commits the toggle and refreshes the public notice", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  let enabled = true;
  const onChanged = vi.fn(async () => {
    enabled = false;
    render();
  });
  const render = () => root.render(
      <DiceSettings
        worldId="w"
        diceEnabled={enabled}
        isTurnRunning={() => false}
        onChanged={onChanged}
      />,
  );
  await act(async () => render());
  const checkbox = host.querySelector<HTMLInputElement>(
    'input[type="checkbox"]',
  )!;
  expect(checkbox.checked).toBe(true);
  await act(async () => checkbox.click());
  expect(ipc).toHaveBeenCalledWith("set_dice_enabled", {
    worldId: "w",
    enabled: false,
  });
  expect(checkbox.checked).toBe(false);
  expect(onChanged).toHaveBeenCalledOnce();
  expect(ipc).not.toHaveBeenCalledWith("read_state", expect.anything());
});
it("adds the built-in rules book, then removes only its untouched entries", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const onAdd = vi.fn(async () => {});
  const onRemove = vi.fn(async (_uids: number[]) => true);
  const render = (entries: { uid: number; title: string; content: string }[]) =>
    root.render(
      <DiceSettings
        worldId="w"
        diceEnabled
        entries={entries}
        onAddRulesBook={onAdd}
        onRemoveEntries={onRemove}
        isTurnRunning={() => false}
      />,
    );
  await act(async () => render([]));
  // 沒加書時清單收在小標後面，點開才列
  expect(host.querySelector(".rule-book")).toBeNull();
  await act(async () => host.querySelector<HTMLButtonElement>(".rule-books-switch")!.click());
  const add = [...host.querySelectorAll(".dice-settings .rule-books button")].find((b) => b.textContent === rulesCopy().add)!;
  await act(async () => (add as HTMLButtonElement).click());
  expect(onAdd).toHaveBeenCalledOnce();
  const [json, label] = onAdd.mock.calls[0] as unknown as [string, string];
  expect(label).toBe("5e SRD 5.1");
  const book = JSON.parse(json) as { entries: Record<string, { comment: string; content: string }> };
  const table = Object.values(book.entries).map((e, uid) => ({ uid, title: e.comment, content: e.content }));
  table[0] = { ...table[0], content: "沃恩改過的檢定規則" };
  table.push({ uid: 99, title: "沃恩的酒館", content: "雷克常來喝酒。" });
  await act(async () => render(table));
  const remove = [...host.querySelectorAll(".dice-settings > .rule-books button")].find((b) => b.textContent === rulesCopy().remove)!;
  await act(async () => (remove as HTMLButtonElement).click());
  await vi.waitFor(() => expect(onRemove).toHaveBeenCalledOnce());
  expect(ask.mock.calls[0][0]).toContain("50");
  const removed = onRemove.mock.calls[0][0];
  expect(removed).toHaveLength(50);
  expect(removed).not.toContain(0);
  expect(removed).not.toContain(99);
});
it("points to table settings when the guide cannot add books", async () => {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root.render(<RuleBookList catalog />));
  const button = [...host.querySelectorAll("button")].find((b) => b.textContent === rulesCopy().inSettings)!;
  expect(button.disabled).toBe(true);
  expect(host.querySelector(".rule-book-details")).toBeNull();
});
