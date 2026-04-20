<script lang="ts">
  /**
   * HotQuotesCarousel - 金句卡横向轮播
   *
   * 每条金句用一张大字号卡片，横向 scroll-snap 排列，
   * 让用户像翻 Instagram Stories 一样刷金句。
   *
   * 设计：
   * - 大号衬线字 + 巨型左右引号
   * - speaker 在下方带 "— XX" 签名
   * - context 作为小字脚注
   */

  import type { HotQuote } from './types';

  interface Props {
    quotes: HotQuote[];
  }

  const { quotes }: Props = $props();
</script>

{#if quotes.length > 0}
  <section class="quotes-section" aria-labelledby="quotes-heading">
    <header class="quotes-header">
      <h2 id="quotes-heading" class="quotes-title">金句与爆点</h2>
      <p class="quotes-sub">吃瓜必传的几句话 · 左右滑动查看更多</p>
    </header>

    <div class="quotes-track" role="list" aria-label="金句列表（可左右滑动）">
      {#each quotes as q, i (i + q.quote)}
        <article class="quote-card" role="listitem">
          <span class="quote-open" aria-hidden="true">“</span>
          <blockquote class="quote-body">
            {q.quote || '（无原话）'}
          </blockquote>
          <span class="quote-close" aria-hidden="true">”</span>

          <footer class="quote-foot">
            <span class="quote-speaker">— {q.speaker || '匿名'}</span>
            {#if q.context}
              <span class="quote-context">{q.context}</span>
            {/if}
          </footer>
        </article>
      {/each}
    </div>
  </section>
{/if}

<style>
  .quotes-section {
    padding: clamp(2rem, 3vw, 4rem) 0;
    background: var(--bg-primary, #0a0a0d);
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .quotes-header {
    text-align: center;
    margin-bottom: 1.5rem;
    padding: 0 clamp(1rem, 2vw, 2.5rem);
  }

  .quotes-title {
    margin: 0 0 0.4rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.5rem, 2vw + 0.75rem, 2.25rem);
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .quotes-sub {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9a9a94);
  }

  /* 横向轨道：scroll-snap 让卡片对齐 */
  .quotes-track {
    display: flex;
    gap: 1rem;
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    -webkit-overflow-scrolling: touch;
    padding: 0.5rem clamp(1rem, 6vw, 4rem) 1.5rem;
    scrollbar-width: thin;
    scrollbar-color: var(--border, rgba(255, 255, 255, 0.15)) transparent;
  }

  .quotes-track:focus-visible {
    outline: 2px solid var(--accent-hot, #e6534f);
    outline-offset: 4px;
  }

  .quotes-track::-webkit-scrollbar {
    height: 6px;
  }
  .quotes-track::-webkit-scrollbar-thumb {
    background: var(--border, rgba(255, 255, 255, 0.15));
    border-radius: 3px;
  }

  .quote-card {
    position: relative;
    flex: 0 0 min(560px, 85vw);
    scroll-snap-align: start;
    padding: 2rem 1.5rem 1.5rem;
    border-radius: 12px;
    border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
    background:
      linear-gradient(180deg, rgba(230, 83, 79, 0.04), transparent 50%),
      var(--bg-card, #17171c);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    min-height: 240px;
  }

  .quote-open,
  .quote-close {
    position: absolute;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: 4rem;
    color: var(--accent-hot, #e6534f);
    opacity: 0.6;
    line-height: 1;
    font-weight: 700;
  }
  .quote-open {
    top: 0.4rem;
    left: 1rem;
  }
  .quote-close {
    bottom: -0.5rem;
    right: 1.2rem;
  }

  .quote-body {
    flex: 1;
    margin: 0;
    padding: 0 1rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.05rem, 1vw + 0.75rem, 1.35rem);
    line-height: 1.45;
    color: var(--text-primary, #f6f6f3);
    font-weight: 500;
  }

  .quote-foot {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .quote-speaker {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.78rem;
    letter-spacing: 0.05em;
    color: var(--accent-gold, #d4a851);
  }

  .quote-context {
    font-size: 0.72rem;
    color: var(--text-secondary, #9a9a94);
    line-height: 1.4;
  }
</style>
