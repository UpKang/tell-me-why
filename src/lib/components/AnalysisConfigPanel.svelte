<script lang="ts">
  /**
   * AnalysisConfigPanel 组件 - 分析参数配置面板（紧凑下拉样式）
   *
   * 设计理念：
   * - 横向排列在输入框下方，每个参数点击弹出下拉小框选择
   * - 紧凑、专业，类似 ChatGPT/Claude 的参数设置条
   * - 不占用独立右侧面板空间
   *
   * 功能：
   * - 温度：下拉选择预设档位
   * - 公正严明程度：下拉选择档位
   * - 道德底线：下拉选择档位
   * - 输出格式：下拉选择
   * - 分析深度：下拉选择
   */

  // ========== 类型定义 ==========

  /**
   * 分析参数配置结构
   */
  export interface AnalysisConfig {
    /** 温度：0.1（保守）- 1.0（创意） */
    temperature: number;
    /** 公正严明程度：0-100 */
    fairness: number;
    /** 道义底线：0-100 */
    morality: number;
    /** 输出格式 */
    outputFormat: 'summary' | 'detailed' | 'list' | 'table';
    /** 分析深度 */
    analysisDepth: 'surface' | 'medium' | 'deep';
  }

  // ========== Props ==========

  interface Props {
    /** 当前配置（由父组件管理） */
    config: AnalysisConfig;
    /** 配置变更回调 */
    onChange: (config: AnalysisConfig) => void;
  }

  let { config, onChange }: Props = $props();

  // ========== 下拉选项预设 ==========

  /**
   * 温度档位选项
   * 简化为 5 个档位，方便快速选择
   */
  const temperatureOptions = [
    { value: 0.1, label: '0.1 · 极保守' },
    { value: 0.3, label: '0.3 · 保守' },
    { value: 0.5, label: '0.5 · 平衡（推荐）' },
    { value: 0.7, label: '0.7 · 创意' },
    { value: 1.0, label: '1.0 · 极创意' },
  ];

  /**
   * 公正严明程度档位选项
   */
  const fairnessOptions = [
    { value: 20, label: '自由判断' },
    { value: 40, label: '允许主观' },
    { value: 60, label: '较为中立' },
    { value: 80, label: '极度客观（推荐）' },
    { value: 100, label: '绝对客观' },
  ];

  /**
   * 道义底线档位选项
   */
  const moralityOptions = [
    { value: 20, label: '宽松处理' },
    { value: 40, label: '适度放宽' },
    { value: 60, label: '一般遵守（推荐）' },
    { value: 80, label: '高度遵守' },
    { value: 100, label: '严格遵守' },
  ];

  /**
   * 输出格式选项
   */
  const outputFormatOptions = [
    { value: 'summary', label: '简洁摘要' },
    { value: 'detailed', label: '详细报告（推荐）' },
    { value: 'list', label: '分条列举' },
    { value: 'table', label: '表格对比' },
  ];

  /**
   * 分析深度选项
   */
  const analysisDepthOptions = [
    { value: 'surface', label: '表面分析' },
    { value: 'medium', label: '中等深度（推荐）' },
    { value: 'deep', label: '深度挖掘' },
  ];

  // ========== 下拉框状态管理 ==========

  /**
   * 当前展开的下拉框名称
   * null 表示全部收起，只允许同时展开一个下拉框
   */
  let activeDropdown: string | null = $state(null);

  /**
   * 切换下拉框展开状态
   * 点击同一个参数：收起；点击不同参数：切换到新参数
   */
  function toggleDropdown(name: string, event: MouseEvent): void {
    event.stopPropagation(); // 阻止冒泡，避免触发 window onclick 关闭下拉
    activeDropdown = activeDropdown === name ? null : name;
  }

  /**
   * 关闭所有下拉框（window onclick 触发）
   */
  function closeAllDropdowns(): void {
    activeDropdown = null;
  }

  /**
   * 阻止下拉框内部点击冒泡（防止点击选项时关闭下拉）
   */
  function stopBubble(event: MouseEvent): void {
    event.stopPropagation();
  }

  // ========== 选择方法 ==========

  /**
   * 选择温度档位
   */
  function selectTemperature(value: number): void {
    onChange({ ...config, temperature: value });
    closeAllDropdowns();
  }

  /**
   * 选择公正严明程度档位
   */
  function selectFairness(value: number): void {
    onChange({ ...config, fairness: value });
    closeAllDropdowns();
  }

  /**
   * 选择道义底线档位
   */
  function selectMorality(value: number): void {
    onChange({ ...config, morality: value });
    closeAllDropdowns();
  }

  /**
   * 选择输出格式
   */
  function selectOutputFormat(value: AnalysisConfig['outputFormat']): void {
    onChange({ ...config, outputFormat: value });
    closeAllDropdowns();
  }

  /**
   * 选择分析深度
   */
  function selectAnalysisDepth(value: AnalysisConfig['analysisDepth']): void {
    onChange({ ...config, analysisDepth: value });
    closeAllDropdowns();
  }

  // ========== 显示标签 ==========

  /**
   * 获取当前温度的显示标签
   */
  function getTemperatureLabel(): string {
    const opt = temperatureOptions.find(o => o.value === config.temperature);
    return opt ? opt.label : `${config.temperature}`;
  }

  /**
   * 获取当前公正严明程度的显示标签
   */
  function getFairnessLabel(): string {
    const opt = fairnessOptions.find(o => o.value === config.fairness);
    return opt ? opt.label : `${config.fairness}`;
  }

  /**
   * 获取当前道义底线的显示标签
   */
  function getMoralityLabel(): string {
    const opt = moralityOptions.find(o => o.value === config.morality);
    return opt ? opt.label : `${config.morality}`;
  }

  /**
   * 获取当前输出格式的显示标签
   */
  function getOutputFormatLabel(): string {
    const opt = outputFormatOptions.find(o => o.value === config.outputFormat);
    return opt ? opt.label : config.outputFormat;
  }

  /**
   * 获取当前分析深度的显示标签
   */
  function getAnalysisDepthLabel(): string {
    const opt = analysisDepthOptions.find(o => o.value === config.analysisDepth);
    return opt ? opt.label : config.analysisDepth;
  }
