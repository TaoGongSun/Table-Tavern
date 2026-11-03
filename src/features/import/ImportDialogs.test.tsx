// @vitest-environment happy-dom
// 貼出開場白排在進行中的回合後面：貼出鈕就地換成等待提示、兩顆貼出鈕都停用，不會重複送出。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { ImportDialogs } from "./ImportDialogs";
import type { ComponentProps } from "react";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root | null = null;
let host: HTMLElement | null = null;

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
});

function render(
  postBusy: boolean,
  postWaiting: boolean,
  onPostOpening = vi.fn(),
  extra: Partial<ComponentProps<typeof ImportDialogs>> = {},
) {
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  act(() =>
    root?.render(
      <ImportDialogs
        busy={false}
        choice={null}
        onAnswerChoice={() => {}}
        route={null}
        onAnswerRoute={() => {}}
        openings={["甲的開場"]}
        expanded={0}
        translationState={{}}
        translations={{}}
        translateAllBusy={false}
        tier="fast"
        onSetTier={() => {}}
        tierModels={[]}
        onSetExpanded={() => {}}
        onCloseOpenings={() => {}}
        onTranslateAll={() => {}}
        onPostOpening={onPostOpening}
        postBusy={postBusy}
        postWaiting={postWaiting}
        onTranslateAndPost={() => {}}
        onRetranslate={() => {}}
        {...extra}
      />,
    ),
  );
}

function postButton() {
  return [...document.querySelectorAll<HTMLButtonElement>("button.btn-primary")].find((button) =>
    button.textContent?.includes(t("openingLineOk")),
  )!;
}

const shownLabel = (button: HTMLButtonElement) =>
  button.querySelector('.dialog-swap > span:not([aria-hidden="true"])')?.textContent;

describe("開場白貼出鈕", () => {
  it("排在回合後面：就地顯示等待提示，貼出與翻譯後貼出都停用", () => {
    render(true, true);
    const button = postButton();
    expect(shownLabel(button)).toBe(t("turnQueuedWait"));
    expect(button.disabled).toBe(true);
    const translateAndPost = [...document.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
      b.textContent?.includes(t("openingTranslatePostBtn")),
    )!;
    expect(translateAndPost.disabled).toBe(true);
  });

  it("平常：顯示貼出、可以按", () => {
    const onPostOpening = vi.fn();
    render(false, false, onPostOpening);
    const button = postButton();
    expect(shownLabel(button)).toBe(t("openingLineOk"));
    act(() => button.click());
    expect(onPostOpening).toHaveBeenCalledWith("甲的開場", 0, false);
  });
});

describe("第二張卡路由框", () => {
  it("開新桌並匯入分聊天桌與 VN 桌兩顆", () => {
    const onAnswerRoute = vi.fn();
    render(false, false, vi.fn(), {
      openings: null,
      route: { worldId: "W", data: [1], identity: "character", label: "莉亞", route: "ask" },
      onAnswerRoute,
    });
    const button = (key: "importRouteNewVnTable" | "importRouteNewTable") =>
      [...document.querySelectorAll<HTMLButtonElement>("button")].find(
        (b) => b.textContent === t(key),
      )!;
    act(() => button("importRouteNewVnTable").click());
    act(() => button("importRouteNewTable").click());
    expect(onAnswerRoute.mock.calls).toEqual([["new_vn_table"], ["new_table"]]);
  });
});
