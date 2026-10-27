// 回饋句契約（src/shared/contracts/mvu-zod-feedback.json）：桌面 cargo 用同一份比 build_notes 的結果。
import { describe, expect, it } from "vitest";
import contract from "../../shared/contracts/mvu-zod-feedback.json";
import type { ZodCommandResult } from "./host-contract";
import { NOTE_MAX, zodFeedbackLine, zodNotes } from "./zod-feedback";

describe("zod 回饋句與桌面同一份契約", () => {
  for (const sample of contract.cases) {
    it(sample.name, () => {
      const results = sample.results as ZodCommandResult[];
      expect(results.map(zodFeedbackLine)).toEqual(sample.lines);
      expect(zodNotes(results)).toEqual(sample.notes);
      for (const note of sample.notes) expect(Array.from(note).length).toBeLessThanOrEqual(NOTE_MAX);
    });
  }
});
