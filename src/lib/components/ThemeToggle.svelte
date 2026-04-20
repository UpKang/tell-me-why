<script lang="ts">
  /**
   * ThemeToggle - 主题切换按钮
   *
   * 交互设计：
   * - 单击按钮弹出下拉菜单，展示三个选项：浅色 / 暗黑 / 跟随系统
   * - 当前模式带有选中态标识
   * - 按钮本身展示当前解析后的主题图标（太阳 / 月亮），让用户一眼看出此刻是什么主题
   *
   * 为什么用下拉菜单而不是"循环点击"：
   * - 三态循环顺序（light→dark→system）用户不容易预测
   * - 显式下拉让用户一次看清所有选项，符合设置类交互习惯
   */

  import { themeMode, resolvedTheme, setThemeMode, type ThemeMode } from '../stores/theme';

  /** 下拉菜单是否打开 */
  let isOpen = $state(false);

  /** 按钮根元素引用（用于点击外部关闭菜单） */
  let rootEl: HTMLDivElement | undefined = $state();

  /** 订阅当前用户选择的模式 */
  let currentMode = $state<ThemeMode>('system');
  themeMode.subscribe((v) => {
    currentMode = v;
  });

  /** 订阅当前实际生效的主题（light / dark），用于选择按钮图标 */
  let current = $state<'light' | 'dark'>('light');
  resolvedTheme.subscribe((v) => {
    current = v;
  });

  /**
   * 选项定义：按显示顺序列出
   * labelZh 是中文显示名，value 是写入 store 的值
   */
  const options: ReadonlyArray<{ value: ThemeMode; labelZh: string }> = [
    { value: 'light', labelZh: '浅色' },
    { value: 'dark', labelZh: '暗黑' },
    { value: 'system', labelZh: '跟随系统' },
  ];

  /**
   * 切换菜单展开 / 收起
   */
  function toggleMenu(): void {
    isOpen = !isOpen;
  }

  /**
   * 选中某个主题模式
   * 切换完成后关闭菜单
   */
  function selectMode(mode: ThemeMode): void {
    setThemeMode(mode);
    isOpen = false;
  }

  /**
   * 监听全局点击，点到按钮外部时关闭菜单
   * 使用 $effect 在组件挂载后注册、卸载时清理
   */
  $effect(() => {
    if (!isOpen) return;
    function handleClickOutside(e: MouseEvent): void {
      if (rootEl && !rootEl.contains(e.target as Node)) {
        isOpen = false;
      }
    }
    document.addEventListener('click', handleClickOutside);
    return () => document.removeEventListener('click', handleClickOutside);
  });
</script>

<div class="theme-toggle" bind:this={rootEl}>
  <button
    class="theme-toggle-btn"
    onclick={toggleMenu}
    aria-label="切换主题"
    aria-haspopup="menu"
    aria-expanded={isOpen}
    title="切换主题"
  >
    {#if current === 'dark'}
      <!-- 月亮图标：暗黑模式下显示 -->
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
      </svg>
    {:else}
      <!-- 太阳图标：浅色模式下显示 -->
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <circle cx="12" cy="12" r="4"/>
        <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41"/>
      </svg>
    {/if}
  </button>

  {#if isOpen}
    <div class="theme-menu" role="menu">
      {#each options as opt (opt.value)}
        <button
          class="theme-menu-item"
          class:active={currentMode === opt.value}
          role="menuitemradio"
          aria-checked={currentMode === opt.value}
          onclick={() => selectMode(opt.value)}
        >
          <span class="theme-menu-label">{opt.labelZh}</span>
          {#if currentMode === opt.value}
            <!-- 选中态的勾号 -->
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <polyline points="20 6 9 17 4 12"/>
            </svg>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* 按钮容器：定位上下文，供下拉菜单绝对定位 */
  .theme-toggle {
    position: relative;
    display: inline-flex;
  }

  /* 图标按钮：复用与 history-btn 相近的极简风格 */
  .theme-toggle-btn {
    padding: 6px 8px;
    border-radius: 4px;
    background-color: transparent;
    color: var(--text-secondary);
    border: none;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .theme-toggle-btn:hover {
    color: var(--text-primary);
    background-color: var(--bg-card);
  }

  /* 下拉菜单 */
  .theme-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    min-width: 120px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    padding: 4px;
    z-index: 50;
    display: flex;
    flex-direction: column;
  }

  .theme-menu-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 0.78rem;
    cursor: pointer;
    text-align: left;
    transition: background-color 0.12s, color 0.12s;
  }

  .theme-menu-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* 选中态：文字使用主色调，勾号颜色也继承 */
  .theme-menu-item.active {
    color: var(--text-primary);
    font-weight: 500;
  }

  .theme-menu-label {
    letter-spacing: -0.01em;
  }
</style>
