<script lang="ts">
  /**
   * CharacterNetwork - 人物关系网（SVG 环形布局）
   *
   * 吃瓜场景核心模块之一：用关系图取代原本扁平的"人物列表"。
   * 让用户一眼看清"谁和谁是一伙的、谁和谁在撕"。
   *
   * 布局策略：
   * - 所有人物节点均匀分布在一个圆上（圆心在容器中心）
   * - 名称和角色文字统一放在头像正下方，水平居中排列，不会被裁剪
   * - 人物名通过 from/to 匹配 RelationEdge，画成 SVG line
   * - sentiment 决定连线颜色：positive 冷蓝 / negative 爆点红 / neutral 灰
   * - 点击节点 → 通过 onSelect 回调把 character 传给父级（由父级打开 Drawer）
   */

  import type { CharacterProfile } from '../CharacterProfilePanel.svelte';
  import type { RelationEdge } from './types';

  interface Props {
    characters: CharacterProfile[];
    relations: RelationEdge[];
    /** 点击节点回调 */
    onSelect: (character: CharacterProfile) => void;
  }

  const { characters, relations, onSelect }: Props = $props();

  // ========== 布局几何参数（动态适配） ==========
  // SVG viewBox 尺寸：加大给标签留出充足边距
  const VIEW_SIZE = 1400;
  const CENTER = VIEW_SIZE / 2;

  // 根据人物数量动态计算圆半径
  // 2 人 → 380, 5 人 → 420, 10 人 → 460, 15+ 人 → 500
  const RADIUS = $derived(
    Math.min(Math.max(380 + characters.length * 8, 380), 500),
  );

  // 节点头像半径：人物多时稍微缩小
  const NODE_R = $derived(
    characters.length > 8 ? 34 : characters.length > 5 ? 38 : 44,
  );

  // 首字母字号：与 NODE_R 保持比例（约 0.85 倍）
  const INITIAL_SIZE = $derived(Math.round(NODE_R * 0.85));

  // 标签字号：根据人物数量动态缩放
  const LABEL_SIZE = $derived(
    characters.length > 8 ? 16 : characters.length > 5 ? 18 : 20,
  );
  const SUB_LABEL_SIZE = $derived(
    characters.length > 8 ? 11 : characters.length > 5 ? 12 : 13,
  );

  // 名称文字到头像底边的间距（头像半径 + gap）
  const LABEL_GAP = $derived(NODE_R + 12);

  // 角色文字到名称文字的行间距
  const LINE_GAP = $derived(LABEL_SIZE * 0.7 + 4);

  /**
   * 计算某个索引在圆周上的坐标
   * 从正上方 (−π/2) 开始，顺时针均匀分布
   */
  function polar(i: number, total: number): { x: number; y: number; angle: number } {
    const total_safe = Math.max(total, 1);
    const angle = -Math.PI / 2 + (i / total_safe) * Math.PI * 2;
    return {
      x: CENTER + RADIUS * Math.cos(angle),
      y: CENTER + RADIUS * Math.sin(angle),
      angle,
    };
  }

  // 为每个人物预计算坐标，按 name 做索引方便 relation 连线查 from/to
  const positions = $derived(() => {
    const map = new Map<string, { x: number; y: number; angle: number }>();
    characters.forEach((c, i) => {
      map.set(c.name, polar(i, characters.length));
    });
    return map;
  });

  /**
   * 给定 sentiment 返回 SVG 连线颜色和描边宽度
   * positive → 冷蓝/同盟 · negative → 爆点红/敌对 · neutral → 灰虚线/中性
   */
  function strokeFor(sentiment: string): { stroke: string; width: number; dash: string } {
    const normalized = normalizeSentiment(sentiment);
    switch (normalized) {
      case 'positive':
        return { stroke: 'var(--accent-cold, #6ca0de)', width: 2.5, dash: '0' };
      case 'negative':
        return { stroke: 'var(--accent-hot, #e6534f)', width: 2.5, dash: '0' };
      default:
        return { stroke: 'var(--text-secondary, #9a9a94)', width: 1.5, dash: '6 4' };
    }
  }

  /** 前端兜底：将非标准 sentiment 值映射为 positive/negative/neutral */
  function normalizeSentiment(raw: string): string {
    const lowered = raw.trim().toLowerCase();
    switch (lowered) {
      case 'positive': case 'negative': case 'neutral':
        return lowered;
      // 敌对类变体 → negative
      case 'hostile': case 'enemy': case 'adversarial': case 'antagonistic':
      case 'oppositional': case 'confrontational': case 'against': case 'rival':
      case 'conflict': case 'bad':
        return 'negative';
      // 同盟类变体 → positive
      case 'friendly': case 'cooperative': case 'ally': case 'alliance':
      case 'supportive': case 'good': case 'collaborative': case 'amicable':
        return 'positive';
      // 空值/未知 → neutral
      default:
        return 'neutral';
    }
  }

  // 过滤掉引用不存在人物的关系边（避免画出断线）
  const validRelations = $derived(
    relations.filter((r) => positions().has(r.from) && positions().has(r.to) && r.from !== r.to),
  );

  // 被丢弃的边数量 > 0 时在 console 输出 warn
  $effect(() => {
    const dropped = relations.length - validRelations.length;
    if (dropped > 0) {
      const pos = positions();
      console.warn(
        `[CharacterNetwork] ${dropped} 条关系边被丢弃（from/to 人名不在节点列表中）`,
        relations.filter((r) => !pos.has(r.from) || !pos.has(r.to) || r.from === r.to),
      );
    }
  });

  /** 可见人物数量阈值：少于 2 人时关系网没意义，退化为简单网格 */
  const isMeaningful = $derived(characters.length >= 2);
