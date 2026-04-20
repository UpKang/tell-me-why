<script lang="ts">
  /**
   * ResultView - 吃瓜版结果页容器（Phase 2 扩展版）
   *
   * 作用：
   * - 接收后端 TruthAnalysisResult + 人物画像 + 长文 HTML
   * - 按"吃瓜阅读顺序"组合各模块：
   *     1. Hero：一句话锐评
   *     2. 人物关系网（点击头像 → Drawer 显示完整画像）
   *     3. 事件时间线
   *     4. 金句卡
   *     5. 阵营站队
   *     6. 赢家 vs 输家
   *     7. 关键反转
   *     8. 长文回退区（折叠）
   *
   * - 模块只在对应字段有内容时显示（自己判断 `xxx.length > 0`）
   * - 人物画像以 Drawer 形式内嵌（不再需要单独的"画像页"）
   */

  import type { CharacterProfile } from '../CharacterProfilePanel.svelte';
  import type { TruthExtras } from './types';
  import { emptyExtras, hasRichExtras } from './types';
  import { revealOnScroll } from '../../actions/revealOnScroll';
  import BackButton from '../BackButton.svelte';
  import VerdictHero from './VerdictHero.svelte';
  import CharacterNetwork from './CharacterNetwork.svelte';
  import CharacterDrawer from './CharacterDrawer.svelte';
  import TimelineSection from './TimelineSection.svelte';
  import HotQuotesCarousel from './HotQuotesCarousel.svelte';
  import FactionsSplit from './FactionsSplit.svelte';
  import WinnersLosersBoard from './WinnersLosersBoard.svelte';
  import PlotTwistCard from './PlotTwistCard.svelte';
  import LongformFallback from './LongformFallback.svelte';

  interface Props {
    /** 吃瓜结构化包；可能为 null（老记录 / 模型没吐 JSON） */
    extras: TruthExtras | null | undefined;
    /** 已净化的长文 HTML（renderContent 出来的） */
    longformHtml: string;
    /** 人物画像列表 */
    characters: CharacterProfile[];
    /** 材料条目数（用于 Hero meta 展示） */
    materialCount: number;
    /** 操作按钮回调 */
    onRestart: () => void;
    onBackHome: () => void;
    onCopy: () => void;
    onExport: () => void;
    /** 是否正在导出（按钮 loading 态） */
    isExporting: boolean;
    /** 导出错误提示 */
    exportError: string;
    /**
     * 返回按钮的文案
     * - 从首页进来时传 "返回首页"
     * - 从历史页进来时传 "返回历史"
     * 默认 "返回首页" 保持向后兼容。
     */
    backLabel?: string;
  }

  const {
    extras,
    longformHtml,
    characters,
    materialCount,
    onRestart,
    onBackHome,
    onCopy,
    onExport,
    isExporting,
    exportError,
    backLabel = '返回首页',
  }: Props = $props();

  // 有没有"吃瓜料"决定了长文是否默认展开。没料时长文就是全部。
  const isRich = $derived(hasRichExtras(extras));
  // 安全拿 extras：传进来 null 的话用空壳兜底
  const safeExtras = $derived<TruthExtras>(extras ?? emptyExtras());
  // Hero 副标签：给用户一点数据感
  const heroMeta = $derived(`基于 ${materialCount} 条材料 · ${characters.length} 位人物`);

  // ========== 人物 Drawer 状态 ==========
  // 点击 CharacterNetwork 节点 → 打开 Drawer；关闭 = null
  let selectedCharacter: CharacterProfile | null = $state(null);

  function openCharacter(c: CharacterProfile): void {
    selectedCharacter = c;
  }

  function closeCharacter(): void {
    selectedCharacter = null;
  }
</script>

