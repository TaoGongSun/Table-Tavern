// @vitest-environment happy-dom
// 舞台退路鏈：該表情臉層 → 定稿 → 角色卡原圖 → 剪影。
import { act } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";
import type { SpriteView } from "../../shared/contracts/backend-contracts";
import { MIN_SCALE, Stage, placeOf } from "./Stage";
import { spriteLayers } from "./SpriteFigure";
import { spriteNight, tintOf } from "./StageBackdrop";
import type { Stage as StageState } from "./vn-cells";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const sprite: SpriteView = {
  meta: {
    version: 1,
    face: { x: 100, y: 50, w: 200, h: 300 },
    face_fallback: false,
    variants: [
      { key: "v1", label: "Armor", face: null },
      { key: "v2", label: "泳裝", face: { x: 0, y: 0, w: 50, h: 50 } },
      { key: "v3", label: "Gone", face: null },
    ],
    default: "base",
    revs: {},
  },
  width: 1000,
  height: 1500,
  body: "data:body",
  faces: { happy: "data:happy" },
  variant_layers: {
    v1: { body: "data:armor", width: 1000, height: 1500, faces: {} },
    v2: { body: "data:swim", width: 500, height: 800, faces: { sad: "data:swim-sad" } },
  },
  expressions: [{ key: "tender", label: "溫柔" }],
  declared_variants: [],
  scale: 0.5,
};

function render(expressions: Record<string, string>, slots: (string | null)[] = ["a", "b", "c"]) {
  const stage: StageState = {
    slots,
    names: { a: "A", b: "B", c: "C" },
    expressions,
    variants: {},
    speaker: "a",
    directive: null,
    location: null,
    time: null,
    backdrop: null,
  };
  const host = document.createElement("div");
  act(() =>
    createRoot(host).render(
      <Stage stage={stage} sprites={{ a: sprite }} images={{ b: "data:card" }} cellKey="k" />,
    ),
  );
  return host;
}

describe("Stage fallback chain", () => {
  it("overlays the expression face layer on the body at the face box", () => {
    const host = render({ a: "happy" });
    const face = host.querySelector<HTMLImageElement>(".vn-slot-0 .vn-face")!;
    expect(face.getAttribute("src")).toBe("data:happy");
    expect(face.style.left).toBe("10%");
    expect(face.style.height).toBe("20%");
    expect(host.querySelector(".vn-slot-1 img")!.getAttribute("src")).toBe("data:card");
    expect(host.querySelector(".vn-slot-2 .vn-silhouette")).not.toBeNull();
  });

  it("card-declared expression keys use their face layer; height scale sets the slot ratio", () => {
    const host = render({ a: "tender" });
    expect(host.querySelector(".vn-slot-0 img")!.getAttribute("src")).toBe("data:body");
    const tender = { ...sprite, faces: { tender: "data:tender" } };
    expect(spriteLayers(tender, undefined, "tender").face).toBe("data:tender");
    // 名單最高者＝1，下限 0.7：0.5 的角色照 0.7 畫
    expect(host.querySelector<HTMLElement>(".vn-slot-0")!.style.getPropertyValue("--vn-scale")).toBe(String(MIN_SCALE));
    expect(host.querySelector<HTMLElement>(".vn-slot-1")!.style.getPropertyValue("--vn-scale")).toBe("");
  });

  it("someone alone on stage stands in the centre slot; two or more keep their slots", () => {
    expect(placeOf(["a", null, null], 0)).toBe(2);
    expect(placeOf([null, "b", null], 1)).toBe(2);
    expect(placeOf(["a", "b", null], 0)).toBe(0);
    expect(placeOf(["a", "b", null], 1)).toBe(1);
    const host = render({}, ["a", null, null]);
    expect(host.querySelector(".vn-slot-2 .vn-layers")).not.toBeNull();
    expect(host.querySelector(".vn-slot-0")).toBeNull();
  });

  it("shows only the body when that expression has no face layer yet", () => {
    const host = render({ a: "sad" });
    expect(host.querySelector(".vn-slot-0 .vn-face")).toBeNull();
    expect(host.querySelector(".vn-slot-0 img")!.getAttribute("src")).toBe("data:body");
  });

  it("@variant: shared set reuses base faces, own set uses its own, unknown falls back to base", () => {
    expect(spriteLayers(sprite, "armor", "happy")).toMatchObject({ body: "data:armor", face: "data:happy", box: sprite.meta.face });
    expect(spriteLayers(sprite, "泳裝", "sad")).toMatchObject({ body: "data:swim", face: "data:swim-sad", width: 500 });
    expect(spriteLayers(sprite, "v2", "happy").face).toBeUndefined();
    expect(spriteLayers(sprite, "Gone", "happy")).toMatchObject({ body: "data:body", face: "data:happy" });
    expect(spriteLayers(sprite, "nope", "happy").body).toBe("data:body");
  });
});

describe("time-of-day tint", () => {
  const image = (period: string) => ({ src: "data:harbor", period, rev: 1 });
  it("tints the shared base for other periods but not its own or unknown card periods", () => {
    expect(tintOf("day", image("day"))).toBeNull();
    expect(tintOf("night", image("day"))?.opacity).toBeGreaterThan(0);
    expect(tintOf("dusk", image("day"))).not.toEqual(tintOf("morning", image("day")));
    // 專屬圖（實際時段＝要的）不疊；卡自訂時段沒有對應色
    expect(tintOf("night", image("night"))).toBeNull();
    expect(tintOf("crisis", image("day"))).toBeNull();
  });
});

describe("sprite arrival", () => {
  const stage = (backdrop: StageState["backdrop"] = null): StageState => ({
    slots: ["a", null, null],
    names: { a: "A" },
    expressions: { a: "neutral" },
    variants: {},
    speaker: "a",
    directive: null,
    location: null,
    time: null,
    backdrop,
  });
  it("swaps card art for the new sprite only on the next cell, fading the old one out", () => {
    const host = document.createElement("div");
    const root = createRoot(host);
    const draw = (sprites: Record<string, SpriteView>, cellKey: string) =>
      act(() => root.render(<Stage stage={stage()} sprites={sprites} images={{ a: "data:card" }} cellKey={cellKey} />));
    draw({}, "k1");
    draw({ a: sprite }, "k1");
    // 打字中途不換
    expect(host.querySelector(".vn-layers")).toBeNull();
    draw({ a: sprite }, "k2");
    expect(host.querySelector(".vn-layers.vn-swap-in")).not.toBeNull();
    expect(host.querySelector("img.vn-swap-out")?.getAttribute("src")).toBe("data:card");
  });

  it("night darkens sprites at half the backdrop tint; other periods leave them alone", () => {
    expect(spriteNight("night")!.opacity).toBeLessThan(0.5);
    expect(spriteNight("dusk")).toBeNull();
    const host = document.createElement("div");
    act(() =>
      createRoot(host).render(
        <Stage stage={stage({ location: "港口", slug: "s", period: "night" })} sprites={{ a: sprite }} images={{}} cellKey="k" />,
      ),
    );
    expect(host.querySelector(".vn-layers .vn-sprite-tint")).not.toBeNull();
  });
});
