// 設定頁「生圖模型」：OpenRouter 官方生圖清單下拉＋自訂 id。
// 存檔值不在清單內（舊存檔、清單還沒抓到）就顯示成自訂並帶原值，不改也不丟。
import { useState } from "react";
import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";
import { DEFAULT_IMAGE_MODEL, type ModelOption } from "../ai-connection/model-catalog";

const CUSTOM = "__custom__";

export function ImageModelField({
  value,
  options,
  onChange,
}: {
  /** 空字串＝用預設 */
  value: string;
  options: ModelOption[];
  onChange: (value: string) => void;
}) {
  // 明確選了「自訂」，或值不在清單裡（舊存檔、清單更新後被移除）都顯示自訂框，存的值一定看得到
  const [customChosen, setCustomChosen] = useState(false);
  const custom =
    customChosen || (value !== "" && !options.some((model) => model.id === value));
  return (
    <label>
      {t("imageModelLabel")}
      <select
        value={custom ? CUSTOM : value}
        onChange={(event) => {
          const next = event.currentTarget.value;
          setCustomChosen(next === CUSTOM);
          if (next !== CUSTOM) onChange(next);
        }}
      >
        <option value="">{t("imageModelDefaultOption", { model: DEFAULT_IMAGE_MODEL })}</option>
        {options.map((model) => (
          <option key={model.id} value={model.id}>
            {backendText(model.label)}
          </option>
        ))}
        <option value={CUSTOM}>{t("customModelOption")}</option>
      </select>
      {custom && (
        <input
          value={value}
          placeholder={t("customModelPlaceholder")}
          onChange={(event) => onChange(event.currentTarget.value)}
        />
      )}
    </label>
  );
}
