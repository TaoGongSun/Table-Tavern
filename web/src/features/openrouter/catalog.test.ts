import { describe, expect, it } from "vitest";
import { buildLineup, eligibleModels, parseCatalog, parseRankedSlugs, stableCandidates, type FreeModel } from "./catalog";

const NOW = 1_800_000_000;
const OLD = NOW - 30 * 86_400;

function entry(id: string, extra: Record<string, unknown> = {}) {
  return {
    id,
    name: id,
    created: OLD,
    context_length: 65_536,
    pricing: { prompt: "0", completion: "0" },
    architecture: { input_modalities: ["text"], output_modalities: ["text"] },
    ...extra,
  };
}

const model = (id: string, extra: Partial<FreeModel> = {}): FreeModel => ({
  id,
  canonicalSlug: id.replace(/:free$/, ""),
  name: id,
  created: OLD,
  contextLength: 65_536,
  expirationAt: null,
  supportedParameters: [],
  ...extra,
});

describe("parseCatalog", () => {
  it("keeps only zero-priced text-only models", () => {
    const parsed = parseCatalog({
      data: [
        entry("a/free:free"),
        entry("b/paid", { pricing: { prompt: "0.000001", completion: "0" } }),
        entry("c/music:free", { architecture: { input_modalities: ["text"], output_modalities: ["text", "audio"] } }),
      ],
    });
    expect(parsed?.map((m) => m.id)).toEqual(["a/free:free"]);
  });

  it("returns null for a broken body so the caller keeps the old cache", () => {
    expect(parseCatalog({ nope: true })).toBeNull();
    expect(parseCatalog({ data: [] })).toEqual([]);
  });

  it("falls back to id when canonical_slug is blank", () => {
    expect(parseRankedSlugs({ data: [{ id: "x/y", canonical_slug: " " }] })).toEqual(["x/y"]);
  });
});

describe("selection", () => {
  it("drops models that cannot hold the sentence or expire within a day", () => {
    const list = [model("a:free"), model("b:free", { contextLength: 1_000 }), model("c:free", { expirationAt: NOW + 3_600 })];
    expect(eligibleModels(list, 8_000, NOW).map((m) => m.id)).toEqual(["a:free"]);
  });

  it("ranks roleplay first, then weekly, then the rest; skips stealth and fresh models", () => {
    const list = [
      model("p/one:free"),
      model("q/two:free"),
      model("r/three:free"),
      model("stealth/x:free"),
      model("s/new:free", { created: NOW - 86_400 }),
    ];
    const ranked = stableCandidates(list, ["q/two"], ["r/three:free"], 4_096, NOW);
    expect(ranked.map(({ model, source }) => `${model.id}:${source}`)).toEqual([
      "q/two:free:roleplay",
      "r/three:free:weekly",
      "p/one:free:available",
    ]);
  });

  it("does not put two models from the same upstream next to each other", () => {
    const ranked = ["a1", "a2", "b1", "b2"].map((id) => ({ model: model(id), source: "roleplay" as const }));
    const upstreams = new Map([
      ["a1", ["A"]],
      ["a2", ["A"]],
      ["b1", ["B"]],
      ["b2", ["B"]],
    ]);
    const lineup = buildLineup(ranked, upstreams);
    expect(lineup.entries.map(({ model }) => model.id)).toEqual(["a1", "b1", "a2", "b2"]);
    expect(lineup.diversified).toBe(true);
  });
});
