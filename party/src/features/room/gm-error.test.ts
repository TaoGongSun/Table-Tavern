import { expect, it } from "vitest";
import { gmErrorKind, modelBroken } from "./gm-error";

it("錯誤碼分成要換模型（下架、金鑰）與可直接重試（額度、逾時、網路）", () => {
  expect(["AI_HTTP_STATUS_404", "AI_HTTP_STATUS_401", "cli_model_unavailable"].map(gmErrorKind).map(modelBroken)).toEqual([true, true, true]);
  expect(["AI_HTTP_STATUS_429", "E_GEN_TIMEOUT", "AI_CALL_FAILED", "AI_HTTP_STATUS_503", "WHATEVER"].map(gmErrorKind)).toEqual(["quota", "stalled", "network", "network", "other"]);
});
