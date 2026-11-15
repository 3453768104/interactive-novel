/**
 * 暗场（hidden beats）。
 *
 * 主角视角不是"只写主角看得到的东西"——模型仍然把旁人的心思、别处的动静
 * 写出来，只是用「〔暗场〕」标记成读者看不见的段落。这样：
 *  - 剧情在暗场里可以真实推进（分析阶段据此更新别人的状态）；
 *  - 主角视角下阅读器不显示它们，全知视角下正常显示（含历史回合）；
 *  - 作者想看全貌，可以在阅读设置里打开「显示暗场」。
 */

/** 完整的暗场标记，允许全角/半角括号与空格。 */
export const HIDDEN_MARK =
  /^\s*(?:〔\s*暗场\s*〕|【\s*暗场\s*】|\[\s*暗场\s*\]|（\s*暗场\s*）|<<<\s*暗场\s*>>>)\s*/;

/**
 * 流式输出时标记可能只到一半（`〔暗`、`〔暗场`）。这类残缺行也按暗场处理，
 * 否则读者会看到「〔暗场」这种碎片。
 */
function looksLikePartialMark(line: string): boolean {
  const t = line.trim();
  if (!t) return false;
  return /^[〔【\[（]\s*暗?场?\s*[〕】\]）]?$/.test(t) && /[〔【\[（]/.test(t);
}

export interface Para {
  text: string;
  hidden: boolean;
}

export function toParagraphs(raw: string): Para[] {
  return (raw ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) =>
      HIDDEN_MARK.test(line) || looksLikePartialMark(line)
        ? { text: line.replace(HIDDEN_MARK, ""), hidden: true }
        : { text: line, hidden: false },
    );
}
