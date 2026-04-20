<script lang="ts">
  /**
   * WinnersLosersBoard - 赢家输家榜
   *
   * 左右双栏：赢家（金色）/ 输家（红色）。每一条包含：
   * - 人物名
   * - 一句话理由
   *
   * 吃瓜视角下这是最有"情绪价值"的模块——直接告诉你"谁爽了、谁凉了"。
   */

  import type { WinnersLosers } from './types';

  interface Props {
    data: WinnersLosers;
  }

  const { data }: Props = $props();

  const hasWinners = $derived(data?.winners && data.winners.length > 0);
  const hasLosers = $derived(data?.losers && data.losers.length > 0);
  const hasAny = $derived(hasWinners || hasLosers);
</script>

{#if hasAny}
  <section class="winlose-section" aria-labelledby="winlose-heading">
    <header class="winlose-header">
      <h2 id="winlose-heading" class="winlose-title">赢家 vs 输家</h2>
      <p class="winlose-sub">这场瓜里，谁爽了、谁凉了</p>
    </header>

    <div class="winlose-grid">
      <!-- 赢家列 -->
      <article class="winlose-col col-win" aria-labelledby="col-win-head">
        <header class="col-head">
          <span class="col-arrow" aria-hidden="true">▲</span>
          <h3 id="col-win-head" class="col-title">赢家</h3>
          <span class="col-count">{data.winners?.length ?? 0}</span>
        </header>
        {#if hasWinners}
          <ul class="wl-list" role="list">
            {#each data.winners as item, i (i + item.name)}
              <li class="wl-item">
                <span class="wl-rank">#{i + 1}</span>
                <div class="wl-body">
                  <span class="wl-name">{item.name || '未知'}</span>
                  <span class="wl-reason">{item.reason}</span>
                </div>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="col-empty">本局没有明确的赢家</p>
        {/if}
      </article>

      <!-- 输家列 -->
      <article class="winlose-col col-lose" aria-labelledby="col-lose-head">
        <header class="col-head">
          <span class="col-arrow" aria-hidden="true">▼</span>
          <h3 id="col-lose-head" class="col-title">输家</h3>
          <span class="col-count">{data.losers?.length ?? 0}</span>
        </header>
        {#if hasLosers}
          <ul class="wl-list" role="list">
            {#each data.losers as item, i (i + item.name)}
              <li class="wl-item">
                <span class="wl-rank">#{i + 1}</span>
                <div class="wl-body">
                  <span class="wl-name">{item.name || '未知'}</span>
                  <span class="wl-reason">{item.reason}</span>
                </div>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="col-empty">本局没有明确的输家</p>
        {/if}
      </article>
    </div>
  </section>
{/if}

<style>
  .winlose-section {
    padding: clamp(2rem, 3vw, 4rem) clamp(1rem, 2vw, 2.5rem);
    background: var(--bg-secondary, #0e0e12);
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .winlose-header {
    text-align: center;
    margin-bottom: 1.5rem;
  }

  .winlose-title {
    margin: 0 0 0.4rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.5rem, 2vw + 0.75rem, 2.25rem);
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .winlose-sub {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9a9a94);
  }

  .winlose-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    max-width: 1100px;
    margin: 0 auto;
  }

  .winlose-col {
    padding: 1.25rem;
    border-radius: 10px;
    background: var(--bg-card, #17171c);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  }

  .col-win {
    border-color: rgba(212, 168, 81, 0.35);
    background:
      linear-gradient(135deg, rgba(212, 168, 81, 0.08), transparent 60%),
      var(--bg-card, #17171c);
  }
  .col-lose {
    border-color: rgba(230, 83, 79, 0.35);
    background:
      linear-gradient(135deg, rgba(230, 83, 79, 0.08), transparent 60%),
      var(--bg-card, #17171c);
  }

  .col-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 1rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .col-arrow {
    font-size: 1rem;
    line-height: 1;
  }
  .col-win .col-arrow {
    color: var(--accent-gold, #d4a851);
  }
  .col-lose .col-arrow {
    color: var(--accent-hot, #e6534f);
  }

  .col-title {
    margin: 0;
    flex: 1;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: 1.15rem;
    color: var(--text-primary, #f6f6f3);
  }

  .col-count {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.78rem;
    color: var(--text-secondary, #9a9a94);
    padding: 2px 8px;
    border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
    border-radius: 999px;
  }

  .wl-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .wl-item {
    display: flex;
    gap: 0.75rem;
    align-items: flex-start;
    padding: 0.6rem 0.75rem;
    border-radius: 6px;
    background: var(--bg-secondary, #0e0e12);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.06));
  }

  .wl-rank {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.72rem;
    color: var(--text-secondary, #9a9a94);
    line-height: 1.5;
    flex-shrink: 0;
    padding-top: 2px;
  }

  .wl-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .wl-name {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--text-primary, #f6f6f3);
  }
  .col-win .wl-name {
    color: var(--accent-gold, #d4a851);
  }
  .col-lose .wl-name {
    color: var(--accent-hot, #e6534f);
  }

  .wl-reason {
    font-size: 0.8rem;
    color: var(--text-secondary, #c4c4bd);
    line-height: 1.45;
  }

  .col-empty {
    margin: 0;
    padding: 0.75rem;
    font-size: 0.8rem;
    color: var(--text-secondary, #9a9a94);
    font-style: italic;
    text-align: center;
  }

  @media (max-width: 640px) {
    .winlose-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
