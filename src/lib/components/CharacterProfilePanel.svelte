<script lang="ts">
  /**
   * CharacterProfilePanel 组件 - 人物画像展示面板（纯展示模式）
   *
   * 功能：
   * - 左侧：显示所有人物列表（可滚动）
   * - 右侧：显示选中人物的可能动机（可滚动，纯展示）
   * - 点击开始分析后，将所有动机传给大模型分析
   *
   * 页面流转：
   * 1. 首页点击开始分析 → 人物画像生成
   * 2. 人物画像显示 → 查看人物详情和动机（不需要选择）
   * 3. 点击开始分析 → 深度分析（大模型自行判断动机）
   * 4. 结果页显示 → 可返回重新分析
   */

  // ========== 类型定义 ==========

  /**
   * 动机选项结构
   */
  export interface Motivation {
    /** 动机唯一 ID */
    id: string;
    /** 动机描述文本 */
    content: string;
    /** AI 给出的可信度评分（0-100） - 不显示给用户 */
    confidence: number;
    /** 来源提示（可选） */
    sourceHint?: string;
  }

  /**
   * 人物画像结构
   */
  export interface CharacterProfile {
    /** 人物唯一 ID */
    id: string;
    /** 人物名称 */
    name: string;
    /** 角色（当事人、目击者、旁观者等） */
    role: string;
    /** 人物描述 */
    description: string;
    /** 可选动机列表 */
    motivations: Motivation[];
    /** 用户选中的动机 ID（内部状态） */
    selectedMotivationId?: string;
  }

  /**
   * 人物画像生成结果
   */
  export interface CharacterGenerationResult {
    /** 人物画像列表 */
    characters: CharacterProfile[];
    /** 是否成功提取人物 */
    hasCharacters: boolean;
    /** 错误提示 */
    errorMessage?: string;
    /** 事件梳理摘要（包含时间线、关键事实、信息冲突点等） */
    eventSummary?: string;
  }

  // ========== Props ==========

  interface Props {
    /** 人物画像列表 */
    characters: CharacterProfile[];
    /** 事件梳理摘要 */
    eventSummary?: string;
    /** 开始分析回调（直接开始，不需要用户选择动机） */
    onStartAnalysis: () => void;
    /** 返回首页回调 */
    onGoBack: () => void;
    /** 是否正在分析 */
    isAnalyzing?: boolean;
  }

  let { characters, eventSummary = '', onStartAnalysis, onGoBack, isAnalyzing = false }: Props = $props();

  // ========== 内部状态 ==========

  /**
   * 当前选中的人物 ID（左侧列表点击选中，用于查看详情）
   * 注意：这是纯展示模式，不再需要用户选择动机
   */
  let selectedCharacterId: string = $state('');

  // ========== 状态初始化与重置 ==========

  /**
   * 当 characters prop 变化时，重置选中状态并自动选中第一个人物
   *
   * 触发时机：
   * - characters 从有值变为空（返回首页）
   * - characters 内容变化（新分析开始）
   * - characters 馍次传入（首次进入页面）
   */
  $effect(() => {
    // 通过访问 characters 属性建立依赖追踪
    const characterCount = characters?.length ?? 0;
    const firstCharId = characters?.[0]?.id;

    // 重置选中状态
    selectedCharacterId = '';

    // 自动选中第一个人物
    if (characters && characters.length > 0) {
      selectedCharacterId = characters[0].id;
    }
  });

  /**
   * 获取当前选中的人物对象
   */
  function getSelectedCharacter(): CharacterProfile | undefined {
    return characters.find(c => c.id === selectedCharacterId);
  }

  // ========== 辅助方法 ==========

  /**
   * 点击人物列表项，切换选中人物（查看详情）
   */
  function handleCharacterClick(characterId: string): void {
    selectedCharacterId = characterId;
  }

  /**
   * 获取角色标签样式类
   */
  function getRoleClass(role: string): string {
    const roleMap: Record<string, string> = {
      '当事人': 'role-protagonist',
      '嫌疑人': 'role-suspect',
      '受害者': 'role-victim',
      '目击者': 'role-witness',
      '旁观者': 'role-bystander',
      '其他': 'role-other',
    };
    return roleMap[role] || 'role-other';
  }

  /**
   * 生成人物头像占位符（使用首字母）
   */
  function getAvatarInitial(name: string): string {
    return name.charAt(0) || '?';
  }
</script>

