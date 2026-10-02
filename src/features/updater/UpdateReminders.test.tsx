// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FormatUpdateDialog, UpdateBanner } from "./UpdateReminders";
import type { UpdateController, UpdateOffer } from "./useUpdateController";

const offer = (level: UpdateOffer["level"]): UpdateOffer => ({
  version: "0.3.0",
  current_version: "0.2.0",
  notes: null,
  pub_date: null,
  level,
  skipped: false,
});

function controller(kind: "banner" | "dialog" | null): UpdateController {
  return {
    reminder: kind ? { kind, offer: offer(kind === "dialog" ? "format" : "feature") } : null,
    markReminderShown: vi.fn(),
    dismissReminder: vi.fn(),
  } as unknown as UpdateController;
}

describe("update reminders", () => {
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

  function mount(node: React.ReactNode) {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => {
      root?.render(node);
    });
  }

  it("records the reminder only once it is drawn", () => {
    const update = controller("banner");
    mount(<UpdateBanner update={update} preferences={{}} onView={() => {}} />);
    expect(host?.textContent).toContain("0.3.0");
    expect(update.markReminderShown).toHaveBeenCalledWith("0.3.0");
  });

  it("a skipped version draws nothing and records nothing", () => {
    const update = controller("banner");
    mount(
      <UpdateBanner
        update={update}
        preferences={{ update_skipped_version: "0.3.0" }}
        onView={() => {}}
      />,
    );
    expect(host?.textContent).toBe("");
    expect(update.markReminderShown).not.toHaveBeenCalled();
  });

  it("the banner and the dialog each draw only their own kind", () => {
    const update = controller("dialog");
    mount(
      <>
        <UpdateBanner update={update} preferences={{}} onView={() => {}} />
        <FormatUpdateDialog update={update} preferences={{}} onView={() => {}} />
      </>,
    );
    expect(host?.querySelector(".update-banner")).toBeNull();
    expect(host?.querySelector("dialog")).not.toBeNull();
    expect(update.markReminderShown).toHaveBeenCalledTimes(1);
  });

  it("view opens the versions tab and dismisses", () => {
    const update = controller("banner");
    const view = vi.fn();
    mount(<UpdateBanner update={update} preferences={{}} onView={view} />);
    const button = [...(host?.querySelectorAll("button") ?? [])].find(
      (item) => item.textContent === "查看",
    );
    act(() => {
      button?.click();
    });
    expect(view).toHaveBeenCalled();
    expect(update.dismissReminder).toHaveBeenCalled();
  });
});
