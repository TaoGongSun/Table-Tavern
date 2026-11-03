// 格邊旗標提示（plans/vn-mode-pkg4.md §2.7）：播到某則事件的第一格時，列出這則的旗標與揭露變動。
import { t } from "../../../i18n";
import type { FlagHint } from "../../../shared/contracts/backend-contracts";
import "./vn-bible.css";

function hintText(hint: FlagHint): string {
  if (hint.kind === "reveal") return t("vnFlagReveal", { title: hint.title });
  if (hint.mark) return `${hint.label} ${hint.mark}`;
  const value = hint.value === "true" ? t("vnFlagOn") : hint.value === "false" ? t("vnFlagOff") : hint.value;
  return `${hint.label}：${value ?? ""}`;
}

export function FlagHints({ hints }: { hints: FlagHint[] }) {
  return (
    <ul className="vn-flag-hints" aria-live="polite">
      {hints.map((hint, index) => (
        <li key={index}>◆ {hintText(hint)}</li>
      ))}
    </ul>
  );
}
