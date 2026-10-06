// @vitest-environment happy-dom
// 生圖模型下拉：預設、清單值、清單外舊值顯示成自訂且原值不變、清單晚到不動自訂值。

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_IMAGE_MODEL, type ModelOption } from "../ai-connection/model-catalog";
import { ImageModelField } from "./ImageModelField";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const LIST: ModelOption[] = [
  { id: "google/gemini-3.1-flash-image", label: "Gemini Flash Image" },
  { id: "openai/gpt-image-2", label: "GPT Image 2" },
];

let root: Root | null = null;
let host: HTMLDivElement | null = null;

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
});

function Harness({ initial, options, onChange }: { initial: string; options: ModelOption[]; onChange: (v: string) => void }) {
  const [value, setValue] = useState(initial);
  return (
    <ImageModelField
      value={value}
      options={options}
      onChange={(next) => {
        setValue(next);
        onChange(next);
      }}
    />
  );
}

function mount(initial: string, options: ModelOption[], onChange = vi.fn()) {
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  const render = (list: ModelOption[]) =>
    act(() => root!.render(<Harness initial={initial} options={list} onChange={onChange} />));
  render(options);
  return {
    render,
    select: () => host!.querySelector("select")!,
    input: () => host!.querySelector("input"),
    onChange,
  };
}

function choose(select: HTMLSelectElement, value: string) {
  act(() => {
    select.value = value;
    select.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

function type(input: HTMLInputElement, value: string) {
  act(() => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
    setter.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

describe("ImageModelField", () => {
  it("空值選到預設，沒有自訂框", () => {
    const field = mount("", LIST);
    expect(field.select().value).toBe("");
    expect(field.select().options[0].textContent).toContain(DEFAULT_IMAGE_MODEL);
    expect(field.input()).toBeNull();
  });

  it("清單內的值直接選中；換選項回報新值", () => {
    const field = mount("openai/gpt-image-2", LIST);
    expect(field.select().value).toBe("openai/gpt-image-2");
    choose(field.select(), "google/gemini-3.1-flash-image");
    expect(field.onChange).toHaveBeenLastCalledWith("google/gemini-3.1-flash-image");
  });

  it("清單外的舊值顯示成自訂並帶原值，不回報變更", () => {
    const field = mount("vendor/old-image-model", LIST);
    expect(field.select().value).toBe("__custom__");
    expect(field.input()!.value).toBe("vendor/old-image-model");
    expect(field.onChange).not.toHaveBeenCalled();
  });

  it("清單晚到：已選自訂並輸入的值不變", () => {
    const field = mount("", []);
    choose(field.select(), "__custom__");
    type(field.input()!, "openai/gpt-image-2");
    field.render(LIST);
    expect(field.select().value).toBe("__custom__");
    expect(field.input()!.value).toBe("openai/gpt-image-2");
  });

  it("清單晚到：沒抓到清單時顯示成自訂的舊值，清單回來後值不變", () => {
    const field = mount("openai/gpt-image-2", []);
    expect(field.input()!.value).toBe("openai/gpt-image-2");
    field.render(LIST);
    expect(field.select().value).toBe("openai/gpt-image-2");
    expect(field.onChange).not.toHaveBeenCalled();
  });

  it("選了清單裡的模型後，背景更新把它移除：顯示成自訂並帶原值", () => {
    const field = mount("", LIST);
    choose(field.select(), "openai/gpt-image-2");
    field.render([LIST[0]]);
    expect(field.select().value).toBe("__custom__");
    expect(field.input()!.value).toBe("openai/gpt-image-2");
  });

  it("前端預設模型與 Rust 的 DEFAULT_IMAGE_MODEL 同值", () => {
    const rust = readFileSync(join(process.cwd(), "src-tauri/src/transport/client.rs"), "utf8");
    expect(rust).toContain(`pub const DEFAULT_IMAGE_MODEL: &str = "${DEFAULT_IMAGE_MODEL}";`);
  });
});
