// @vitest-environment happy-dom

import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../../i18n";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { GenerateTableDialog } from "./GenerateTableDialog";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const invokeMock = vi.mocked(invoke);

const OUTLINE = {
  title: "Harbor",
  world: "Fog over the docks.",
  characters: [
    { name: "Ana", tagline: "dock guard" },
    { name: "Bo", tagline: "thief" },
  ],
};

describe("GenerateTableDialog", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;
  const onCreated = vi.fn(async () => {});
  const onClose = vi.fn();
  const renderErrors: unknown[] = [];

  beforeEach(() => {
    invokeMock.mockReset();
    onCreated.mockClear();
    onClose.mockClear();
    renderErrors.length = 0;
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
    document.body.innerHTML = "";
  });

  // 照 main.tsx 的實際入口包 StrictMode
  function mount(room = false) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host, { onUncaughtError: (error) => renderErrors.push(error) });
    act(() => {
      root!.render(
        <StrictMode>
          <GenerateTableDialog open room={room} onClose={onClose} onCreated={onCreated} transport="api" />
        </StrictMode>,
      );
    });
  }

  function setValue(element: HTMLInputElement | HTMLTextAreaElement, value: string) {
    const proto = element instanceof HTMLTextAreaElement ? HTMLTextAreaElement : HTMLInputElement;
    Object.getOwnPropertyDescriptor(proto.prototype, "value")!.set!.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }

  /** 逐鍵輸入：同一批連續送多個 input 事件 */
  function typeKeys(element: HTMLInputElement | HTMLTextAreaElement, text: string) {
    act(() => {
      let value = element.value;
      for (const char of text) {
        value += char;
        setValue(element, value);
      }
    });
  }

  const dialog = () => document.querySelector<HTMLElement>(".gen-table-dialog");
  const titleInput = () => document.querySelector<HTMLInputElement>(".gen-outline-preview > input")!;
  const worldInput = () =>
    document.querySelector<HTMLTextAreaElement>(".gen-outline-preview > textarea")!;
  const nameInputs = () => [...document.querySelectorAll<HTMLInputElement>(".gen-character-name")];
  const taglineInputs = () =>
    [...document.querySelectorAll<HTMLTextAreaElement>(".gen-character-row textarea")];
  const alertSection = () => document.querySelector<HTMLElement>(".gen-result-error");

  async function generateOutline(outcome: () => Promise<unknown>) {
    invokeMock.mockImplementation(async (command) => {
      if (command === "generate_table_outline") return outcome();
      if (command === "generate_table_expand") return { worldId: "w-new", raw: "" };
      throw new Error(`unexpected ${command}`);
    });
    const prompt = document.querySelector<HTMLTextAreaElement>("textarea[data-autofocus]")!;
    act(() => setValue(prompt, "a foggy harbor"));
    const generate = document.querySelector<HTMLButtonElement>(".gen-generate-row button")!;
    await act(async () => generate.click());
  }

  it("keeps the app alive and updates exactly the edited field while typing in the outline", async () => {
    mount();
    await generateOutline(async () => ({ parsed: OUTLINE, raw: "raw" }));
    expect(titleInput().value).toBe("Harbor");

    typeKeys(titleInput(), "XY");
    typeKeys(worldInput(), "!!");
    typeKeys(nameInputs()[1], "b");
    typeKeys(taglineInputs()[0], "?");

    expect(renderErrors).toEqual([]);
    expect(dialog()).not.toBeNull();
    expect(titleInput().value).toBe("HarborXY");
    expect(worldInput().value).toBe("Fog over the docks.!!");
    expect(nameInputs().map((input) => input.value)).toEqual(["Ana", "Bob"]);
    expect(taglineInputs().map((input) => input.value)).toEqual(["dock guard?", "thief"]);
  });

  // 聊天桌一句話與題材都空白不能生成（VN 精靈才收空白，見 features/vn/wizard）
  it("keeps generate disabled while the idea and genres are both empty", () => {
    mount();
    const generate = document.querySelector<HTMLButtonElement>(".gen-generate-row button")!;
    expect(generate.disabled).toBe(true);
    const prompt = document.querySelector<HTMLTextAreaElement>("textarea[data-autofocus]")!;
    act(() => setValue(prompt, "a foggy harbor"));
    expect(generate.disabled).toBe(false);
  });

  it("sends the edited draft title when creating the table", async () => {
    mount();
    await generateOutline(async () => ({ parsed: OUTLINE, raw: "raw" }));
    act(() => setValue(titleInput(), "  Glass Harbor  "));
    const create = document.querySelector<HTMLButtonElement>(".dialog .btn-primary")!;
    await act(async () => create.click());

    expect(invokeMock).toHaveBeenCalledWith(
      "generate_table_expand",
      expect.objectContaining({ title: "Glass Harbor", tableKind: "chat" }),
    );
    expect(onCreated).toHaveBeenCalledWith("w-new");
  });

  it("explains an API failure in plain words without leaking the user id", async () => {
    mount();
    const raw =
      'AI_HTTP_STATUS_429: status=429 Too Many Requests body={"error":{"message":"upstream rate-limited","code":429},"user_id":"user_secret123"}';
    await generateOutline(async () => Promise.reject(raw));

    const section = alertSection()!;
    expect(section.getAttribute("role")).toBe("alert");
    expect(section.querySelectorAll('[role="alert"]')).toHaveLength(0);
    expect(section.textContent).toContain(t("errQuotaApi"));
    expect(section.textContent).not.toContain("user_secret123");
    expect(section.textContent!.split("upstream rate-limited")).toHaveLength(2);
    expect(section.querySelector("pre")).toBeNull();
  });

  it("still shows the model's raw output when the outline cannot be parsed", async () => {
    mount();
    await generateOutline(async () => ({ parsed: null, raw: "not a marked outline" }));

    const section = alertSection()!;
    expect(section.textContent).toContain(t("genParseFail"));
    expect(section.querySelector("pre")!.textContent).toBe("not a marked outline");
  });

  it("room mode offers who builds characters and sends that choice", async () => {
    mount(true);
    const own = document.querySelector<HTMLInputElement>('input[name="room-cast"][value="own"]')!;
    const made = document.querySelector<HTMLInputElement>('input[name="room-cast"][value="made"]')!;
    expect(own.checked).toBe(true);
    expect(document.body.textContent).toContain(t("genRoomCastOwnNote"));
    expect(document.querySelector(".gen-table-dialog")!.textContent).toContain(t("genRoomTitle"));

    await generateOutline(async () => ({ parsed: { title: "Harbor", world: "Fog.", characters: [] }, raw: "raw" }));
    expect(invokeMock).toHaveBeenCalledWith(
      "generate_table_outline",
      expect.objectContaining({ room: { cast: false } }),
    );
    expect(document.querySelector(".gen-character-list")).toBeNull();
    expect(document.body.textContent).not.toContain(t("genAddCharacterAI"));
    const create = () => document.querySelector<HTMLButtonElement>(".dialog .btn-primary")!;
    expect(create().textContent).toContain(t("genRoomCreateBtn"));
    await act(async () => create().click());
    expect(invokeMock).toHaveBeenCalledWith(
      "generate_table_expand",
      expect.objectContaining({ room: { cast: false }, tableKind: "chat" }),
    );

    act(() => made.click());
    await generateOutline(async () => ({ parsed: OUTLINE, raw: "raw" }));
    expect(invokeMock).toHaveBeenCalledWith(
      "generate_table_outline",
      expect.objectContaining({ room: { cast: true } }),
    );
    expect(document.querySelector(".gen-character-list")).not.toBeNull();
    await act(async () => create().click());
    expect(invokeMock).toHaveBeenCalledWith(
      "generate_table_expand",
      expect.objectContaining({ room: { cast: true } }),
    );
  });

  it("drops a chat outline when the dialog is closed and reopened for a room", async () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host, { onUncaughtError: (error) => renderErrors.push(error) });
    const show = (open: boolean, room: boolean) => {
      act(() => {
        root!.render(
          <StrictMode>
            <GenerateTableDialog open={open} room={room} onClose={onClose} onCreated={onCreated} transport="api" />
          </StrictMode>,
        );
      });
    };
    show(true, false);
    await generateOutline(async () => ({ parsed: OUTLINE, raw: "raw" }));
    show(false, false);
    show(true, true);
    expect(document.querySelector(".dialog-foot-end button")).toBeNull();
    expect(document.querySelector(".gen-outline-preview")).toBeNull();
    expect(document.querySelector<HTMLTextAreaElement>("textarea[data-autofocus]")!.value).toBe("a foggy harbor");
  });

  it("chat mode has no cast choice and sends room null", async () => {
    mount();
    expect(document.querySelector('input[name="room-cast"]')).toBeNull();
    await generateOutline(async () => ({ parsed: OUTLINE, raw: "raw" }));
    expect(invokeMock).toHaveBeenCalledWith(
      "generate_table_outline",
      expect.objectContaining({ room: null }),
    );
  });
});
