<script lang="ts">
  /**
   * FactionsSplit - 阵营对撞
   *
   * 把 factions 以左右（或多列）分栏对撞呈现。每个阵营用一张卡片显示：
   * - 阵营名称 + 一句话口号
   * - 核心立场
   * - 代表人物 chip
   *
   * 视觉：首个阵营用"热"色系、第二个用"冷"色系，第三个起用中性灰。
   * 强调"站队感"。
   */

  import type { Faction } from './types';

  interface Props {
    factions: Faction[];
  }

  const { factions }: Props = $props();

  /**
   * 根据 index 决定阵营配色（热 / 冷 / 中性）
   * 规则：index=0 热色，index=1 冷色，其余中性
   */
  function toneFor(i: number): 'hot' | 'cold' | 'neutral' {
    if (i === 0) return 'hot';
    if (i === 1) return 'cold';
    return 'neutral';
  }
</script>

{#if factions.length > 0}
  <section class="factions-section" aria-labelledby="factions-heading">
    <header class="factions-header">
      <h2 id="factions-heading" class="factions-title">阵营站队</h2>
      <p class="factions-sub">大家站在哪一边？谁和谁是一伙的？</p>
    </header>

    <div class="factions-grid" class:single={factions.length === 1} class:duo={factions.length === 2}>
      {#each factions as f, i (i + f.name)}
        {@const tone = toneFor(i)}
        <article class="faction-card" data-tone={tone}>
          <!-- 顶部标签：阵营在"几号阵营" -->
          <span class="faction-index">阵营 {i + 1}</span>

          <!-- 阵营名 + 口号 -->
          <h3 class="faction-name">{f.name || '未命名阵营'}</h3>
          {#if f.keyClaim}
            <p class="faction-claim">
              <span class="claim-quote" aria-hidden="true">“</span>
              {f.keyClaim}
              <span class="claim-quote" aria-hidden="true">”</span>
            </p>
          {/if}

          <!-- 核心立场 -->
          {#if f.stance}
            <div class="faction-stance">
              <span class="stance-tag">立场</span>
              <span class="stance-body">{f.stance}</span>
            </div>
          {/if}

          <!-- 代表人物 -->
          {#if f.members && f.members.length > 0}
            <ul class="faction-members" role="list">
              {#each f.members as m (m)}
                <li class="member-chip">
                  <span class="chip-avatar">{m.charAt(0)}</span>
                  <span class="chip-name">{m}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </article>
      {/each}
    </div>
  </section>
{/if}

<style>
  .factions-section {
    padding: clamp(2rem, 3vw, 4rem) clamp(1rem, 2vw, 2.5rem);
    background: var(--bg-secondary, #0e0e12);
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .factions-header {
    text-align: center;
    margin-bottom: 1.5rem;
  }

  .factions-title {
    margin: 0 0 0.4rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.5rem, 2vw + 0.75rem, 2.25rem);
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .factions-sub {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9a9a94);
  }

  .factions-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 1rem;
    max-width: 1100px;
    margin: 0 auto;
  }
  .factions-grid.duo {
    grid-template-columns: 1fr 1fr;
  }
  .factions-grid.single {
    grid-template-columns: minmax(0, 560px);
    justify-content: center;
  }

  .faction-card {
    position: relative;
    padding: 1.25rem 1.25rem 1rem;
    border-radius: 10px;
    border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
    background: var(--bg-card, #17171c);
    overflow: hidden;
  }

  /* 色调：让首个阵营偏红、第二个偏蓝，形成对撞 */
  .faction-card[data-tone='hot'] {
    border-color: rgba(230, 83, 79, 0.4);
    background:
      linear-gradient(135deg, rgba(230, 83, 79, 0.08), transparent 60%),
      var(--bg-card, #17171c);
  }
  .faction-card[data-tone='hot'] .faction-index {
    color: var(--accent-hot, #e6534f);
    border-color: rgba(230, 83, 79, 0.4);
  }
  .faction-card[data-tone='hot'] .claim-quote {
    color: var(--accent-hot, #e6534f);
  }

  .faction-card[data-tone='cold'] {
    border-color: rgba(108, 160, 222, 0.35);
    background:
      linear-gradient(135deg, rgba(108, 160, 222, 0.08), transparent 60%),
      var(--bg-card, #17171c);
  }
  .faction-card[data-tone='cold'] .faction-index {
    color: var(--accent-cold, #6ca0de);
    border-color: rgba(108, 160, 222, 0.4);
  }
  .faction-card[data-tone='cold'] .claim-quote {
    color: var(--accent-cold, #6ca0de);
  }

  .faction-index {
    display: inline-block;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.65rem;
    letter-spacing: 0.25em;
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: 999px;
    border: 1px solid var(--border, rgba(255, 255, 255, 0.2));
    color: var(--text-secondary, #9a9a94);
    margin-bottom: 0.75rem;
  }

  .faction-name {
    margin: 0 0 0.5rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: 1.35rem;
    font-weight: 700;
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .faction-claim {
    margin: 0 0 1rem;
    font-size: 0.95rem;
    line-height: 1.5;
    color: var(--text-primary, #f6f6f3);
    font-style: italic;
  }

  .claim-quote {
    font-size: 1.4em;
    font-weight: 700;
    margin: 0 0.1em;
    opacity: 0.7;
  }

  .faction-stance {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
    align-items: flex-start;
  }

  .stance-tag {
    flex-shrink: 0;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.65rem;
    padding: 2px 8px;
    border-radius: 4px;
    background: var(--bg-secondary, #0e0e12);
    color: var(--text-secondary, #9a9a94);
    letter-spacing: 0.2em;
    height: fit-content;
  }

  .stance-body {
    font-size: 0.85rem;
    color: var(--text-secondary, #c4c4bd);
    line-height: 1.5;
  }

  .faction-members {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .member-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0.6rem 0.25rem 0.25rem;
    border-radius: 999px;
    background: var(--bg-secondary, #0e0e12);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
    font-size: 0.78rem;
    color: var(--text-primary, #f6f6f3);
  }

  .chip-avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--accent-hot, #e6534f), var(--accent-gold, #d4a851));
    color: #fff;
    font-size: 0.72rem;
    font-weight: 700;
  }

  .chip-name {
    line-height: 1;
  }

  @media (max-width: 640px) {
    .factions-grid,
    .factions-grid.duo {
      grid-template-columns: 1fr;
    }
  }
</style>