<div class="character-panel">
  <!-- 面板标题 -->
  <div class="panel-header">
    <h2 class="panel-title">人物画像分析</h2>
    <!-- 人物数量提示 -->
    <p class="selection-progress">
      已提取 <span class="progress-count">{characters.length}</span> 位核心人物
    </p>
  </div>

  <!-- 事件梳理摘要区域（如有） -->
  {#if eventSummary}
    <div class="event-summary-section">
      <div class="event-summary-header">
        <span class="event-summary-title">事件梳理</span>
        <span class="event-summary-hint">AI 整理的事件关键信息</span>
      </div>
      <div class="event-summary-content">
        {eventSummary}
      </div>
    </div>
  {/if}

  <!-- 左右分栏布局 -->
  <div class="split-layout">
    <!-- 左侧：人物列表（可滚动） -->
    <div class="character-list-panel">
      <div class="list-header">
        <span class="list-title">人物列表</span>
      </div>

      <div class="character-list-scroll">
        {#each characters as character (character.id)}
          <button
            type="button"
            class="character-item {selectedCharacterId === character.id ? 'selected' : ''}"
            onclick={() => handleCharacterClick(character.id)}
            aria-pressed={selectedCharacterId === character.id}
          >
            <!-- 人物头像 -->
            <div class="item-avatar">
              <span class="avatar-initial">{getAvatarInitial(character.name)}</span>
            </div>

            <!-- 人物简要信息 -->
            <div class="item-info">
              <span class="item-name">{character.name}</span>
              <span class="item-role {getRoleClass(character.role)}">{character.role}</span>
            </div>
          </button>
        {/each}
      </div>
    </div>

    <!-- 右侧：动机展示面板（可滚动） -->
    <div class="motivation-panel">
      {#if selectedCharacterId}
        {@const selectedCharacter = getSelectedCharacter()}
        {#if selectedCharacter}
          <!-- 选中人物信息 -->
          <div class="selected-character-info">
            <div class="char-header">
              <div class="char-avatar">
                <span class="avatar-initial-large">{getAvatarInitial(selectedCharacter.name)}</span>
              </div>
              <div class="char-meta">
                <h3 class="char-name">{selectedCharacter.name}</h3>
                <span class="char-role {getRoleClass(selectedCharacter.role)}">{selectedCharacter.role}</span>
              </div>
            </div>
            <p class="char-description">{selectedCharacter.description}</p>
          </div>

          <!-- 动机展示区域（纯展示，不需要用户选择） -->
          <div class="motivation-section">
            <div class="motivation-header">
              <span class="motivation-title">可能动机</span>
              <span class="motivation-hint">AI 分析提取的可能动机</span>
            </div>

            {#if selectedCharacter.motivations && selectedCharacter.motivations.length > 0}
              <!-- 动机展示列表（纯展示，不可点击） -->
              <div class="motivation-list-scroll">
                {#each selectedCharacter.motivations as motivation (motivation.id)}
                  <div class="motivation-item">
                    <!-- 上部分：动机解释（主要内容） -->
                    <span class="motivation-content">{motivation.content}</span>

                    <!-- 下部分：可信度评分 + 依据 -->
                    <div class="motivation-meta">
                      <span class="motivation-confidence">可信度: {motivation.confidence}%</span>
                      {#if motivation.sourceHint}
                        <span class="motivation-source">依据: {motivation.sourceHint}</span>
                      {:else}
                        <span class="motivation-source motivation-source-empty">基于人物行为分析</span>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="no-motivation-data">
                <p class="no-data-text">暂无动机数据</p>
              </div>
            {/if}
          </div>
        {/if}
      {:else}
        <!-- 未选中人物时的提示 -->
        <div class="no-character-selected">
          <p class="no-char-text">请从左侧列表选择一个人物</p>
          <p class="no-char-hint">点击人物查看详情和可能动机</p>
        </div>
      {/if}
    </div>
  </div>

  <!-- 操作按钮区域 -->
  <div class="panel-actions">
    <button
      class="btn btn-primary"
      onclick={onStartAnalysis}
      disabled={isAnalyzing}
    >
      {#if isAnalyzing}
        分析中...
      {:else}
        开始深度分析
      {/if}
    </button>

    <button class="btn btn-secondary" onclick={onGoBack}>
      返回首页
    </button>
  </div>
</div>

<style>
  /* ========== 主面板容器（去掉外框包围） ========== */

  .character-panel {
    background-color: var(--bg-secondary);
    border-radius: 0;
    padding: 6px 10px;
    border: none;
    box-shadow: none;
    max-width: 100%;
    margin: 0;
    height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
  }

  /* ========== 面板头部（精简） ========== */

  .panel-header {
    text-align: center;
    margin-bottom: 6px;
    padding-bottom: 5px;
    border-bottom: 1px solid var(--border);
  }

  .panel-icon {
    font-size: 1rem;
    margin-bottom: 3px;
  }

  .panel-title {
    font-size: 0.9rem;
    color: var(--text-primary);
    margin: 0 0 3px 0;
    letter-spacing: -0.02em;
  }

  .panel-desc {
    color: var(--text-secondary);
    font-size: 0.75rem;
    margin: 0 0 4px 0;
  }

  /* 选择进度提示 */
  .selection-progress {
    color: var(--text-secondary);
    font-size: 0.72rem;
    margin: 0;
  }

  .progress-count {
    font-weight: 600;
    font-size: 0.78rem;
    color: var(--text-primary);
  }

  /* ========== 左右分栏布局 ========== */

  .split-layout {
    display: grid;
    grid-template-columns: 150px 1fr;
    gap: 10px;
    flex: 1;
    min-height: 0;
    margin-bottom: 6px;
  }

  /* ========== 左侧：人物列表面板 ========== */

  .character-list-panel {
    background-color: var(--bg-card);
    border-radius: 4px;
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .list-header {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 8px;
    border-bottom: 1px solid var(--border);
    background-color: var(--bg-secondary);
    border-radius: 4px 4px 0 0;
  }

  .list-icon {
    font-size: 0.78rem;
  }

  .list-title {
    font-size: 0.68rem;
    color: var(--text-primary);
    font-weight: 600;
  }

  /* 人物列表滚动区域 */
  .character-list-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
    min-height: 0;
  }

  /* 滚动条样式美化 */
  .character-list-scroll::-webkit-scrollbar {
    width: 4px;
  }

  .character-list-scroll::-webkit-scrollbar-track {
    background: var(--bg-secondary);
    border-radius: 2px;
  }

  .character-list-scroll::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 2px;
  }

  .character-list-scroll::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }

  /* 人物列表项（使用 button 元素）- 更紧凑 */
  .character-item {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 6px;
    background-color: var(--bg-secondary);
    border-radius: 4px;
    border: 1px solid var(--border);
    cursor: pointer;
    transition: all 0.15s;
    margin-bottom: 3px;
    width: 100%;
    text-align: left;
    font-family: inherit;
  }

  .character-item:hover {
    border-color: #ccc;
    background-color: var(--bg-hover);
  }

  .character-item.selected {
    border-color: var(--accent);
    background-color: var(--accent-light);
    box-shadow: var(--shadow-sm);
  }

  /* 按钮焦点样式 */
  .character-item:focus {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  /* 人物头像（左侧列表）- 更小 */
  .item-avatar {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: linear-gradient(135deg, #333, #888);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .avatar-initial {
    font-size: 0.68rem;
    font-weight: 600;
    color: #fff;
  }

  /* 人物简要信息 */
  .item-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .item-name {
    font-size: 0.72rem;
    color: var(--text-primary);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-role {
    font-size: 0.6rem;
    padding: 1px 3px;
    border-radius: 2px;
    background-color: var(--bg-card);
    color: var(--text-secondary);
  }

  /* ========== 右侧：动机选择面板 ========== */

  .motivation-panel {
    background-color: var(--bg-card);
    border-radius: 4px;
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  /* 选中人物信息 - 更紧凑 */
  .selected-character-info {
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
    background-color: var(--bg-secondary);
  }

  .char-header {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-bottom: 5px;
  }

  .char-avatar {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: linear-gradient(135deg, #333, #888);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .avatar-initial-large {
    font-size: 0.85rem;
    font-weight: 600;
    color: #fff;
  }

  .char-meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .char-name {
    font-size: 0.82rem;
    color: var(--text-primary);
    margin: 0;
    font-weight: 600;
  }

  .char-role {
    font-size: 0.6rem;
    padding: 1px 5px;
    border-radius: 2px;
    display: inline-block;
  }

  .char-description {
    font-size: 0.7rem;
    color: var(--text-secondary);
    line-height: 1.4;
    margin: 0;
  }

  /* 动机选择区域 - 更紧凑 */
  .motivation-section {
    flex: 1;
    padding: 7px 10px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .motivation-header {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-bottom: 6px;
    padding-bottom: 5px;
    border-bottom: 1px solid var(--border);
  }

  .motivation-icon {
    font-size: 0.78rem;
  }

  .motivation-title {
    font-size: 0.72rem;
    color: var(--text-primary);
    font-weight: 600;
  }

  .motivation-hint {
    font-size: 0.62rem;
    color: var(--text-muted);
    margin-left: auto;
  }

  /* 动机列表滚动区域 */
  .motivation-list-scroll {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding-right: 4px; /* 为滚动条预留空间 */
  }

  /* 滚动条样式美化 */
  .motivation-list-scroll::-webkit-scrollbar {
    width: 4px;
  }

  .motivation-list-scroll::-webkit-scrollbar-track {
    background: var(--bg-secondary);
    border-radius: 2px;
  }

  .motivation-list-scroll::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 2px;
  }

  .motivation-list-scroll::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }

  /* 动机展示项 - 纯展示样式 */
  .motivation-item {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 8px 10px;
    border-radius: 4px;
    background-color: var(--bg-secondary);
    border: 1px solid var(--border);
    margin-bottom: 6px;
    transition: border-color 0.1s;
  }

  .motivation-item:hover {
    border-color: #ccc;
  }

  /* 动机内容 */
  .motivation-content {
    font-size: 0.78rem;
    color: var(--text-primary);
    font-weight: 500;
    line-height: 1.4;
  }

  /* 动机元数据：可信度 + 依据 */
  .motivation-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }

  .motivation-confidence {
    font-size: 0.68rem;
    color: var(--text-primary);
    font-weight: 600;
  }

  .motivation-source {
    font-size: 0.62rem;
    color: var(--text-muted);
    line-height: 1.3;
  }

  .motivation-source-empty {
    color: var(--text-muted);
    opacity: 0.7;
  }

  /* 无动机数据 */
  .no-motivation-data {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .no-data-icon {
    font-size: 1.5rem;
    margin-bottom: 8px;
  }

  .no-data-text {
    color: var(--text-muted);
    font-size: 0.68rem;
    margin: 0;
  }

  /* 未选中人物提示 */
  .no-character-selected {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .no-char-icon {
    font-size: 1.5rem;
    margin-bottom: 8px;
  }

  .no-char-text {
    color: var(--text-primary);
    font-size: 0.78rem;
    font-weight: 500;
    margin: 0 0 4px 0;
  }

  .no-char-hint {
    color: var(--text-muted);
    font-size: 0.68rem;
    margin: 0;
  }

  /* ========== 角色类型样式 ========== */

  .role-protagonist {
    background-color: #3b82f6;
    color: white;
  }

  .role-suspect {
    background-color: #ef4444;
    color: white;
  }

  .role-victim {
    background-color: #f59e0b;
    color: white;
  }

  .role-witness {
    background-color: #10b981;
    color: white;
  }

  .role-bystander {
    background-color: #6366f1;
    color: white;
  }

  /* ========== 操作按钮区域 - 更紧凑 ========== */

  .panel-actions {
    display: flex;
    gap: 6px;
    justify-content: center;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }

  .btn {
    padding: 7px 14px;
    border-radius: 4px;
    font-size: 0.72rem;
    cursor: pointer;
    transition: all 0.15s;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .btn-primary {
    background-color: var(--accent);
    /* 文字色跟主色反色，兼顾浅深两种主题 */
    color: var(--bg-primary);
    border: none;
  }

  .btn-primary:hover:not(:disabled) {
    background-color: var(--accent-hover);
    transform: translateY(-0.5px);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background-color: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
  }

  .btn-secondary:hover {
    border-color: #ccc;
    background-color: var(--bg-card);
  }

  /* ========== 事件梳理摘要样式 ========== */

  .event-summary-section {
    background-color: var(--bg-card);
    border-radius: 4px;
    border: 1px solid var(--border);
    padding: 10px 14px;
    margin-bottom: 10px;
  }

  .event-summary-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--border);
  }

  .event-summary-title {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .event-summary-hint {
    font-size: 0.62rem;
    color: var(--text-muted);
    margin-left: auto;
  }

  .event-summary-content {
    font-size: 0.72rem;
    color: var(--text-secondary);
    line-height: 1.6;
    max-height: 120px;
    overflow-y: auto;
  }

  .event-summary-content::-webkit-scrollbar {
    width: 3px;
  }

  .event-summary-content::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 2px;
  }
</style>