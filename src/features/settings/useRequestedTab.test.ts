// @vitest-environment happy-dom

import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { useRequestedTab } from "./useRequestedTab";

type Tab = "appearance" | "versions";
type Props = {
  requested: Tab;
  requestKey: number;
  confirmLeave: () => Promise<boolean>;
  onTab: (tab: Tab, setTab: (tab: Tab) => void) => void;
};

function Harness({ requested, requestKey, confirmLeave, onTab }: Props) {
  const [tab, setTab] = useRequestedTab<Tab>(requested, requestKey, confirmLeave);
  useEffect(() => {
    onTab(tab, setTab);
  });
  return null;
}

describe("useRequestedTab", () => {
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

  async function render(props: Props) {
    if (!root) {
      host = document.createElement("div");
      document.body.appendChild(host);
      root = createRoot(host);
    }
    await act(async () => {
      root?.render(createElement(Harness, props));
    });
  }

  it("an open window switches to a new request only after the leave check passes", async () => {
    const box: { tab: Tab | null } = { tab: null };
    let allow = false;
    let asked = 0;
    const props: Props = {
      requested: "appearance",
      requestKey: 1,
      confirmLeave: async () => {
        asked += 1;
        return allow;
      },
      onTab: (tab) => {
        box.tab = tab;
      },
    };
    await render(props);
    expect(box.tab).toBe("appearance");
    expect(asked).toBe(0);

    await render({ ...props, requested: "versions", requestKey: 2 });
    expect(asked).toBe(1);
    expect(box.tab).toBe("appearance");

    allow = true;
    await render({ ...props, requested: "versions", requestKey: 3 });
    expect(box.tab).toBe("versions");
  });

  it("asking again for the same tab after moving away switches back", async () => {
    const box: { tab: Tab | null; set: ((tab: Tab) => void) | null } = { tab: null, set: null };
    const props: Props = {
      requested: "versions",
      requestKey: 1,
      confirmLeave: async () => true,
      onTab: (tab, setTab) => {
        box.tab = tab;
        box.set = setTab;
      },
    };
    await render(props);
    act(() => {
      box.set?.("appearance");
    });
    expect(box.tab).toBe("appearance");
    await render({ ...props, requestKey: 2 });
    expect(box.tab).toBe("versions");
  });
});
