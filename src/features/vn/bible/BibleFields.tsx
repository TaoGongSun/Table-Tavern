// 劇本分頁的可改欄位（plans/vn-mode-pkg4.md §2.6）：主線、文風、各章文字、秘密文字與揭露時機、旗標說明、
// 結局標題。結構（增刪章、旗標、結局，id 與條件）只能看。
import { t } from "../../../i18n";
import type { Bible, BibleReveal } from "../../../shared/contracts/backend-contracts";

const lines = (text: string) => text.split("\n").map((line) => line.trim()).filter(Boolean);

function revealKey(reveal: BibleReveal): string {
  if (reveal === "never" || typeof reveal === "string") return "never";
  return "flag" in reveal ? `f:${reveal.flag}` : `c:${reveal.chapter}`;
}

function revealOf(key: string): BibleReveal {
  if (key.startsWith("f:")) return { flag: key.slice(2) };
  if (key.startsWith("c:")) return { chapter: Number(key.slice(2)) };
  return "never";
}

function conditionText(condition: Record<string, unknown>): string {
  const { kind, path, min, max, expect, any } = condition;
  if (kind === "range") {
    return [min !== undefined && `${String(min)} ≤`, String(path), max !== undefined && `≤ ${String(max)}`]
      .filter(Boolean)
      .join(" ");
  }
  if (kind === "flag") return `${String(path)} = ${String(expect)}`;
  return `${String(path)} ∈ ${(any as string[] | undefined)?.join(" / ") ?? ""}`;
}

export function BibleFields({
  draft,
  names,
  onChange,
}: {
  draft: Bible;
  names: Record<string, string>;
  onChange: (next: Bible) => void;
}) {
  const set = (patch: Partial<Bible>) => onChange({ ...draft, ...patch });
  const setAt = <K extends "chapters" | "secrets" | "flags" | "endings">(
    key: K,
    index: number,
    patch: Partial<Bible[K][number]>,
  ) =>
    set({
      [key]: draft[key].map((item, at) => (at === index ? { ...item, ...patch } : item)),
    } as Partial<Bible>);
  const text = (label: string, value: string, change: (value: string) => void, rows = 2) => (
    <label>
      {label}
      <textarea rows={rows} value={value} onChange={(event) => change(event.currentTarget.value)} />
    </label>
  );
  const list = (label: string, value: string[], change: (value: string[]) => void) =>
    text(label, value.join("\n"), (raw) => change(lines(raw)), 3);
  const chapters = Array.from({ length: draft.chapter_max }, (_, index) => index + 1);
  return (
    <div className="vn-bible-fields">
      {text(t("vnBibleLogline"), draft.logline, (logline) => set({ logline }))}
      {text(t("vnBibleStyle"), draft.style, (style) => set({ style }))}
      <p>
        {t("vnBibleBranching")}：
        {t(
          draft.branching === "none"
            ? "vnBranchingNone"
            : draft.branching === "light"
              ? "vnBranchingLight"
              : "vnBranchingFull",
        )}
      </p>

      <h3>{t("vnBibleChapters")}</h3>
      {draft.chapters.map((chapter, index) => (
        <fieldset key={index} className="vn-bible-item">
          <legend>{t("vnBibleChapterN", { n: index + 1 })}</legend>
          <label>
            {t("vnChapterTitle")}
            <input
              value={chapter.title}
              onChange={(event) => setAt("chapters", index, { title: event.currentTarget.value })}
            />
          </label>
          {text(t("vnChapterGoal"), chapter.goal, (goal) => setAt("chapters", index, { goal }))}
          {text(t("vnChapterBeats"), chapter.beats, (beats) => setAt("chapters", index, { beats }), 4)}
          {text(t("vnChapterEnd"), chapter.end_condition, (end_condition) =>
            setAt("chapters", index, { end_condition }),
          )}
          {list(t("vnChapterPlant"), chapter.plant, (plant) => setAt("chapters", index, { plant }))}
          {list(t("vnChapterPayoff"), chapter.payoff, (payoff) => setAt("chapters", index, { payoff }))}
          {list(t("vnChapterAnchors"), chapter.anchors, (anchors) => setAt("chapters", index, { anchors }))}
        </fieldset>
      ))}

      <h3>{t("vnBibleSecrets")}</h3>
      {draft.secrets.map((secret, index) => (
        <fieldset key={secret.id} className="vn-bible-item">
          <legend>
            {secret.id}（{secret.holder ? (names[secret.holder] ?? secret.holder) : t("vnSecretWorld")}）
          </legend>
          <label>
            {t("vnSecretTitle")}
            <input
              value={secret.title}
              onChange={(event) => setAt("secrets", index, { title: event.currentTarget.value })}
            />
          </label>
          {text(t("vnSecretText"), secret.text, (value) => setAt("secrets", index, { text: value }), 3)}
          {text(t("vnSecretHint"), secret.hint, (hint) => setAt("secrets", index, { hint }))}
          <label>
            {t("vnSecretReveal")}
            <select
              value={revealKey(secret.reveal)}
              onChange={(event) =>
                setAt("secrets", index, { reveal: revealOf(event.currentTarget.value) })
              }
            >
              <option value="never">{t("vnRevealNever")}</option>
              {chapters.map((n) => (
                <option key={n} value={`c:${n}`}>
                  {t("vnRevealChapter", { n })}
                </option>
              ))}
              {draft.flags.map((flag) => (
                <option key={flag.id} value={`f:${flag.id}`}>
                  {t("vnRevealFlag", { flag: flag.label || flag.id })}
                </option>
              ))}
            </select>
          </label>
        </fieldset>
      ))}

      <h3>{t("vnBibleFlags")}</h3>
      {draft.flags.map((flag, index) => (
        <fieldset key={flag.id} className="vn-bible-item">
          <legend>
            {flag.id}（{flag.type === "route" ? (flag.values ?? []).join(" / ") : flag.type}）
          </legend>
          <label>
            {t("vnFlagLabel")}
            <input
              value={flag.label}
              onChange={(event) => setAt("flags", index, { label: event.currentTarget.value })}
            />
          </label>
          {text(t("vnFlagTrigger"), flag.trigger, (trigger) => setAt("flags", index, { trigger }))}
        </fieldset>
      ))}

      <h3>{t("vnBibleEndings")}</h3>
      {draft.endings.map((ending, index) => (
        <fieldset key={ending.id} className="vn-bible-item">
          <legend>{ending.id}</legend>
          <label>
            {t("vnEndingTitle")}
            <input
              value={ending.title}
              onChange={(event) => setAt("endings", index, { title: event.currentTarget.value })}
            />
          </label>
          <p className="vn-bible-when">
            {ending.when.length === 0 ? t("vnEndingAlways") : ending.when.map(conditionText).join("；")}
          </p>
        </fieldset>
      ))}
    </div>
  );
}
