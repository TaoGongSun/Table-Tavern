// 時間巨集直接用 ST 用的 moment（釘版本 06bde939 的 package-lock：moment 2.30.1，預設 en 語系），
// format token、長格式、humanize 門檻與字串解析都跟 ST 同一份實作。
import moment from "moment";

/** 本地時間排版（moment().format）。 */
export function formatLocal(now: Date, format: string): string {
  return moment(now).format(format);
}

/** moment().utc().utcOffset(offset).format(format)：offset 照 moment 的規則（-16..16 視為小時）。 */
export function formatUtcOffset(now: Date, offset: number, format: string): string {
  return moment(now).utc().utcOffset(offset).format(format);
}

/** moment.duration(ms).humanize(withSuffix)。 */
export function humanize(ms: number, withSuffix = false): string {
  return moment.duration(ms).humanize(withSuffix);
}

/** {{timeDiff}}：moment.duration(moment(left).diff(moment(right))).humanize(true)。 */
export function timeDiff(left: string, right: string): string {
  return moment.duration(moment(left).diff(moment(right))).humanize(true);
}
