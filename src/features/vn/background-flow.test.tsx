// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { t } from "../../i18n";
import { StageBackdrop } from "./StageBackdrop";
import { ChapterCardView } from "./ChapterCardView";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("shows a location gradient and an explicit retry that does not advance the story", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  const retry = vi.fn(); const advance = vi.fn();
  await act(async () => root.render(<div onClick={advance}><StageBackdrop image={null} period="night" weather="clear" location="Station" state="failed" onRetry={retry} /></div>));
  expect(host.querySelector<HTMLElement>(".vn-backdrop-placeholder")?.style.background).toContain("linear-gradient");
  expect(host.querySelector(".vn-location")?.textContent).toBe("Station");
  expect(retry).not.toHaveBeenCalled();
  await act(async () => host.querySelector("button")!.click());
  expect(retry).toHaveBeenCalledOnce(); expect(advance).not.toHaveBeenCalled();
  await act(async () => root.render(<StageBackdrop image={{ src: "data:old", period: "day", rev: 1 }} period="day" weather="clear" location="Station" />));
  await act(async () => root.render(<StageBackdrop image={null} period="night" weather="clear" location="Station" state="paused" onRetry={retry} />));
  expect(host.querySelector(".vn-backdrop-waiting")).not.toBeNull();
  expect(host.querySelector(".vn-location")).toBeNull();
  expect(host.querySelector("button")?.textContent).toBe(t("vnBackgroundQuota"));
  await act(async () => root.unmount());
});

it("keeps opening progress under the chapter title and never claims paused art is preparing", async () => {
  const host = document.createElement("div"); const root = createRoot(host);
  const card = { phase: "opening" as const, chapterEnd: true, chapter: 1, error: null, fromScene: 0, beat: true };
  const action = vi.fn();
  const render = (backgroundState: string, replay = false) => root.render(<ChapterCardView card={card} nextChapter={2} backgroundState={backgroundState} onBackgroundAction={action} replay={replay} onRetry={() => {}} onClose={() => {}} />);
  await act(async () => render("queued"));
  expect(host.textContent).toContain(t("vnChapterStart", { n: 2 }));
  expect(host.textContent).toContain(t("vnChapterOpeningBg"));
  expect(host.querySelector("button")).toBeNull();
  await act(async () => render("paused"));
  expect(host.textContent).toContain(t("vnChapterOpening"));
  expect(host.querySelector("button")?.textContent).toBe(t("vnChapterQuota"));
  await act(async () => host.querySelector("button")!.click());
  expect(action).toHaveBeenCalledOnce();
  await act(async () => render("failed"));
  expect(host.querySelector("button")?.textContent).toBe(t("vnBackgroundRetry"));
  await act(async () => host.querySelector("button")!.click());
  expect(action).toHaveBeenCalledTimes(2);
  expect(host.textContent).not.toContain(t("vnChapterOpeningBg"));
  await act(async () => render("queued", true));
  expect(host.textContent).not.toContain(t("vnChapterOpeningBg"));
  await act(async () => root.unmount());
});
