import { describe, expect, it, vi } from "vitest";
import {
  COVER_MAX,
  coverCandidates,
  coverLayout,
  loadCoverImages,
  type CoverCandidate,
} from "./table-cover";

const character = (id: string, flags: Partial<Parameters<typeof coverCandidates>[0][number]> = {}) => ({
  id,
  archived: false,
  auto_hidden: false,
  show_image: true,
  ...flags,
});

describe("coverCandidates", () => {
  it("puts the GM image first and keeps the roster order", () => {
    const list = coverCandidates([character("a"), character("b"), character("c")], new Set());
    expect(list).toEqual([
      { kind: "gm" },
      { kind: "character", id: "a" },
      { kind: "character", id: "b" },
      { kind: "character", id: "c" },
    ]);
  });

  it("leaves out archived, hidden and show_image=false characters", () => {
    const list = coverCandidates(
      [
        character("kept"),
        character("archived", { archived: true }),
        character("hidden", { auto_hidden: true }),
        character("noimage", { show_image: false }),
      ],
      new Set(),
    );
    expect(list).toEqual([{ kind: "gm" }, { kind: "character", id: "kept" }]);
  });

  it("brings an auto-hidden character back once it has appeared this scene", () => {
    const list = coverCandidates([character("late", { auto_hidden: true })], new Set(["late"]));
    expect(list).toEqual([{ kind: "gm" }, { kind: "character", id: "late" }]);
  });

  it("does not truncate: every eligible character stays a fallback", () => {
    const many = Array.from({ length: 9 }, (_, index) => character(`c${index}`));
    expect(coverCandidates(many, new Set())).toHaveLength(10);
  });
});

describe("coverLayout", () => {
  it.each([
    [0, "book"],
    [1, "fill"],
    [2, "tiles"],
  ] as const)("%i images -> %s", (count, layout) => {
    expect(coverLayout(count)).toBe(layout);
  });
});

describe("loadCoverImages", () => {
  const ids = (n: number): CoverCandidate[] =>
    Array.from({ length: n }, (_, index) => ({ kind: "character", id: `c${index}` }));
  const srcOf = async (candidate: CoverCandidate) =>
    candidate.kind === "gm" ? "gm" : `img-${candidate.id}`;
  const decodeAll = async () => true;
  const fresh = () => false;

  it("stops at the maximum and never reads more than needed", async () => {
    const fetchImage = vi.fn(srcOf);
    const images = await loadCoverImages(ids(10), fetchImage, decodeAll, fresh);
    expect(images).toEqual(["img-c0", "img-c1"]);
    expect(fetchImage).toHaveBeenCalledTimes(COVER_MAX);
  });

  it("fills from later candidates when the first two fail to load or decode", async () => {
    const fetchImage = async (candidate: CoverCandidate) =>
      candidate.kind === "character" && candidate.id === "c1" ? null : srcOf(candidate);
    const decodes = async (src: string) => src !== "img-c0";
    const images = await loadCoverImages(ids(8), fetchImage, decodes, fresh);
    // c0 解碼失敗、c1 讀不到，c2、c3 成功補上
    expect(images).toEqual(["img-c2", "img-c3"]);
  });

  it("a read that throws counts as a failure, not a crash", async () => {
    const fetchImage = async (candidate: CoverCandidate) => {
      if (candidate.kind === "character" && candidate.id === "c0") throw new Error("boom");
      return srcOf(candidate);
    };
    expect(await loadCoverImages(ids(2), fetchImage, decodeAll, fresh)).toEqual(["img-c1"]);
  });

  it("returns what it found when candidates run out", async () => {
    expect(await loadCoverImages([{ kind: "gm" }], srcOf, decodeAll, fresh)).toEqual(["gm"]);
    expect(await loadCoverImages([], srcOf, decodeAll, fresh)).toEqual([]);
  });

  it("drops a stale response instead of returning it", async () => {
    let stale = false;
    const fetchImage = async (candidate: CoverCandidate) => {
      stale = true;
      return srcOf(candidate);
    };
    expect(await loadCoverImages(ids(3), fetchImage, decodeAll, () => stale)).toBeNull();
  });
});