<div class="result-view">
  <!-- 悬疑档案风的 grain 噪点背景（纯装饰层，不影响布局） -->
  <div class="grain-layer" aria-hidden="true"></div>

  <!-- ============== 顶部工具栏：返回 / 重新分析 / 复制 / 导出 ==============
       - 复用全局 .sticky-topbar 工具类，保证与首页 / 历史页的固定行为一致
       - 左侧：返回（动态文案，由 backLabel 决定"返回首页"还是"返回历史"）
       - 右侧：三个操作按钮（重新分析 / 复制 / 导出），主操作（导出）用 primary -->
  <nav class="result-topbar sticky-topbar" aria-label="结果操作">
    <!-- 返回按钮使用通用 BackButton 组件，视觉跟历史页保持一致 -->
    <BackButton label={backLabel} onClick={onBackHome} />

    <div class="topbar-actions">
      <button type="button" class="topbar-btn ghost" onclick={onRestart}>
        重新分析
      </button>
      <button type="button" class="topbar-btn ghost" onclick={onCopy}>
        复制结果
      </button>
      <button
        type="button"
        class="topbar-btn primary"
        onclick={onExport}
        disabled={isExporting}
      >
        {isExporting ? '导出中...' : '导出 HTML'}
      </button>
    </div>
  </nav>

  {#if exportError}
    <div class="result-export-error" role="alert">{exportError}</div>
  {/if}

  <!-- ============== Hero：一句话锐评（Hero 首屏不做 reveal 动画，避免首次看到空白） ============== -->
  <VerdictHero verdict={safeExtras.oneLinerVerdict} meta={heroMeta} />

  <!-- ============== 人物关系网（替代旧的人物条） ============== -->
  {#if characters.length > 0}
    <div use:revealOnScroll>
      <CharacterNetwork
        characters={characters}
        relations={safeExtras.characterRelations}
        onSelect={openCharacter}
      />
    </div>
  {/if}

  <!-- ============== 时间线 ============== -->
  <div use:revealOnScroll>
    <TimelineSection events={safeExtras.timelineEvents} />
  </div>

  <!-- ============== 金句卡 ============== -->
  <div use:revealOnScroll>
    <HotQuotesCarousel quotes={safeExtras.hotQuotes} />
  </div>

  <!-- ============== 阵营站队 ============== -->
  <div use:revealOnScroll>
    <FactionsSplit factions={safeExtras.factions} />
  </div>

  <!-- ============== 赢家 vs 输家 ============== -->
  <div use:revealOnScroll>
    <WinnersLosersBoard data={safeExtras.winnersLosers} />
  </div>

  <!-- ============== 关键反转 ============== -->
  <div use:revealOnScroll>
    <PlotTwistCard twists={safeExtras.plotTwists} />
  </div>

  <!-- ============== 长文回退区 ============== -->
  <div use:revealOnScroll>
    <LongformFallback html={longformHtml} defaultOpen={!isRich} />
  </div>
</div>

<!-- Drawer 作为 sibling 放在 .result-view 外，保证 fixed 定位不被父级 transform 限制 -->
<CharacterDrawer character={selectedCharacter} onClose={closeCharacter} />

<style>
  .result-view {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    min-height: 100%;
    background: var(--bg-primary, #0a0a0d);
    color: var(--text-primary, #f6f6f3);
    /* 为 grain-layer 铺底 */
    isolation: isolate;
  }

  /* ========== Grain 噪点背景（悬疑档案风关键装饰） ========== */
  /*
   * 使用 SVG turbulence 生成极轻的噪点纹理，叠在整个结果页上。
   * 仅作为视觉氛围层，不承担语义，不参与交互。
   * 关键：pointer-events: none，避免阻挡点击；z-index: 0 铺底
   */
  .grain-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 0;
    opacity: 0.35;
    mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='120' height='120'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 0.04  0 0 0 0 0.04  0 0 0 0 0.04  0 0 0 0.6 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
    background-size: 200px 200px;
  }

  /* 所有 section 应该相对 grain 层上浮（z-index: 1 > grain 的 z-index: 0）
   *
   * 为什么要排除 .sticky-topbar？
   * ---------------------------
   * 这条规则原本会把 .result-topbar 也设成 position: relative，
   * 从而把全局 .sticky-topbar 工具类的 `position: sticky` 覆盖掉，
   * 让顶栏在滚动时无法固定。
   * .sticky-topbar 自己有 z-index: 50，本来就浮在 grain 上面，
   * 不需要外部再强制 position: relative，所以这里直接把它排除。
   */
  .result-view > :global(*:not(.grain-layer):not(.sticky-topbar)) {
    position: relative;
    z-index: 1;
  }

  /* ========== 顶部工具栏 ==========
     sticky / 背景 / 磨砂 / 边线由全局 .sticky-topbar 工具类提供。
     这里只管布局：两端对齐，右侧按钮组紧凑排列。 */
  .result-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding: 0.75rem clamp(1rem, 2vw, 2rem);
  }

  /* 右侧操作按钮组 */
  .topbar-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .topbar-btn {
    padding: 0.4rem 0.9rem;
    border-radius: 6px;
    font-size: 0.8rem;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }

  .topbar-btn.ghost {
    background: transparent;
    color: var(--text-secondary, #c4c4bd);
    border-color: var(--border, rgba(255, 255, 255, 0.2));
  }

  .topbar-btn.ghost:hover,
  .topbar-btn.ghost:focus-visible {
    background: var(--bg-card, #17171c);
    color: var(--text-primary, #f6f6f3);
    border-color: var(--accent-hot, #e6534f);
    outline: none;
  }

  .topbar-btn.primary {
    background: var(--accent-hot, #e6534f);
    color: #fff;
    border-color: var(--accent-hot, #e6534f);
  }

  .topbar-btn.primary:hover:not(:disabled),
  .topbar-btn.primary:focus-visible:not(:disabled) {
    background: var(--accent-hot-strong, #d13c38);
    border-color: var(--accent-hot-strong, #d13c38);
    outline: none;
  }

  .topbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .result-export-error {
    margin: 0.75rem clamp(1rem, 2vw, 2rem) 0;
    padding: 0.6rem 0.9rem;
    background: rgba(230, 83, 79, 0.1);
    color: var(--accent-hot, #e6534f);
    border-radius: 6px;
    font-size: 0.85rem;
    border: 1px solid rgba(230, 83, 79, 0.3);
    position: relative;
    z-index: 1;
  }

  /* Reduced motion：关掉装饰性动效 */
  @media (prefers-reduced-motion: reduce) {
    .grain-layer {
      opacity: 0.2;
    }
  }
</style>
