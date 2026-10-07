// @vitest-environment happy-dom
// 找卡清單：五站只放連結（D6）、一行 18 禁標示、外連不帶來源。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeAll, describe, expect, it } from "vitest";
import { CARD_SITES } from "./card-sites";
import { FindCards } from "./FindCards";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

describe("where to find cards", () => {
  it("lists the five sites as plain https links with distinct ids", () => {
    expect(CARD_SITES.map((site) => site.name)).toEqual(["RisuRealm", "Chub", "AICharacterCards", "Character Tavern", "Pygmalion"]);
    expect(new Set(CARD_SITES.map((site) => site.id)).size).toBe(CARD_SITES.length);
    for (const site of CARD_SITES) expect(new URL(site.url).protocol).toBe("https:");
  });

  it("renders each site as an outside link with the one-line adult notice", async () => {
    const host = document.createElement("div");
    const root = createRoot(host);
    await act(async () => root.render(<FindCards />));
    const anchors = [...host.querySelectorAll("a")];
    expect(anchors.map((a) => a.getAttribute("href"))).toEqual(CARD_SITES.map((site) => site.url));
    for (const a of anchors) {
      expect(a.getAttribute("target")).toBe("_blank");
      expect(a.getAttribute("rel")).toBe("noopener noreferrer");
    }
    expect(host.querySelectorAll(".find-adult")).toHaveLength(1);
    expect(host.textContent).toContain("18 禁");
    await act(async () => root.unmount());
  });
});
