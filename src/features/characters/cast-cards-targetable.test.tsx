// @vitest-environment happy-dom
// 只走 GM 的桌（介面接管桌）：GM 磚與角色卡同一套——不帶選對象／取消對象提示與 aria-pressed，選取外觀照留。
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { NarrowCastTile, NarrowGmTile, WideCastCard, WideGmCard } from "./CastCards";
import type { CharacterMeta } from "./card-model";
import { t } from "../../i18n";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const noop = () => {};
const fox = { id: "c1", name: "狐狸", color: "#336699", avatar: "🦊", tier: "balanced" } as CharacterMeta;

describe("cast cards targetable", () => {
  let root: Root;
  let host: HTMLDivElement;

  beforeEach(() => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function mains(targetable: boolean) {
    act(() =>
      root.render(
        <>
          <WideGmCard selected targetable={targetable} image={null} locked={false} onSelect={noop} onEdit={noop} />
          <NarrowGmTile selected targetable={targetable} image={null} locked={false} onSelect={noop} onEdit={noop} />
          <WideCastCard
            character={fox}
            selected={false}
            targetable={targetable}
            dragging={false}
            image={undefined}
            avatar={undefined}
            locked={false}
            dragProps={null}
            justDragged={() => false}
            onSelect={noop}
            onEdit={noop}
          />
          <NarrowCastTile
            character={fox}
            selected={false}
            targetable={targetable}
            image={undefined}
            avatar={undefined}
            locked={false}
            onSelect={noop}
            onEdit={noop}
          />
        </>,
      ),
    );
    return [...host.querySelectorAll<HTMLButtonElement>(".tcard-main, .rail-tile-main")];
  }

  it("一般桌：GM 磚帶取消提示與按壓態", () => {
    const [wideGm, narrowGm] = mains(true);
    for (const gm of [wideGm, narrowGm]) {
      expect(gm.getAttribute("aria-pressed")).toBe("true");
      expect(gm.title).toBe(t("gmTargetHintClear"));
    }
  });

  it("只走 GM 的桌：GM 磚與角色卡都不帶選對象提示與 aria-pressed，選取外觀照留", () => {
    const buttons = mains(false);
    expect(buttons).toHaveLength(4);
    for (const button of buttons) {
      expect(button.hasAttribute("aria-pressed")).toBe(false);
      expect(button.title).not.toContain(t("gmTargetHintClear"));
      expect(button.title).not.toContain(t("gmTargetHint"));
    }
    expect(host.querySelector(".tcard-gm.tcard-selected")).not.toBeNull();
    expect(host.querySelector(".rail-tile-gm.rail-tile-selected")).not.toBeNull();
  });
});
