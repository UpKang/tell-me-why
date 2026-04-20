/**
 * 历史记录"事件精华一句话"提取工具
 *
 * 为什么需要这个文件？
 * ---------------------
 * 历史记录列表需要用"一句话"概括这次分析的核心结论，但不同时期生成的
 * 历史记录数据结构差异很大，不能只靠单一字段：
 *
 * - Phase 1 之后（新记录）：后端会返回 `truthExtras.oneLinerVerdict`
 *   —— ≤ 30 字的锐评，直接可用。
 * - Phase 1 之前（老记录）：只有一坨 Markdown 长文 `analysisResult`，
 *   甚至有些老版本是 HTML。需要从长文里抽出有价值的首段。
 * - 极端情况：长文也是空的/只有标题，兜底用前 60 字截断。
 *
 * 因此本模块提供一条**三级回退链**，保证任何时期的历史记录都能给出一句话。
 *
 * 使用示例：
 * ```ts
 * import { extractEssence } from '$lib/utils/extractEssence';
 * const oneLine = extractEssence(session); // 永远返回 string（最差是 '暂无摘要'）
 * ```
 */

import type { TruthExtras } from '../components/result/types';

/**
 * 历史记录会话的最小字段子集
 *
 * 为什么不直接 import AnalysisSession 类型？
 * - AnalysisSession 定义在 App.svelte 里（没单独抽出来），循环依赖风险高。
 * - 这里只需要两个字段，用结构化类型降低耦合，以后 session 类型变了这里不受影响。
 */
export interface EssenceSource {
  /** 后端返回的吃瓜结构化包，老记录为 null/undefined */
  truthExtras?: TruthExtras | null;
  /** 分析结果长文（Markdown 或旧 HTML） */
  analysisResult: string;
}

/** 精华句兜底长度（按中文阅读舒适度选 60，约两行 9 号字） */
const FALLBACK_TRUNCATE_LENGTH = 60;

/** 抽取首段时要跳过的最小字数（太短的段落不算"有内容"） */
const MIN_PARAGRAPH_LENGTH = 10;

/** 空结果兜底文案 */
const EMPTY_FALLBACK = '暂无摘要';

/**
 * 主入口：按"三级回退链"从历史记录中抽出一句话精华
 *
 * 回退顺序（优先级由高到低）：
 *   1. `truthExtras.oneLinerVerdict`     —— 有值就用，≤30 字已经是一句话
 *   2. `extractFirstParagraph(analysisResult)` —— 从长文抽首段非空内容
 *   3. `fallbackTruncate(analysisResult)` —— 直接截前 60 字 + "…"
 *   4. `EMPTY_FALLBACK`                   —— 什么都没有时的默认文案
 *
 * @param session 历史记录（至少包含 truthExtras 和 analysisResult）
 * @returns 一句话精华（保证非空字符串）
 */
export function extractEssence(session: EssenceSource): string {
  // 第一级：新记录优先用 AI 自己写的锐评
  const verdict = session.truthExtras?.oneLinerVerdict?.trim();
  if (verdict) {
    return verdict;
  }

  // 第二级：从长文里抽首段（跳过标题、引用、代码块等噪音）
  const firstPara = extractFirstParagraph(session.analysisResult);
  if (firstPara) {
    // 即使抽到了也要限制长度，避免列表项被长段落撑爆
    return firstPara.length > FALLBACK_TRUNCATE_LENGTH
      ? firstPara.slice(0, FALLBACK_TRUNCATE_LENGTH) + '…'
      : firstPara;
  }

  // 第三级：直接截断兜底
  const truncated = fallbackTruncate(session.analysisResult);
  if (truncated) {
    return truncated;
  }

  // 最后兜底：完全没内容
  return EMPTY_FALLBACK;
}

