<script lang="ts">
  /**
   * VerdictHero - 一句话锐评 Hero 区
   *
   * 吃瓜视角的"第一屏杀手锏"：
   * 用最巨的字体 + 打字机揭示，让用户进来第一眼就被爆点抓住。
   *
   * Phase 1 实现原则：
   * - 结构先跑通（布局 / 文案 / 无障碍）
   * - Phase 2 再在此基础上叠加 grain 背景、字符逐个揭示、副标签流转等动效
   *
   * Props:
   * - verdict: 后端给的 oneLinerVerdict（≤30 字）；为空时回退到占位文案
   * - meta: 副标签文本（例如"基于 3 条材料 · 5 位人物"），用于小字说明
   */

  interface Props {
    verdict: string;
    meta?: string;
  }

  const { verdict, meta = '' }: Props = $props();

  /**
   * 显示文本：模型没吐 oneLinerVerdict 时给一句兜底，避免 Hero 区空白
   * 兜底文案有意留得模糊——让用户知道"模型没产出锐评"，而不是假装有结论
   */
  const displayVerdict = $derived(verdict && verdict.trim() ? verdict.trim() : '这件事，没有一眼看透的结论');
</script>

<section class="verdict-hero" aria-labelledby="verdict-hero-title">
  <!-- 小标签：提示这是"一句话锐评"而非完整结论 -->
  <span class="verdict-kicker">一句话锐评</span>

  <!-- 主标题：吃瓜版最大字号 -->
  <h1 id="verdict-hero-title" class="verdict-title">
    <span class="verdict-quote-open" aria-hidden="true">「</span>{displayVerdict}<span class="verdict-quote-close" aria-hidden="true">」</span>
  </h1>

  <!-- 副标签：显示基础统计（材料数、人物数等） -->
  {#if meta}
    <p class="verdict-meta">{meta}</p>
  {/if}
</section>

<style>
  .verdict-hero {
    position: relative;
    padding: clamp(2rem, 3vw + 1rem, 4rem) clamp(1rem, 2vw, 2.5rem);
    text-align: center;
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    background: var(--bg-secondary, #0e0e12);
    overflow: hidden;
  }

  /* 顶部小 kicker：悬疑档案风里的"分类标签"位置 */
  .verdict-kicker {
    display: inline-block;
    font-size: 0.7rem;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    color: var(--accent-hot, #e6534f);
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    margin-bottom: 1rem;
  }

  /* 核心大标题：吃瓜视角下字要够大，让用户一眼就抓到爆点 */
  .verdict-title {
    margin: 0 auto;
    max-width: 18ch;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.75rem, 2vw + 1.25rem, 3.25rem);
    font-weight: 700;
    line-height: 1.15;
    letter-spacing: -0.02em;
    color: var(--text-primary, #f6f6f3);
  }

  /* 左右大引号：戏剧感 */
  .verdict-quote-open,
  .verdict-quote-close {
    display: inline-block;
    color: var(--accent-hot, #e6534f);
    font-size: 1em;
    opacity: 0.7;
    margin: 0 0.05em;
  }

  .verdict-meta {
    margin: 1rem 0 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9a9a94);
    letter-spacing: 0.05em;
  }
</style>
