<script lang="ts">
  /**
   * BackButton - 通用返回按钮
   *
   * 为什么单独抽一个组件？
   * ------------------------
   * 项目里有多个页面需要"返回"按钮（历史页、详情页、未来可能还有分析页），
   * 之前每个地方都自己写一个 `← 返回` 的 <button>，结果：
   *   - 视觉不统一（字号、颜色、hover 动效各不相同）
   *   - 无障碍属性（aria-label / focus-visible）写得参差不齐
   *   - 图标改一次要改很多地方
   *
   * 抽成独立组件后：
   *   - 图标用内联 SVG（不依赖外部 icon 库，导出 HTML 时也不会丢）
   *   - hover 时箭头左移 + 背景加深，形成统一的"可点击+方向性"反馈
   *   - 支持 label 自定义（"返回首页" / "返回历史" / "返回上一步"）
   *   - 支持 ghost / solid 两种外观（对齐 ResultView topbar 的设计）
   *
   * 使用示例：
   *   <BackButton label="返回首页" onClick={goHome} />
   *   <BackButton label="返回历史" onClick={goHistory} variant="solid" />
   */

  interface Props {
    /** 按钮文案（默认 "返回"），建议带目的地，如 "返回首页"、"返回历史" */
    label?: string;
    /** 点击回调 */
    onClick: () => void;
    /**
     * 外观样式：
     * - ghost（默认）：透明背景 + 细边框，适合融入 sticky 顶栏
     * - solid：实色背景，适合需要强调的场景（目前未必用到，留口）
     */
    variant?: 'ghost' | 'solid';
    /** 自定义 aria-label（不传则回落到 label） */
    ariaLabel?: string;
  }

  const {
    label = '返回',
    onClick,
    variant = 'ghost',
    ariaLabel,
  }: Props = $props();
</script>

<button
  type="button"
  class="back-btn back-btn-{variant}"
  onclick={onClick}
  aria-label={ariaLabel ?? label}
>
  <!-- 使用内联 SVG 绘制左箭头；stroke="currentColor" 让颜色跟随文字颜色
       这样在明暗主题切换 + hover 态下都不用单独定义颜色 -->
  <svg
    class="back-btn-icon"
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path d="M19 12H5" />
    <path d="M12 19l-7-7 7-7" />
  </svg>
  <span class="back-btn-label">{label}</span>
</button>

<style>
  /* ========== 基础按钮样式（布局 + 字体 + 过渡） ========== */
  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px 6px 10px;
    border-radius: 6px;
    font-size: 0.82rem;
    font-weight: 500;
    line-height: 1;
    cursor: pointer;
    /* 让箭头的位移动效单独配一条曲线，避免整体晃动 */
    transition: background-color 160ms ease,
                color 160ms ease,
                border-color 160ms ease;
    letter-spacing: -0.01em;
    /* 避免被父容器的 text-align 影响 */
    text-align: left;
  }

  /* ghost 变体：透明背景 + 细边框，适合放在 sticky 顶栏里 */
  .back-btn-ghost {
    background: transparent;
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }

  .back-btn-ghost:hover,
  .back-btn-ghost:focus-visible {
    background: var(--bg-card);
    color: var(--text-primary);
    border-color: color-mix(in srgb, var(--text-muted) 50%, var(--border));
    outline: none;
  }

  /* solid 变体：实色背景（预留给需要强调的场景） */
  .back-btn-solid {
    background: var(--accent);
    color: var(--bg-primary);
    border: 1px solid var(--accent);
  }

  .back-btn-solid:hover,
  .back-btn-solid:focus-visible {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
    outline: none;
  }

  /* ========== 图标动效 ==========
     hover 时箭头左移 2px，形成"向左回退"的方向暗示
     transform 比 margin-left 动更流畅（不会触发 layout 计算） */
  .back-btn-icon {
    flex-shrink: 0;
    transition: transform 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .back-btn:hover .back-btn-icon,
  .back-btn:focus-visible .back-btn-icon {
    transform: translateX(-2px);
  }

  .back-btn-label {
    white-space: nowrap;
  }

  /* reduced-motion 用户：关掉图标位移 */
  @media (prefers-reduced-motion: reduce) {
    .back-btn-icon {
      transition: none;
    }
    .back-btn:hover .back-btn-icon,
    .back-btn:focus-visible .back-btn-icon {
      transform: none;
    }
  }
</style>
