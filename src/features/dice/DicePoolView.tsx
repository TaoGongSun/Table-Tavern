import { memo } from "react";
import copy from "./dice-copy.json";
import { inspectPool } from "./dice-pool";
import "./dice-pool.css";

export const DicePoolView = memo(function DicePoolView({
  pool,
  text,
  language,
}: {
  pool?: string;
  text: string;
  language: string;
}) {
  if (pool === undefined) return null;
  const labels = copy[language as keyof typeof copy] ?? copy.en;
  const result = inspectPool(pool, text);
  return (
    <details className={`dice-pool${result?.references === 0 ? " dice-idle" : ""}`}>
      <summary>{labels.title}</summary>
      {!result ? (
        <p role="status">{labels.invalid}</p>
      ) : (
        <>
          {result.rows.map((row) => (
            <div className="dice-row" key={row.sides}>
              <strong>d{row.sides}</strong>
              {row.dice.map((die, index) => (
                <span
                  key={index}
                  className={`dice-cell${die.used ? " dice-used" : ""}${die.issues.length ? " dice-error" : ""}`}
                  title={die.used ? labels.used : undefined}
                >
                  #{index + 1}={die.value}
                  {die.used ? " ✓" : ""}
                  {die.issues.map((issue) => (
                    <small key={issue}>⚠ {issue === "face" ? labels.face.replace("{written}", die.reported!.join(", ")).replace("{actual}", String(die.value)) : labels[issue]}</small>
                  ))}
                </span>
              ))}
            </div>
          ))}
          {result.unknown.map((ref, index) => (
            <p className="dice-error" key={index}>
              ⚠ {labels.unknown}: {ref}
            </p>
          ))}
        </>
      )}
    </details>
  );
});
