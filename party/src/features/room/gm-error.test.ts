import { expect, it } from "vitest";
import { gmErrorText, modelBroken } from "./gm-error";
import { t } from "../../i18n";

it("模型本身用不了（下架、金鑰／登入）才灰掉；沒種類的錯誤只講房間出了問題", () => {
  expect((["model_unavailable", "not_ready", "quota", "provider_unreachable", "other"] as const).map(modelBroken)).toEqual([true, true, false, false, false]);
  expect(gmErrorText("scene_full", "AI_CONTEXT_TOO_LONG")).toBe(t("gmErrSceneFull"));
  expect(gmErrorText(null, "ROOM_CONNECT_FAILED")).toBe(t("roomErrOther", { code: "ROOM_CONNECT_FAILED" }));
});
