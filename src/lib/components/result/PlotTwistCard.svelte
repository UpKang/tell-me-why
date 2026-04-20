<script lang="ts">
  /**
   * PlotTwistCard - 反转点卡片
   *
   * 把 plotTwists 以竖向卡片序列渲染。
   * 每张卡突出：
   * - "反转" 标签（金色胶囊）
   * - 引用的时间线事件标题
   * - 为什么算反转（whyTwist）
   *
   * 吃瓜场景下反转是最勾人的"情节钉子"——单独拎出来强调而不是藏在时间线里。
   */

  import type { PlotTwist } from './types';

  interface Props {
    twists: PlotTwist[];
  }

  const { twists }: Props = $props();
</script>

{#if twists.length > 0}
  <section class="twist-section" aria-labelledby="twist-heading">
    <header class="twist-header">
      <h2 id="twist-heading" class="twist-title">关键反转</h2>
      <p class="twist-sub">剧情在这里拐了弯——原本你以为的真相被推翻了</p>
    </header>

    <ul class="twist-list" role="list">
      {#each twists as t, i (i + t.eventRef)}
        <li class="twist-card">
          <div class="twist-top">
            <span class="twist-tag">REVERSAL #{i + 1}</span>
            {#if t.eventRef}
              <span class="twist-ref">→ {t.eventRef}</span>
            {/if}
          </div>
          <p class="twist-why">{t.whyTwist || '（模型未说明为什么是反转）'}</p>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .twist-section {
    padding: clamp(2rem, 3vw, 4rem) clamp(1rem, 2vw, 2.5rem);
    background: var(--bg-primary, #0a0a0d);
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .twist-header {
    text-align: center;
    margin-bottom: 1.5rem;
  }

  .twist-title {
    margin: 0 0 0.4rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.5rem, 2vw + 0.75rem, 2.25rem);
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .twist-sub {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9a9a94);
  }

  .twist-list {
    list-style: none;
    margin: 0 auto;
    padding: 0;
    max-width: 880px;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .twist-card {
    position: relative;
    padding: 1.1rem 1.2rem;
    border-radius: 10px;
    background:
      linear-gradient(135deg, rgba(212, 168, 81, 0.1), transparent 50%),
      var(--bg-card, #17171c);
    border: 1px solid rgba(212, 168, 81, 0.3);
    overflow: hidden;
  }

  /* 左侧金色竖条：视觉"反转钉子" */
  .twist-card::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 4px;
    background: linear-gradient(
      180deg,
      transparent,
      var(--accent-gold, #d4a851) 15%,
      var(--accent-gold, #d4a851) 85%,
      transparent
    );
  }

  .twist-top {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.6rem;
    flex-wrap: wrap;
  }

  .twist-tag {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.68rem;
    letter-spacing: 0.25em;
    padding: 3px 10px;
    border-radius: 4px;
    background: rgba(212, 168, 81, 0.15);
    color: var(--accent-gold, #d4a851);
    border: 1px solid rgba(212, 168, 81, 0.4);
  }

  .twist-ref {
    font-size: 0.82rem;
    color: var(--text-primary, #f6f6f3);
    font-weight: 600;
  }

  .twist-why {
    margin: 0;
    font-size: 0.92rem;
    line-height: 1.6;
    color: var(--text-secondary, #c4c4bd);
  }
</style>
