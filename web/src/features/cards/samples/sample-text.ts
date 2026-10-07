/** 內建範例卡每個語系要寫的欄位（其餘欄位各語系相同，見 sample-card.ts） */
export interface SampleText {
  name: string;
  description: string;
  personality: string;
  scenario: string;
  first_mes: string;
  creator_notes: string;
  tags: string[];
}
