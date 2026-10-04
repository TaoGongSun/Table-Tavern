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
  function mount() {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host, { onUncaughtError: (error) => renderErrors.push(error) });
    act(() => {
      root!.render(
        <StrictMode>
          <GenerateTableDialog open onClose={onClose} onCreated={onCreated} transport="api" />
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

  it("sends the edited draft title when creating the table", async () => {
    mount();
    await generateOutline(async () => ({ parsed: OUTLINE, raw: "raw" }));
    act(() => setValue(titleInput(), "  Glass Harbor  "));
    const create = document.querySelector<HTMLButtonElement>(".dialog .btn-primary")!;
    await act(async () => create.click());

    expect(invokeMock).toHaveBeenCalledWith(
      "generate_table_expand",
      expect.objectContaining({ title: "Glass Harbor" }),
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
});
