<script lang="ts">
  /**
   * LongformFallback - 底部折叠的 Markdown 长文报告
   *
   * 作用：
   * - 即便吃瓜模块都有内容，长文报告依然是最全的原始输出——保留为"读完热评想看全文"的入口
   * - 老历史记录没有 extras 时，长文就是唯一内容——这时默认展开
   *
   * Props:
   * - html: 已经过 renderContent（marked + DOMPurify）处理的 HTML 字符串，不要传生 Markdown
   * - defaultOpen: 是否默认展开（有 extras 时 false，无 extras 时 true）
   */

  interface Props {
    html: string;
    defaultOpen?: boolean;
  }

  const { html, defaultOpen = false }: Props = $props();

  // 折叠状态。Svelte 5 rune 写法
  let expanded = $state(defaultOpen);

  function toggle(): void {
    expanded = !expanded;
  }
</script>

{#if html}
  <section class="longform-section" aria-labelledby="longform-heading">
    <header class="longform-header">
      <h2 id="longform-heading" class="longform-title">完整深度分析报告</h2>
      <button
        type="button"
        class="longform-toggle"
        onclick={toggle}
        aria-expanded={expanded}
      >
        {expanded ? '收起' : '展开长文'}
        <span class="toggle-caret" aria-hidden="true">{expanded ? '▲' : '▼'}</span>
      </button>
    </header>

    {#if expanded}
      <!--
        用 {@html} 渲染已净化的 HTML。
        html 必须来自 renderContent（marked.parse + DOMPurify.sanitize）。
      -->
      <article class="longform-body result-html-content">
        {@html html}
      </article>
    {:else}
      <!-- 折叠状态下给一句提示，避免空白 -->
      <p class="longform-hint">点击上面按钮展开查看完整的多揣测 / 支持反驳 / 相似案例长文。</p>
    {/if}
  </section>
{/if}

<style>
  .longform-section {
    padding: clamp(2rem, 3vw, 4rem) clamp(1rem, 2vw, 2.5rem);
    background: var(--bg-secondary, #0e0e12);
    border-top: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .longform-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .longform-title {
    margin: 0;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.25rem, 1.5vw + 0.75rem, 1.75rem);
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .longform-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.9rem;
    border-radius: 999px;
    background: transparent;
    color: var(--text-secondary, #c4c4bd);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.2));
    cursor: pointer;
    font-size: 0.8rem;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }

  .longform-toggle:hover,
  .longform-toggle:focus-visible {
    background: var(--bg-card, #17171c);
    color: var(--text-primary, #f6f6f3);
    border-color: var(--accent-hot, #e6534f);
    outline: none;
  }

  .toggle-caret {
    font-size: 0.6rem;
  }

  .longform-hint {
    margin: 0;
    color: var(--text-secondary, #9a9a94);
    font-size: 0.85rem;
  }

  /* 长文正文本身的排版继承全局 .result-html-content（已在 App.svelte 定义） */
  .longform-body {
    max-width: 880px;
    margin: 0 auto;
  }
</style>