</script>

<!-- 点击空白区域关闭下拉框 -->
<svelte:window onclick={closeAllDropdowns} />

<!-- 参数设置条：横向排列 -->
<div class="config-bar">
  <!-- 温度 -->
  <div class="param-pill" class:active={activeDropdown === 'temperature'}>
    <button
      class="pill-btn"
      onclick={(e) => toggleDropdown('temperature', e)}
      aria-label="设置温度"
    >
      <span class="pill-label">温度</span>
      <span class="pill-value">{getTemperatureLabel()}</span>
      <svg class="pill-arrow" width="10" height="10" viewBox="0 0 10 10">
        <path d="M2 4l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.2"/>
      </svg>
    </button>

    <!-- 下拉选项框 -->
    {#if activeDropdown === 'temperature'}
      <div class="dropdown-menu" onclick={stopBubble}>
        {#each temperatureOptions as opt (opt.value)}
          <button
            class="dropdown-item {config.temperature === opt.value ? 'selected' : ''}"
            onclick={() => selectTemperature(opt.value)}
          >
            {opt.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- 公正严明程度 -->
  <div class="param-pill" class:active={activeDropdown === 'fairness'}>
    <button
      class="pill-btn"
      onclick={(e) => toggleDropdown('fairness', e)}
      aria-label="设置公正严明程度"
    >
      <span class="pill-label">公正</span>
      <span class="pill-value">{getFairnessLabel()}</span>
      <svg class="pill-arrow" width="10" height="10" viewBox="0 0 10 10">
        <path d="M2 4l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.2"/>
      </svg>
    </button>

    {#if activeDropdown === 'fairness'}
      <div class="dropdown-menu" onclick={stopBubble}>
        {#each fairnessOptions as opt (opt.value)}
          <button
            class="dropdown-item {config.fairness === opt.value ? 'selected' : ''}"
            onclick={() => selectFairness(opt.value)}
          >
            {opt.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- 道义底线 -->
  <div class="param-pill" class:active={activeDropdown === 'morality'}>
    <button
      class="pill-btn"
      onclick={(e) => toggleDropdown('morality', e)}
      aria-label="设置道义底线"
    >
      <span class="pill-label">道义</span>
      <span class="pill-value">{getMoralityLabel()}</span>
      <svg class="pill-arrow" width="10" height="10" viewBox="0 0 10 10">
        <path d="M2 4l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.2"/>
      </svg>
    </button>

    {#if activeDropdown === 'morality'}
      <div class="dropdown-menu" onclick={stopBubble}>
        {#each moralityOptions as opt (opt.value)}
          <button
            class="dropdown-item {config.morality === opt.value ? 'selected' : ''}"
            onclick={() => selectMorality(opt.value)}
          >
            {opt.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- 输出格式 -->
  <div class="param-pill" class:active={activeDropdown === 'outputFormat'}>
    <button
      class="pill-btn"
      onclick={(e) => toggleDropdown('outputFormat', e)}
      aria-label="设置输出格式"
    >
      <span class="pill-label">格式</span>
      <span class="pill-value">{getOutputFormatLabel()}</span>
      <svg class="pill-arrow" width="10" height="10" viewBox="0 0 10 10">
        <path d="M2 4l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.2"/>
      </svg>
    </button>

    {#if activeDropdown === 'outputFormat'}
      <div class="dropdown-menu" onclick={stopBubble}>
        {#each outputFormatOptions as opt (opt.value)}
          <button
            class="dropdown-item {config.outputFormat === opt.value ? 'selected' : ''}"
            onclick={() => selectOutputFormat(opt.value)}
          >
            {opt.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- 分析深度 -->
  <div class="param-pill" class:active={activeDropdown === 'analysisDepth'}>
    <button
      class="pill-btn"
      onclick={(e) => toggleDropdown('analysisDepth', e)}
      aria-label="设置分析深度"
    >
      <span class="pill-label">深度</span>
      <span class="pill-value">{getAnalysisDepthLabel()}</span>
      <svg class="pill-arrow" width="10" height="10" viewBox="0 0 10 10">
        <path d="M2 4l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.2"/>
      </svg>
    </button>

    {#if activeDropdown === 'analysisDepth'}
      <div class="dropdown-menu" onclick={stopBubble}>
        {#each analysisDepthOptions as opt (opt.value)}
          <button
            class="dropdown-item {config.analysisDepth === opt.value ? 'selected' : ''}"
            onclick={() => selectAnalysisDepth(opt.value)}
          >
            {opt.label}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  /* ========== 参数设置条 ========== */

  .config-bar {
    /* 横向排列，居中，撑满宽度 */
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    flex-wrap: nowrap;
    overflow: visible;
    padding: 0;
  }

  /* 单个参数胶囊按钮 */
  .param-pill {
    position: relative;
  }

  /* 激活状态：下拉框展开时 */
  .param-pill.active .pill-btn {
    border-color: var(--accent);
    background: var(--accent-light);
  }

  /* 胶囊按钮 */
  .pill-btn {
    /* 圆角胶囊样式 */
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--bg-card);
    color: var(--text-primary);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.72rem;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .pill-btn:hover {
    border-color: #ccc;
    background: var(--bg-hover);
  }

  /* 参数名称标签 */
  .pill-label {
    font-weight: 500;
    color: var(--text-secondary);
  }

  /* 当前选中值 */
  .pill-value {
    color: var(--text-primary);
    font-weight: 600;
  }

  /* 下拉箭头 */
  .pill-arrow {
    color: var(--text-muted);
    margin-left: 2px;
  }

  /* ========== 下拉选项框 ========== */

  .dropdown-menu {
    /* 绝对定位，在按钮下方弹出 */
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    min-width: 160px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: var(--shadow-lg);
    z-index: 100;
    padding: 4px 0;
    animation: dropdownFadeIn 0.12s ease;
  }

  @keyframes dropdownFadeIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* 下拉选项项 */
  .dropdown-item {
    display: block;
    width: 100%;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: var(--text-primary);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.72rem;
    text-align: left;
    transition: background 0.1s;
  }

  .dropdown-item:hover {
    background: var(--bg-hover);
  }

  /* 当前选中项高亮 */
  .dropdown-item.selected {
    background: var(--accent-light);
    color: var(--accent);
    font-weight: 600;
  }
</style>