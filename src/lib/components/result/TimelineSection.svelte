<script lang="ts">
  /**
   * TimelineSection - 事件时间线
   *
   * 吃瓜视角核心模块之一。后端返回的 timelineEvents 按时间顺序列出，
   * 前端左右交替气泡 + 爆点/反转高亮，给用户"顺着读就能看完整件事"的体验。
   *
   * Phase 1 实现原则：
   * - 基础交替气泡布局 + 爆点 / 反转标记
   * - Phase 2 会叠加滚动进场、反转节点 pulse 动效、IntersectionObserver 懒渲染
   */

  import type { TimelineEvent } from './types';

  interface Props {
    events: TimelineEvent[];
  }

  const { events }: Props = $props();

  /**
   * 给每个节点一个布局侧：奇数索引在左，偶数在右，交替排列
   * 时间线空时不渲染
   */
  function sideFor(index: number): 'left' | 'right' {
    return index % 2 === 0 ? 'left' : 'right';
  }

  /**
   * 计算节点的"吃瓜标签"：爆点 / 反转 / 普通
   * 优先级：反转 > 爆点 > 普通（因为反转往往更值得高亮）
   */
  function tagFor(ev: TimelineEvent): { label: string; kind: 'twist' | 'hot' | 'normal' } | null {
    if (ev.isTwist) return { label: '反转', kind: 'twist' };
    if (ev.isHot) return { label: '爆点', kind: 'hot' };
    return null;
  }
</script>

{#if events.length > 0}
  <section class="timeline-section" aria-labelledby="timeline-heading">
    <header class="timeline-header">
      <h2 id="timeline-heading" class="timeline-title">事件时间线</h2>
      <p class="timeline-sub">从头到尾捋一遍，看看哪儿是爆点、哪儿有反转</p>
    </header>

    <ol class="timeline-track" role="list">
      {#each events as ev, i (i + ev.title)}
        {@const side = sideFor(i)}
        {@const tag = tagFor(ev)}
        <li
          class="timeline-node"
          class:side-left={side === 'left'}
          class:side-right={side === 'right'}
          class:is-twist={ev.isTwist}
          class:is-hot={ev.isHot}
        >
          <!-- 中间圆点 -->
          <span class="node-dot" aria-hidden="true"></span>

          <!-- 气泡卡片 -->
          <div class="node-card">
            <div class="node-card-top">
              <span class="node-time">{ev.time || '时间未知'}</span>
              {#if tag}
                <span class="node-tag" class:tag-twist={tag.kind === 'twist'} class:tag-hot={tag.kind === 'hot'}>
                  {tag.label}
                </span>
              {/if}
            </div>
            <h3 class="node-title">{ev.title}</h3>
            {#if ev.desc}
              <p class="node-desc">{ev.desc}</p>
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  </section>
{/if}

<style>
  .timeline-section {
    padding: clamp(2rem, 3vw, 4rem) clamp(1rem, 2vw, 2.5rem);
    background: var(--bg-primary, #0a0a0d);
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .timeline-header {
    text-align: center;
    margin-bottom: 2rem;
  }

  .timeline-title {
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.5rem, 2vw + 0.75rem, 2.25rem);
    margin: 0 0 0.5rem;
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .timeline-sub {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-secondary, #9a9a94);
  }

  /* 中轴线：绝对定位一条竖线贯穿所有节点 */
  .timeline-track {
    position: relative;
    list-style: none;
    margin: 0 auto;
    padding: 0;
    max-width: 880px;
  }

  .timeline-track::before {
    content: '';
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 2px;
    background: linear-gradient(
      to bottom,
      transparent,
      var(--border, rgba(255, 255, 255, 0.2)) 8%,
      var(--border, rgba(255, 255, 255, 0.2)) 92%,
      transparent
    );
    transform: translateX(-1px);
    pointer-events: none;
  }

  .timeline-node {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 24px 1fr;
    align-items: start;
    gap: 0;
    min-height: 60px;
    padding: 0.5rem 0;
  }

  /* 奇偶侧：卡片站对应一侧，另一侧为空 */
  .timeline-node.side-left .node-card {
    grid-column: 1;
    text-align: right;
    margin-right: 1rem;
  }
  .timeline-node.side-right .node-card {
    grid-column: 3;
    text-align: left;
    margin-left: 1rem;
  }

  /* 中间圆点：反转/爆点给不同颜色 */
  .node-dot {
    grid-column: 2;
    grid-row: 1;
    justify-self: center;
    margin-top: 0.5rem;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text-secondary, #9a9a94);
    border: 2px solid var(--bg-primary, #0a0a0d);
    box-shadow: 0 0 0 2px var(--border, rgba(255, 255, 255, 0.15));
  }
  .timeline-node.is-hot .node-dot {
    background: var(--accent-hot, #e6534f);
    box-shadow: 0 0 0 2px rgba(230, 83, 79, 0.4), 0 0 12px rgba(230, 83, 79, 0.6);
  }
  .timeline-node.is-twist .node-dot {
    background: var(--accent-gold, #d4a851);
    box-shadow: 0 0 0 2px rgba(212, 168, 81, 0.4), 0 0 12px rgba(212, 168, 81, 0.6);
  }

  /* 节点卡片 */
  .node-card {
    background: var(--bg-card, #17171c);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    border-radius: 8px;
    padding: 0.9rem 1rem;
    max-width: 100%;
  }

  .node-card-top {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    margin-bottom: 0.4rem;
    flex-wrap: wrap;
  }
  .timeline-node.side-left .node-card-top {
    justify-content: flex-end;
  }

  .node-time {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.75rem;
    letter-spacing: 0.05em;
    color: var(--text-secondary, #9a9a94);
  }

  .node-tag {
    font-size: 0.7rem;
    padding: 2px 8px;
    border-radius: 99px;
    font-weight: 600;
    letter-spacing: 0.05em;
  }
  .node-tag.tag-hot {
    background: rgba(230, 83, 79, 0.15);
    color: var(--accent-hot, #e6534f);
    border: 1px solid rgba(230, 83, 79, 0.4);
  }
  .node-tag.tag-twist {
    background: rgba(212, 168, 81, 0.15);
    color: var(--accent-gold, #d4a851);
    border: 1px solid rgba(212, 168, 81, 0.4);
  }

  .node-title {
    margin: 0 0 0.35rem;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-primary, #f6f6f3);
    line-height: 1.35;
  }

  .node-desc {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-secondary, #c4c4bd);
    line-height: 1.5;
  }

  /* 小屏：两列退化为单列，卡片全部靠左 */
  @media (max-width: 640px) {
    .timeline-track::before {
      left: 12px;
    }
    .timeline-node {
      grid-template-columns: 24px 1fr;
    }
    .timeline-node.side-left .node-card,
    .timeline-node.side-right .node-card {
      grid-column: 2;
      text-align: left;
      margin: 0 0 0 0.75rem;
    }
    .timeline-node.side-left .node-card-top {
      justify-content: flex-start;
    }
    .node-dot {
      grid-column: 1;
      justify-self: center;
    }
  }
</style>
