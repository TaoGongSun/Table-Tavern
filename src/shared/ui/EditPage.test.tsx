// @vitest-environment happy-dom

import { act, type FormEvent, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { EditPage } from "./EditPage";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("EditPage", () => {
  let root: Root | null = null;
  let host: HTMLDivElement | null = null;

  afterEach(() => {
    act(() => {
      root?.unmount();
    });
    host?.remove();
    root = null;
    host = null;
  });

  function mount(node: ReactNode) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root?.render(node));
  }

  const moreButton = () => host!.querySelector(`[aria-label="${t("moreActions")}"]`);
  const saveButton = () => host!.querySelector<HTMLButtonElement>(".edit-page-save")!;

  it("does not render the more menu without items, nor the status area when empty", () => {
    mount(<EditPage title="T" onBack={() => {}} formId="f" />);
    expect(moreButton()).toBeNull();
    expect(host!.querySelector(".edit-page-status")).toBeNull();
  });

  it("renders the more menu when it has items", () => {
    mount(
      <EditPage
        title="T"
        onBack={() => {}}
        formId="f"
        moreItems={[{ key: "a", label: "A", onSelect: () => {} }]}
      />,
    );
    expect(moreButton()).not.toBeNull();
  });

  it("submits the external form through the form attribute", () => {
    const onSubmit = vi.fn((event: FormEvent) => event.preventDefault());
    mount(
      <EditPage title="T" onBack={() => {}} formId="outer-form">
        <form id="outer-form" onSubmit={onSubmit}>
          <input />
        </form>
      </EditPage>,
    );
    expect(saveButton().getAttribute("form")).toBe("outer-form");
    // 頂列在 form 外：靠 form 屬性才送得到
    expect(saveButton().closest("form")).toBeNull();
    act(() => saveButton().click());
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("keeps save disabled but present while loading, back stays usable, errors are alerts", () => {
    const onBack = vi.fn();
    mount(<EditPage title="T" onBack={onBack} message="boom" messageIsError />);
    expect(saveButton().disabled).toBe(true);
    expect(host!.querySelector('[role="alert"]')!.textContent).toBe("boom");
    act(() => host!.querySelector<HTMLButtonElement>(".edit-page-back")!.click());
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