/**
 * 从 Markdown / HTML 长文中抽取第一段有实质内容的纯文本
 *
 * 处理逻辑（按行扫描，遇到第一条符合条件的行就返回）：
 *   - 跳过空行
 *   - 跳过 Markdown 标题行（# / ## / ### 开头）
 *   - 跳过 Markdown 引用行（> 开头）
 *   - 跳过 Markdown 分隔线（--- / ***）
 *   - 跳过 Markdown 代码围栏（```）
 *   - 跳过 HTML 块级标签包裹的行（<h1>、<h2>、<div>、<section> 等）
 *   - 跳过纯装饰性字符行（只有星号/下划线/空白）
 *   - 找到后去掉所有 Markdown 修饰符 + HTML 标签，返回纯文本
 *
 * @param text 原始 Markdown 或 HTML 字符串
 * @returns 第一段纯文本；没抽到返回空字符串
 *
 * @example
 *   extractFirstParagraph('# 标题\n\n这是第一段内容。\n\n## 二级标题\n第二段')
 *   // → '这是第一段内容。'
 *
 *   extractFirstParagraph('<h2>章节</h2>\n<p>这是段落</p>')
 *   // → '这是段落'
 */
export function extractFirstParagraph(text: string): string {
  if (!text) return '';

  // 先把 HTML 拆行处理：\n 作为行分隔符，同时把块级 HTML 标签转成换行占位
  // 这样可以让 <h2>标题</h2><p>内容</p> 这种单行 HTML 也能被正确拆分
  const normalized = text
    // 块级标签前后补换行，方便后续按行扫描
    .replace(/<\/?(h[1-6]|p|div|section|article|ul|ol|li|blockquote|pre|hr)\b[^>]*>/gi, '\n')
    // 压缩连续换行
    .replace(/\n{2,}/g, '\n');

  const lines = normalized.split('\n');

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) continue;

    // 跳过 Markdown 标题
    if (/^#{1,6}\s/.test(line)) continue;

    // 跳过 Markdown 引用
    if (/^>\s?/.test(line)) continue;

    // 跳过分隔线：--- / *** / ___
    if (/^[-*_]{3,}\s*$/.test(line)) continue;

    // 跳过代码围栏
    if (/^```/.test(line)) continue;

    // 跳过只有装饰符的行（如只有 **, --, 空格等）
    if (/^[\s*_\-=~`]+$/.test(line)) continue;

    // 跳过残留的 HTML 标签行（清除标签后为空的）
    const stripped = stripMarkdownAndHtml(line);
    if (stripped.length < MIN_PARAGRAPH_LENGTH) continue;

    return stripped;
  }

  return '';
}

/**
 * 去除 Markdown 语法标记 + HTML 标签，返回干净的纯文本
 *
 * 覆盖常见标记：
 *   - **加粗** / __加粗__ → 加粗
 *   - *斜体* / _斜体_ → 斜体
 *   - `行内代码` → 行内代码
 *   - [链接文字](url) → 链接文字
 *   - ~~删除线~~ → 删除线
 *   - HTML 标签 <xxx> → 直接剥离
 *   - HTML 实体 &nbsp; / &amp; → 空格 / &
 *
 * @param text 待清理的字符串
 * @returns 纯文本
 */
function stripMarkdownAndHtml(text: string): string {
  return text
    // 链接：[文字](url) → 文字
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    // 图片：![alt](url) → alt
    .replace(/!\[([^\]]*)\]\([^)]+\)/g, '$1')
    // 加粗：**xxx** / __xxx__
    .replace(/(\*\*|__)(.+?)\1/g, '$2')
    // 斜体：*xxx* / _xxx_
    .replace(/(\*|_)([^*_\n]+?)\1/g, '$2')
    // 行内代码：`xxx`
    .replace(/`([^`]+)`/g, '$1')
    // 删除线：~~xxx~~
    .replace(/~~(.+?)~~/g, '$1')
    // HTML 标签：<xxx> / </xxx> / <xxx attr="y">
    .replace(/<[^>]+>/g, '')
    // 常见 HTML 实体
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    // 压缩多余空白
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * 最低兜底：直接对原文做 N 字截断
 *
 * 用于 extractFirstParagraph 也抽不出东西的极端情况。
 * 会先清一遍 Markdown/HTML，避免截到标签中间。
 *
 * @param text 原文
 * @param maxLen 最大长度，默认 60
 * @returns 截断后的字符串；原文为空时返回空字符串
 */
export function fallbackTruncate(text: string, maxLen: number = FALLBACK_TRUNCATE_LENGTH): string {
  if (!text) return '';
  const cleaned = stripMarkdownAndHtml(text);
  if (!cleaned) return '';
  return cleaned.length > maxLen ? cleaned.slice(0, maxLen) + '…' : cleaned;
}
