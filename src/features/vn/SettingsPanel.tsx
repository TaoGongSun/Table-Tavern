// VN 設定面板：旗標提示、打字速度、auto 等待（app 設定，改了立即生效）。標題與快捷列共用。
import { t, type MsgKey } from "../../i18n";
import { Panel } from "./StoryPanels";
import { AUTO_WAITS, TEXT_SPEEDS, type AutoWait, type TextSpeed } from "./vn-cells";

const TEXT_SPEED_LABELS: Record<TextSpeed, MsgKey> = {
  slow: "vnTextSpeedSlow",
  normal: "vnLevelNormal",
  fast: "vnTextSpeedFast",
  instant: "vnTextSpeedInstant",
};
const AUTO_WAIT_LABELS: Record<AutoWait, MsgKey> = {
  short: "vnAutoWaitShort",
  normal: "vnLevelNormal",
  long: "vnAutoWaitLong",
};

export function SettingsPanel({
  flagHints,
  onFlagHints,
  textSpeed,
  onTextSpeed,
  autoWait,
  onAutoWait,
  onClose,
}: {
  flagHints: boolean;
  onFlagHints: (on: boolean) => void;
  textSpeed: TextSpeed;
  onTextSpeed: (speed: TextSpeed) => void;
  autoWait: AutoWait;
  onAutoWait: (wait: AutoWait) => void;
  onClose: () => void;
}) {
  return (
    <Panel label={t("vnTitleSettings")} onClose={onClose}>
      <label className="vn-setting">
        <input type="checkbox" checked={flagHints} onChange={(event) => onFlagHints(event.target.checked)} />
        {t("vnFlagHintsToggle")}
      </label>
      <label className="vn-setting">
        {t("vnTextSpeed")}
        <select value={textSpeed} onChange={(event) => onTextSpeed(event.target.value as TextSpeed)}>
          {(Object.keys(TEXT_SPEEDS) as TextSpeed[]).map((level) => (
            <option key={level} value={level}>
              {t(TEXT_SPEED_LABELS[level])}
            </option>
          ))}
        </select>
      </label>
      <label className="vn-setting">
        {t("vnAutoWait")}
        <select value={autoWait} onChange={(event) => onAutoWait(event.target.value as AutoWait)}>
          {(Object.keys(AUTO_WAITS) as AutoWait[]).map((level) => (
            <option key={level} value={level}>
              {t(AUTO_WAIT_LABELS[level])}
            </option>
          ))}
        </select>
      </label>
    </Panel>
  );
}
