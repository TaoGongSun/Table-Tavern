// 單輪輸出失控偵測，門檻照桌面版 src-tauri/src/transport/runaway.rs:8-14：
// 正文 30,000 字元上限、連續空白 2,000、4,000 字窗口內空白九成。逐字元滑動，分塊方式不影響判定。
import { RUNAWAY_CODE } from "./api-failure";

const LENGTH_CAP = 30_000;
const WHITESPACE_RUN = 2_000;
const RATIO_WINDOW = 4_000;
const RATIO_WHITESPACE = (RATIO_WINDOW * 9) / 10;

export type RunawayReason = "length" | "whitespace_run" | "whitespace_ratio";

const WHITESPACE = /\s/u;

/** 一支只看一條增量流：正文那支有字數上限＋退化偵測，思考那支只做退化偵測。 */
export class RunawayGuard {
  private chars = 0;
  private run = 0;
  private window: boolean[] = new Array(RATIO_WINDOW).fill(false);
  private cursor = 0;
  private filled = 0;
  private whitespaceInWindow = 0;
  private tripped: RunawayReason | null = null;

  private constructor(private lengthCap: number | null) {}

  static text(): RunawayGuard {
    return new RunawayGuard(LENGTH_CAP);
  }

  static thinking(): RunawayGuard {
    return new RunawayGuard(null);
  }

  message(reason: RunawayReason): string {
    return `${RUNAWAY_CODE} reason=${reason} chars=${this.chars}`;
  }

  /** 吃進一段增量；觸發後之後每次都回同一個理由。 */
  push(delta: string): RunawayReason | null {
    if (this.tripped) return this.tripped;
    for (const ch of delta) {
      this.chars += 1;
      if (this.lengthCap !== null && this.chars > this.lengthCap) {
        this.tripped = "length";
        break;
      }
      const space = WHITESPACE.test(ch);
      this.run = space ? this.run + 1 : 0;
      if (this.run >= WHITESPACE_RUN) {
        this.tripped = "whitespace_run";
        break;
      }
      if (this.filled === RATIO_WINDOW) {
        if (this.window[this.cursor]) this.whitespaceInWindow -= 1;
      } else {
        this.filled += 1;
      }
      this.window[this.cursor] = space;
      if (space) this.whitespaceInWindow += 1;
      this.cursor = (this.cursor + 1) % RATIO_WINDOW;
      if (this.filled === RATIO_WINDOW && this.whitespaceInWindow >= RATIO_WHITESPACE) {
        this.tripped = "whitespace_ratio";
        break;
      }
    }
    return this.tripped;
  }
}