</script>

{#if characters.length > 0}
  <section class="char-network-section" aria-labelledby="charnet-heading">
    <header class="charnet-header">
      <h2 id="charnet-heading" class="charnet-title">人物关系网</h2>
      <p class="charnet-sub">
        {#if isMeaningful}
          点击头像查看人物档案 · 红线 = 敌对 · 蓝线 = 同盟 · 虚线 = 中性
        {:else}
          材料里只有一位人物，暂无关系可展示
        {/if}
      </p>
    </header>

    {#if isMeaningful}
      <div class="charnet-canvas-wrap">
        <svg
          class="charnet-svg"
          viewBox="0 0 {VIEW_SIZE} {VIEW_SIZE}"
          aria-hidden="true"
          preserveAspectRatio="xMidYMid meet"
        >
          <!-- 背景同心圆：悬疑档案风的"标靶"装饰 -->
          <circle cx={CENTER} cy={CENTER} r={RADIUS + 100} class="ring-bg ring-1" />
          <circle cx={CENTER} cy={CENTER} r={RADIUS - 60} class="ring-bg ring-2" />

          <!-- 关系连线：先画线再画节点，节点盖住线头 -->
          {#each validRelations as rel, i (i + rel.from + rel.to)}
            {@const a = positions().get(rel.from)!}
            {@const b = positions().get(rel.to)!}
            {@const s = strokeFor(rel.sentiment)}
            {@const ns = normalizeSentiment(rel.sentiment)}
            <line
              x1={a.x}
              y1={a.y}
              x2={b.x}
              y2={b.y}
              stroke={s.stroke}
              stroke-width={s.width}
              stroke-dasharray={s.dash}
              stroke-linecap="round"
              class="rel-line"
              class:rel-neg={ns === 'negative'}
              class:rel-pos={ns === 'positive'}
            />
            <!-- 关系标签：在线段中点显示关系类型文字 -->
            {#if rel.label}
              <text
                x={(a.x + b.x) / 2}
                y={(a.y + b.y) / 2}
                class="edge-label"
                text-anchor="middle"
                dominant-baseline="central"
              >
                {rel.label}
              </text>
            {/if}
          {/each}

          <!-- 人物节点：头像圆 + 正下方名称/角色文字 -->
          {#each characters as character, i (character.id)}
            {@const p = positions().get(character.name)!}
            <g
              class="char-node"
              tabindex="0"
              role="button"
              aria-label="查看 {character.name} 档案"
              onclick={() => onSelect(character)}
              onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && onSelect(character)}
            >
              <!-- 头像外环：hover/focus 时高亮 -->
              <circle cx={p.x} cy={p.y} r={NODE_R + 4} class="node-halo" />
              <!-- 头像本体：金色描边卡片 -->
              <circle cx={p.x} cy={p.y} r={NODE_R} class="node-face" />
              <!-- 首字母 -->
              <text
                x={p.x}
                y={p.y}
                class="node-initial"
                style="font-size: {INITIAL_SIZE}px"
                text-anchor="middle"
                dominant-baseline="central"
              >
                {character.name.charAt(0)}
              </text>
              <!-- 名称文字：头像正下方，水平居中 -->
              <text
                x={p.x}
                y={p.y + LABEL_GAP}
                class="node-label"
                style="font-size: {LABEL_SIZE}px"
                text-anchor="middle"
                dominant-baseline="hanging"
              >
                {character.name}
              </text>
              <!-- 角色副标签：名称下方继续一行 -->
              <text
                x={p.x}
                y={p.y + LABEL_GAP + LINE_GAP}
                class="node-sublabel"
                style="font-size: {SUB_LABEL_SIZE}px"
                text-anchor="middle"
                dominant-baseline="hanging"
              >
                {character.role}
              </text>
            </g>
          {/each}
        </svg>
      </div>
    {:else}
      <!-- 单人兜底：横向一排头像卡 -->
      <ul class="charnet-fallback" role="list">
        {#each characters as character (character.id)}
          <li>
            <button type="button" class="fallback-chip" onclick={() => onSelect(character)}>
              <span class="fallback-avatar">{character.name.charAt(0)}</span>
              <span class="fallback-meta">
                <span class="fallback-name">{character.name}</span>
                <span class="fallback-role">{character.role}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .char-network-section {
    padding: clamp(2rem, 3vw, 4rem) clamp(1rem, 2vw, 2.5rem);
    background: var(--bg-primary, #0a0a0d);
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
  }

  .charnet-header {
    text-align: center;
    margin-bottom: 1.25rem;
  }

  .charnet-title {
    margin: 0 0 0.4rem;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: clamp(1.5rem, 2vw + 0.75rem, 2.25rem);
    color: var(--text-primary, #f6f6f3);
    letter-spacing: -0.01em;
  }

  .charnet-sub {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-secondary, #9a9a94);
  }

  /* SVG 画布：宽高比 1:1，最大宽度 860px */
  .charnet-canvas-wrap {
    margin: 0 auto;
    max-width: 860px;
    aspect-ratio: 1 / 1;
  }

  .charnet-svg {
    display: block;
    width: 100%;
    height: 100%;
  }

  /* 背景环 */
  .ring-bg {
    fill: none;
    stroke: var(--border, rgba(255, 255, 255, 0.08));
    stroke-width: 1;
  }
  .ring-bg.ring-1 {
    stroke-dasharray: 2 8;
    opacity: 0.5;
  }
  .ring-bg.ring-2 {
    opacity: 0.3;
  }

  /* 关系线 */
  .rel-line {
    opacity: 0.7;
    transition: opacity 200ms ease;
  }
  .rel-line.rel-neg {
    filter: drop-shadow(0 0 4px rgba(230, 83, 79, 0.35));
  }
  .rel-line.rel-pos {
    filter: drop-shadow(0 0 4px rgba(108, 160, 222, 0.3));
  }

  /* 关系线标签 */
  .edge-label {
    font-size: 14px;
    font-family: 'Noto Serif SC', 'Source Serif Pro', Georgia, serif;
    fill: var(--text-secondary, #9a9a94);
    pointer-events: none;
    opacity: 0.85;
    transform: translateY(-8px);
  }

  /* 节点 group */
  .char-node {
    cursor: pointer;
    outline: none;
    transition: transform 200ms ease;
  }
  .char-node:hover,
  .char-node:focus-visible {
    transform: translateY(-2px);
  }
  .char-node:focus-visible .node-halo {
    stroke: var(--accent-hot, #e6534f);
    stroke-width: 3;
  }

  .node-halo {
    fill: var(--bg-primary, #0a0a0d);
    stroke: var(--border, rgba(255, 255, 255, 0.2));
    stroke-width: 1.5;
    transition: stroke 200ms ease;
  }
  .char-node:hover .node-halo {
    stroke: var(--accent-hot, #e6534f);
    stroke-width: 2.5;
  }

  /* 头像本体：金色描边卡片 */
  .node-face {
    fill: var(--bg-card, #17171c);
    stroke: var(--accent-gold, #d4a851);
    stroke-width: 2;
  }

  /* 首字母 */
  .node-initial {
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-weight: 700;
    fill: var(--text-primary, #f6f6f3);
    pointer-events: none;
  }

  /* 名称文字：头像正下方居中 */
  .node-label {
    font-weight: 600;
    fill: var(--text-primary, #f6f6f3);
    pointer-events: none;
    /* 给文字加一层半透明暗底，防止与穿过该区域的连线混在一起 */
    paint-order: stroke fill;
    stroke: var(--bg-primary, #0a0a0d);
    stroke-width: 3px;
    stroke-linejoin: round;
  }

  /* 角色副标签 */
  .node-sublabel {
    fill: var(--text-secondary, #9a9a94);
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    pointer-events: none;
    paint-order: stroke fill;
    stroke: var(--bg-primary, #0a0a0d);
    stroke-width: 2px;
    stroke-linejoin: round;
  }

  /* 单人兜底的 fallback 列表 */
  .charnet-fallback {
    display: flex;
    justify-content: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .fallback-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.9rem 0.5rem 0.5rem;
    border-radius: 999px;
    background: var(--bg-card, #17171c);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
    color: var(--text-primary, #f6f6f3);
    cursor: pointer;
    transition: border-color 120ms ease, transform 120ms ease;
  }
  .fallback-chip:hover,
  .fallback-chip:focus-visible {
    border-color: var(--accent-hot, #e6534f);
    outline: none;
    transform: translateY(-1px);
  }

  .fallback-avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--accent-hot, #e6534f), var(--accent-gold, #d4a851));
    color: #fff;
    font-weight: 700;
  }

  .fallback-meta {
    display: inline-flex;
    flex-direction: column;
    line-height: 1.2;
    text-align: left;
  }

  .fallback-name {
    font-size: 0.88rem;
    font-weight: 600;
  }

  .fallback-role {
    font-size: 0.7rem;
    color: var(--text-secondary, #9a9a94);
  }

  /* 尊重 reduced-motion */
  @media (prefers-reduced-motion: reduce) {
    .char-node,
    .fallback-chip,
    .rel-line {
      transition: none;
    }
  }
</style>