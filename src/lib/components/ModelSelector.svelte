<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  /**
   * ModelSelector 组件 - 模型选择与配置弹框
   *
   * 设计理念：
   * - 在输入框底部显示当前使用的模型名称，点击可切换
   * - 如果本地没有保存模型配置，点击时弹出"添加配置"弹框
   * - 弹框包含：baseURL、apiKey、model 名称、是否多模态开关
   * - 只有多模态模型才能上传文件
   *
   * 交互流程：
   * 1. 首次使用 → 点击模型名 → 弹出"添加配置"弹框
   * 2. 已有配置 → 点击模型名 → 显示已保存模型列表，可切换或新增
   * 3. 保存后自动关闭弹框，更新当前选中模型
   */

  // ========== 类型定义 ==========

  /**
   * 模型配置结构（单个配置项）
   * 一个配置项包含完整的 API 连接信息
   */
  export interface ModelConfig {
    /** 配置唯一标识（用于区分不同配置） */
    id: string;
    /** API 基础地址（如 https://dashscope.aliyuncs.com/compatible-mode/v1） */
    baseUrl: string;
    /** API 密钥 */
    apiKey: string;
    /** 模型名称（如 glm-5.1、qwen-plus） */
    model: string;
    /** 是否支持多模态（图片输入） */
    isMultimodal: boolean;
    /** 配置显示名称（自动生成，如 "GLM-5.1 @ DashScope"） */
    displayName: string;
  }

  // ========== Props ==========

  interface Props {
    /** 当前选中的模型配置变更回调 */
    onModelChange: (config: ModelConfig) => void;
  }

  let { onModelChange }: Props = $props();

  // ========== 内部状态 ==========

  /** 所有已保存的模型配置列表 */
  let modelConfigs: ModelConfig[] = $state([]);

  /** 当前选中的模型配置 ID */
  let currentConfigId = $state('');

  /** 是否显示模型选择/添加弹框 */
  let showModelDialog = $state(false);

  /** 是否显示模型管理弹框（从选择弹框关闭后打开） */
  let showManageDialog = $state(false);

  /** 弹框模式：'select' 选择已有配置 | 'add' 添加新配置 */
  let dialogMode: 'select' | 'add' = $state('select');

  /** 管理弹框模式：'manage' 模型列表 | 'edit' 编辑某个模型 */
  let manageMode: 'manage' | 'edit' = $state('manage');

  /** 正在编辑的模型配置 ID */
  let editingConfigId = $state('');

  // ========== 新配置表单状态 ==========

  /** 新配置的 baseUrl */
  let newBaseUrl = $state('https://dashscope.aliyuncs.com/compatible-mode/v1');

  /** 新配置的 apiKey */
  let newApiKey = $state('');

  /** 新配置的 model 名称 */
  let newModelName = $state('');

  /** 新配置是否多模态 */
  let newIsMultimodal = $state(false);

  // ========== 编辑模型表单状态 ==========

  /** 编辑时的 baseUrl（从已有配置预填充） */
  let editBaseUrl = $state('');

  /** 编辑时的 apiKey（从已有配置预填充） */
  let editApiKey = $state('');

  /** 编辑时的 model 名称（从已有配置预填充） */
  let editModelName = $state('');

  /** 编辑时是否多模态（从已有配置预填充） */
  let editIsMultimodal = $state(false);

  /** 是否正在保存 */
  let isSaving = $state(false);

  /** Toast 提示 */
  let toastMessage = $state('');
  let toastType = $state<'success' | 'error' | ''>('');

  // ========== 生命周期 ==========

  onMount(async () => {
    await loadModelConfigs();
  });

  // ========== 方法 ==========

  /**
   * 从本地存储加载模型配置列表
   * 配置存储格式：JSON 数组，key 为 "model_configs"
   */
  async function loadModelConfigs(): Promise<void> {
    try {
      // 从 Tauri 后端获取已保存的配置
      const configs = await invoke<ModelConfig[]>('get_model_configs');
      modelConfigs = configs || [];

      // 如果有配置，设置默认选中第一个
      if (modelConfigs.length > 0) {
        // 检查是否有上次选中的配置
        const savedCurrentId = await invoke<string>('get_current_model_config_id');
        if (savedCurrentId && modelConfigs.some(c => c.id === savedCurrentId)) {
          currentConfigId = savedCurrentId;
        } else {
          currentConfigId = modelConfigs[0].id;
        }
        // 触发回调，通知父组件当前选中的模型
        notifyModelChange();
      }
    } catch (error) {
      console.warn('加载模型配置失败（可能是首次使用）:', error);
      modelConfigs = [];
    }
  }

  /**
   * 通知父组件当前选中的模型配置已变更
   */
  function notifyModelChange(): void {
    const currentConfig = modelConfigs.find(c => c.id === currentConfigId);
    if (currentConfig) {
      onModelChange(currentConfig);
    }
  }

  /**
   * 获取当前选中模型的显示名称
   */
  function getCurrentModelDisplayName(): string {
    const currentConfig = modelConfigs.find(c => c.id === currentConfigId);
    if (currentConfig) {
      return currentConfig.displayName || currentConfig.model;
    }
    return '选择模型'; // 未配置时显示提示文字
  }

  /**
   * 点击模型名称按钮
   * - 有配置：打开选择列表弹框
   * - 无配置：直接打开添加配置弹框
   */
  function handleModelClick(): void {
    if (modelConfigs.length === 0) {
      // 没有配置，直接打开添加弹框
      dialogMode = 'add';
      resetNewConfigForm();
      showModelDialog = true;
    } else {
      // 有配置，打开选择列表弹框（可以切换或新增）
      dialogMode = 'select';
      showModelDialog = true;
    }
  }

  /**
   * 切换到添加新配置模式
   */
  function switchToAddMode(): void {
    dialogMode = 'add';
    resetNewConfigForm();
  }

  /**
   * 切换到管理模型模式
   * 关闭选择弹框，延迟打开管理弹框（视觉上先关闭再打开）
   */
  function switchToManageMode(): void {
    showModelDialog = false;
    manageMode = 'manage';
    editingConfigId = '';
    // 延迟 150ms 打开管理弹框，让用户感知弹框切换
    setTimeout(() => {
      showManageDialog = true;
    }, 150);
  }

  /**
   * 从管理列表进入编辑某个模型
   * 预填充该模型的所有字段到编辑表单
   */
  function switchToEditMode(configId: string): void {
    const config = modelConfigs.find(c => c.id === configId);
    if (!config) return;
    editingConfigId = configId;
    editBaseUrl = config.baseUrl;
    editApiKey = config.apiKey;
    editModelName = config.model;
    editIsMultimodal = config.isMultimodal;
    manageMode = 'edit';
  }

  /**
   * 从编辑模式返回管理列表
   */
  function backFromEdit(): void {
    manageMode = 'manage';
    editingConfigId = '';
  }

  /**
   * 保存编辑后的模型配置
   * 更新已有配置（而非新建），保存后返回管理列表
   */
  async function saveEditedConfig(): Promise<void> {
    // 验证必填字段
    if (!editApiKey.trim()) {
      showToast('请输入 API Key', 'error');
      return;
    }
    if (!editModelName.trim()) {
      showToast('请输入模型名称', 'error');
      return;
    }
    if (!editBaseUrl.trim()) {
      showToast('请输入 Base URL', 'error');
      return;
    }

    isSaving = true;

    try {
      // 生成新的显示名称
      const shortDomain = editBaseUrl.replace(/https?:\/\//, '').split('/')[0];
      const displayName = `${editModelName} @ ${shortDomain}`;

      // 更新配置列表中的对应项（不可变更新）
      const updatedConfigs = modelConfigs.map(c =>
        c.id === editingConfigId
          ? {
              ...c,
              baseUrl: editBaseUrl.trim(),
              apiKey: editApiKey.trim(),
              model: editModelName.trim(),
              isMultimodal: editIsMultimodal,
              displayName,
            }
          : c
      );

      // 保存到 Tauri 后端
      await invoke('save_model_configs', { configs: updatedConfigs });

      // 更新本地状态
      modelConfigs = updatedConfigs;

      // 如果编辑的是当前选中模型，同步到后端并通知父组件
      if (currentConfigId === editingConfigId) {
        const updatedConfig = updatedConfigs.find(c => c.id === editingConfigId)!;
        await syncToBackend(updatedConfig);
        notifyModelChange();
      }

      showToast('配置已更新', 'success');

      // 返回管理列表
      manageMode = 'manage';
      editingConfigId = '';
    } catch (error) {
      console.error('更新模型配置失败:', error);
      showToast(`更新失败: ${String(error)}`, 'error');
    } finally {
      isSaving = false;
    }
  }

  /**
   * 重置新配置表单为默认值
   */
  function resetNewConfigForm(): void {
    newBaseUrl = 'https://dashscope.aliyuncs.com/compatible-mode/v1';
    newApiKey = '';
    newModelName = '';
    newIsMultimodal = false;
  }

  /**
   * 选择一个已有的模型配置
   */
  function selectModelConfig(configId: string): void {
    currentConfigId = configId;
    // 保存当前选中 ID 到本地存储
    invoke('save_current_model_config_id', { id: configId }).catch(e => {
      console.warn('保存当前选中模型失败:', e);
    });
    // 同步保存到后端全局配置（让分析流程使用这个模型）
    syncToBackend(modelConfigs.find(c => c.id === configId)!);
    notifyModelChange();
    showModelDialog = false;
  }

  /**
   * 保存新配置到本地存储
   */
  async function saveNewConfig(): Promise<void> {
    // 验证必填字段
    if (!newApiKey.trim()) {
      showToast('请输入 API Key', 'error');
      return;
    }
    if (!newModelName.trim()) {
      showToast('请输入模型名称', 'error');
      return;
    }
    if (!newBaseUrl.trim()) {
      showToast('请输入 Base URL', 'error');
      return;
    }

    isSaving = true;

    try {
      // 生成唯一 ID
      const configId = `config_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

      // 生成显示名称：模型名 + 短域名
      const shortDomain = newBaseUrl.replace(/https?:\/\//, '').split('/')[0];
      const displayName = `${newModelName} @ ${shortDomain}`;

      const newConfig: ModelConfig = {
        id: configId,
        baseUrl: newBaseUrl.trim(),
        apiKey: newApiKey.trim(),
        model: newModelName.trim(),
        isMultimodal: newIsMultimodal,
        displayName,
      };

      // 添加到配置列表
      const updatedConfigs = [...modelConfigs, newConfig];

      // 保存到 Tauri 后端
      await invoke('save_model_configs', { configs: updatedConfigs });

      // 更新本地状态
      modelConfigs = updatedConfigs;
      currentConfigId = configId;

      // 保存当前选中 ID
      await invoke('save_current_model_config_id', { id: configId });

      // 同步到后端全局配置
      await syncToBackend(newConfig);

      // 通知父组件
      notifyModelChange();

      showToast('配置已保存', 'success');

      // 延迟关闭弹框（让用户看到成功提示）
      setTimeout(() => {
        showModelDialog = false;
      }, 800);
    } catch (error) {
      console.error('保存模型配置失败:', error);
      showToast(`保存失败: ${String(error)}`, 'error');
    } finally {
      isSaving = false;
    }
  }

  /**
   * 同步当前选中配置到后端全局设置
   * 这样分析流程可以直接使用 invoke('get_model') 等获取当前配置
   */
  async function syncToBackend(config: ModelConfig): Promise<void> {
    try {
      await invoke('save_api_key', { key: config.apiKey });
      await invoke('save_base_url', { url: config.baseUrl });
      await invoke('save_model', { model: config.model });
      // 多模态模型保存
      if (config.isMultimodal) {
        await invoke('save_multimodal_model', { model: config.model });
      }
    } catch (error) {
      console.error('同步配置到后端失败:', error);
    }
  }

  /**
   * 删除一个模型配置
   */
  async function deleteModelConfig(configId: string): void {
    try {
      const updatedConfigs = modelConfigs.filter(c => c.id !== configId);
      await invoke('save_model_configs', { configs: updatedConfigs });

      modelConfigs = updatedConfigs;

      // 如果删除的是当前选中的配置，切换到第一个
      if (currentConfigId === configId) {
        if (modelConfigs.length > 0) {
          currentConfigId = modelConfigs[0].id;
          await invoke('save_current_model_config_id', { id: currentConfigId });
          await syncToBackend(modelConfigs[0]);
          notifyModelChange();
        } else {
          currentConfigId = '';
        }
      }
    } catch (error) {
      console.error('删除模型配置失败:', error);
      showToast(`删除失败: ${String(error)}`, 'error');
    }
  }

  /**
   * 显示 Toast 提示
   */
  function showToast(message: string, type: 'success' | 'error'): void {
    toastMessage = message;
    toastType = type;
    setTimeout(() => {
      toastMessage = '';
      toastType = '';
    }, 3000);
  }

  /**
   * 关闭弹框
   */
  function closeDialog(): void {
    showModelDialog = false;
  }

  /**
   * 点击遮罩层关闭弹框
   */
  function handleOverlayClick(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      closeDialog();
    }
  }

  /**
   * 判断当前模型是否为多模态
   * 父组件可通过这个判断是否显示文件上传按钮
   */
  export function isCurrentModelMultimodal(): boolean {
    const currentConfig = modelConfigs.find(c => c.id === currentConfigId);
    return currentConfig?.isMultimodal ?? false;
  }

  /**
   * 获取当前选中的模型配置
   */
  export function getCurrentConfig(): ModelConfig | null {
    return modelConfigs.find(c => c.id === currentConfigId) || null;
  }
</script>

<!-- 模型选择按钮（在输入框底部显示） -->
<button
  class="model-selector-btn"
  onclick={handleModelClick}
  title={modelConfigs.length === 0 ? '添加模型配置' : '切换模型'}
  aria-label="模型选择"
>
  <!-- 模型图标 -->
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <rect x="4" y="4" width="16" height="16" rx="2" ry="2"/>
    <line x1="9" y1="1" x2="9" y2="4"/>
    <line x1="15" y1="1" x2="15" y2="4"/>
    <line x1="9" y1="20" x2="9" y2="23"/>
    <line x1="15" y1="20" x2="15" y2="23"/>
    <line x1="20" y1="9" x2="23" y2="9"/>
    <line x1="20" y1="14" x2="23" y2="14"/>
    <line x1="1" y1="9" x2="4" y2="9"/>
    <line x1="1" y1="14" x2="4" y2="14"/>
  </svg>

  <!-- 模型名称显示 -->
  <span class="model-name">{getCurrentModelDisplayName()}</span>

  <!-- 下拉箭头（仅在有配置时显示） -->
  {#if modelConfigs.length > 0}
    <svg class="dropdown-arrow" width="10" height="10" viewBox="0 0 10 10">
      <path d="M2 4l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.2"/>
    </svg>
  {/if}
</button>

<!-- 模型选择/添加弹框 -->
{#if showModelDialog}
  <div class="dialog-overlay" onclick={handleOverlayClick}>
    <div class="dialog-content" onclick={(e) => e.stopPropagation()}>
      <!-- 弹框头部 -->
      <div class="dialog-header">
        <h2 class="dialog-title">
          {dialogMode === 'select' ? '选择模型' : '添加模型配置'}
        </h2>
        <button class="dialog-close-btn" onclick={closeDialog}>×</button>
      </div>

      <!-- 弹框主体 -->
      <div class="dialog-body">
        {#if dialogMode === 'select'}
          <!-- 选择模式：显示已保存的配置列表 -->
          <div class="config-list">
            {#each modelConfigs as config (config.id)}
              <div
                class="config-item {currentConfigId === config.id ? 'selected' : ''}"
                onclick={() => selectModelConfig(config.id)}
                role="button"
                tabindex="0"
              >
                <!-- 模型名称 -->
                <div class="config-item-name">{config.displayName}</div>
                <!-- 模型标签 -->
                <div class="config-item-tags">
                  {#if config.isMultimodal}
                    <span class="tag-multimodal">多模态</span>
                  {:else}
                    <span class="tag-text-only">纯文本</span>
                  {/if}
                </div>
                <!-- 删除按钮 -->
                <button
                  class="config-delete-btn"
                  onclick={(e) => { e.stopPropagation(); deleteModelConfig(config.id); }}
                  title="删除此配置"
                  aria-label="删除配置"
                >
                  ✕
                </button>
              </div>
            {/each}

            <!-- 添加新配置按钮 -->
            <button class="add-config-btn" onclick={switchToAddMode}>
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M7 1v12M1 7h12" stroke="currentColor" stroke-width="1.5"/>
              </svg>
              添加新配置
            </button>

            <!-- 管理模型按钮：关闭选择弹框，打开管理弹框 -->
            <button class="manage-config-btn" onclick={switchToManageMode}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="3"/>
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
              </svg>
              管理模型
            </button>
          </div>
        {:else}
          <!-- 添加模式：填写新配置表单 -->
          <div class="add-config-form">
            <!-- Base URL -->
            <div class="form-group">
              <label class="form-label">Base URL</label>
              <input
                type="text"
                class="form-input"
                bind:value={newBaseUrl}
                placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
              />
              <small class="form-hint">API 服务地址，默认为 DashScope OpenAI 兼容端点</small>
            </div>

            <!-- API Key -->
            <div class="form-group">
              <label class="form-label">API Key</label>
              <input
                type="password"
                class="form-input"
                bind:value={newApiKey}
                placeholder="sk-..."
                autocomplete="off"
              />
              <small class="form-hint">API 密钥，将安全存储在本地</small>
            </div>

            <!-- 模型名称 -->
            <div class="form-group">
              <label class="form-label">模型名称</label>
              <input
                type="text"
                class="form-input"
                bind:value={newModelName}
                placeholder="如 glm-5.1、qwen-plus、gpt-4o"
              />
              <small class="form-hint">输入模型名称，需确保 Base URL 支持该模型</small>
            </div>

            <!-- 是否多模态开关 -->
            <div class="form-group toggle-group">
              <label class="form-label">支持多模态</label>
              <div class="toggle-wrapper">
                <button
                  class="toggle-btn {newIsMultimodal ? 'active' : ''}"
                  onclick={() => newIsMultimodal = !newIsMultimodal}
                  role="switch"
                  aria-checked={newIsMultimodal}
                  aria-label="是否支持多模态"
                >
                  <span class="toggle-indicator"></span>
                </button>
                <span class="toggle-label">
                  {newIsMultimodal ? '是 · 支持图片输入' : '否 · 仅处理文本'}
                </span>
              </div>
              <small class="form-hint">
                多模态模型可上传图片文件进行分析，非多模态仅支持文本输入
              </small>
            </div>

            <!-- 保存按钮 -->
            <div class="btn-group">
              {#if modelConfigs.length > 0}
                <button class="btn btn-secondary" onclick={() => dialogMode = 'select'}>
                  返回选择
                </button>
              {/if}
              <button class="btn btn-primary" onclick={saveNewConfig} disabled={isSaving}>
                {isSaving ? '保存中...' : '保存配置'}
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<!-- 模型管理弹框（独立弹框，从选择弹框关闭后打开） -->
{#if showManageDialog}
  <div class="dialog-overlay" onclick={(e) => { if (e.target === e.currentTarget) showManageDialog = false; }}>
    <div class="dialog-content" onclick={(e) => e.stopPropagation()}>
      {#if manageMode === 'manage'}
        <!-- ========== 管理列表模式 ========== -->
        <div class="dialog-header">
          <h2 class="dialog-title">管理模型</h2>
          <button class="dialog-close-btn" onclick={() => showManageDialog = false}>×</button>
        </div>

        <div class="dialog-body">
          <div class="manage-config-list">
            {#each modelConfigs as config (config.id)}
              <!-- 每个模型可点击进入编辑 -->
              <div
                class="manage-item {currentConfigId === config.id ? 'current' : ''}"
                onclick={() => switchToEditMode(config.id)}
                role="button"
                tabindex="0"
              >
                <!-- 当前选中标识 -->
                {#if currentConfigId === config.id}
                  <span class="current-badge">当前</span>
                {/if}
                <!-- 模型名称 -->
                <div class="manage-item-name">{config.displayName}</div>
                <!-- 模型标签 -->
                <div class="manage-item-tags">
                  {#if config.isMultimodal}
                    <span class="tag-multimodal">多模态</span>
                  {:else}
                    <span class="tag-text-only">纯文本</span>
                  {/if}
                </div>
                <!-- 点击编辑提示 -->
                <span class="manage-item-edit-hint">编辑</span>
              </div>
            {/each}

            <!-- 管理弹框内也支持添加新配置 -->
            <button class="add-config-btn" onclick={() => { showManageDialog = false; dialogMode = 'add'; resetNewConfigForm(); setTimeout(() => { showModelDialog = true; }, 150); }}>
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M7 1v12M1 7h12" stroke="currentColor" stroke-width="1.5"/>
              </svg>
              添加新配置
            </button>
          </div>

          <!-- 底部取消按钮 -->
          <div class="manage-footer">
            <button class="btn btn-secondary" onclick={() => showManageDialog = false}>
              取消
            </button>
          </div>
        </div>

      {:else if manageMode === 'edit'}
        <!-- ========== 编辑模型模式 ========== -->
        <div class="dialog-header">
          <!-- 左上角返回箭头 -->
          <button class="back-arrow-btn" onclick={backFromEdit} aria-label="返回管理列表" title="返回">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <path d="M10 3L5 8l5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
          <h2 class="dialog-title">编辑模型配置</h2>
          <button class="dialog-close-btn" onclick={() => showManageDialog = false}>×</button>
        </div>

        <div class="dialog-body">
          <div class="add-config-form">
            <!-- Base URL -->
            <div class="form-group">
              <label class="form-label">Base URL</label>
              <input
                type="text"
                class="form-input"
                bind:value={editBaseUrl}
                placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
              />
              <small class="form-hint">API 服务地址</small>
            </div>

            <!-- API Key -->
            <div class="form-group">
              <label class="form-label">API Key</label>
              <input
                type="password"
                class="form-input"
                bind:value={editApiKey}
                placeholder="sk-..."
                autocomplete="off"
              />
              <small class="form-hint">API 密钥，将安全存储在本地</small>
            </div>

            <!-- 模型名称 -->
            <div class="form-group">
              <label class="form-label">模型名称</label>
              <input
                type="text"
                class="form-input"
                bind:value={editModelName}
                placeholder="如 glm-5.1、qwen-plus、gpt-4o"
              />
              <small class="form-hint">输入模型名称，需确保 Base URL 支持该模型</small>
            </div>

            <!-- 是否多模态开关 -->
            <div class="form-group toggle-group">
              <label class="form-label">支持多模态</label>
              <div class="toggle-wrapper">
                <button
                  class="toggle-btn {editIsMultimodal ? 'active' : ''}"
                  onclick={() => editIsMultimodal = !editIsMultimodal}
                  role="switch"
                  aria-checked={editIsMultimodal}
                  aria-label="是否支持多模态"
                >
                  <span class="toggle-indicator"></span>
                </button>
                <span class="toggle-label">
                  {editIsMultimodal ? '是 · 支持图片输入' : '否 · 仅处理文本'}
                </span>
              </div>
              <small class="form-hint">
                多模态模型可上传图片文件进行分析
              </small>
            </div>

            <!-- 底部按钮：取消 + 保存 -->
            <div class="btn-group">
              <button class="btn btn-secondary" onclick={() => showManageDialog = false}>
                取消
              </button>
              <button class="btn btn-primary" onclick={saveEditedConfig} disabled={isSaving}>
                {isSaving ? '保存中...' : '保存'}
              </button>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- Toast 提示 -->
{#if toastMessage}
  <div class="toast toast-{toastType}">
    {toastMessage}
  </div>
{/if}

<style>
  /* ========== 模型选择按钮 ========== */

  .model-selector-btn {
    /* 圆角胶囊样式，和参数pill一致 */
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--bg-card);
    color: var(--text-secondary);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.72rem;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .model-selector-btn:hover {
    border-color: #ccc;
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* 模型名称 */
  .model-name {
    font-weight: 500;
    color: var(--text-primary);
  }

  /* 下拉箭头 */
  .dropdown-arrow {
    color: var(--text-muted);
    margin-left: 2px;
  }

  /* ========== 弹框样式 ========== */

  .dialog-overlay {
    /* 全屏遮罩 */
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.15);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    backdrop-filter: blur(4px);
    animation: overlayFadeIn 0.15s ease;
  }

  @keyframes overlayFadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .dialog-content {
    /* 弹框主体 */
    background: var(--bg-secondary);
    border-radius: 12px;
    padding: 20px;
    width: 420px;
    max-width: 90vw;
    max-height: 80vh;
    overflow-y: auto;
    box-shadow: var(--shadow-xl);
    border: 1px solid var(--border);
    animation: dialogSlideIn 0.2s ease;
  }

  @keyframes dialogSlideIn {
    from {
      opacity: 0;
      transform: translateY(-8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }

  .dialog-title {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
    letter-spacing: -0.02em;
  }

  .dialog-close-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--bg-card);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 0.9rem;
    transition: all 0.15s;
  }

  .dialog-close-btn:hover {
    background: var(--error);
    color: #fff;
  }

  .dialog-body {
    padding: 0;
  }

  /* ========== 配置列表样式 ========== */

  .config-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .config-item {
    /* 配置项卡片 */
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    cursor: pointer;
    transition: all 0.15s;
    position: relative;
  }

  .config-item:hover {
    border-color: #ccc;
    background: var(--bg-hover);
  }

  /* 选中状态 */
  .config-item.selected {
    border-color: var(--accent);
    background: var(--accent-light);
  }

  .config-item-name {
    /* 配置显示名称 */
    flex: 1;
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--text-primary);
  }

  .config-item-tags {
    /* 标签区域 */
    display: flex;
    gap: 4px;
  }

  .tag-multimodal {
    /* 多模态标签 */
    font-size: 0.65rem;
    padding: 2px 8px;
    border-radius: 10px;
    background: rgba(16, 163, 127, 0.1);
    color: #10a37f;
    border: 1px solid rgba(16, 163, 127, 0.2);
  }

  .tag-text-only {
    /* 纯文本标签 */
    font-size: 0.65rem;
    padding: 2px 8px;
    border-radius: 10px;
    background: rgba(59, 130, 246, 0.06);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }

  .config-delete-btn {
    /* 删除按钮 */
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    font-size: 0.72rem;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
    padding: 0;
  }

  .config-delete-btn:hover {
    color: var(--error);
    background: rgba(220, 38, 38, 0.06);
  }

  /* 添加新配置按钮 */
  .add-config-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px;
    border-radius: 8px;
    border: 1px dashed var(--border);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 0.78rem;
    font-family: inherit;
    transition: all 0.15s;
    width: 100%;
  }

  .add-config-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-light);
  }

  /* ========== 管理模型按钮 ========== */

  .manage-config-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 0.78rem;
    font-family: inherit;
    transition: all 0.15s;
    width: 100%;
  }

  .manage-config-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-light);
  }

  /* ========== 管理列表样式 ========== */

  .manage-config-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .manage-item {
    /* 管理列表项：可点击进入编辑 */
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    cursor: pointer;
    transition: all 0.15s;
    position: relative;
  }

  .manage-item:hover {
    border-color: var(--accent);
    background: var(--accent-light);
  }

  /* 当前选中项高亮 */
  .manage-item.current {
    border-color: var(--accent);
    background: var(--accent-light);
  }

  .current-badge {
    font-size: 0.6rem;
    padding: 1px 6px;
    border-radius: 10px;
    background: var(--accent);
    /* 文字反色，暗黑模式下才可读 */
    color: var(--bg-primary);
    font-weight: 500;
  }

  .manage-item-name {
    /* 配置显示名称 */
    flex: 1;
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--text-primary);
  }

  .manage-item-tags {
    /* 标签区域 */
    display: flex;
    gap: 4px;
  }

  .manage-item-edit-hint {
    font-size: 0.68rem;
    color: var(--text-muted);
    opacity: 0;
    transition: opacity 0.15s;
  }

  .manage-item:hover .manage-item-edit-hint {
    opacity: 1;
    color: var(--accent);
  }

  /* 管理弹框底部 */
  .manage-footer {
    display: flex;
    justify-content: flex-end;
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }

  /* ========== 编辑模式：返回箭头 ========== */

  .back-arrow-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--bg-card);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all 0.15s;
    padding: 0;
  }

  .back-arrow-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  /* ========== 添加配置表单样式 ========== */

  .add-config-form {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .form-group {
    margin-bottom: 14px;
  }

  .form-label {
    display: block;
    font-size: 0.78rem;
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: 5px;
  }

  .form-input {
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background-color: var(--bg-primary);
    color: var(--text-primary);
    font-size: 0.78rem;
    transition: border-color 0.15s;
    box-sizing: border-box;
    font-family: inherit;
  }

  .form-input:focus {
    outline: none;
    border-color: var(--border-focus);
    box-shadow: 0 0 0 1px var(--border-focus);
  }

  .form-input::placeholder {
    color: var(--text-muted);
  }

  .form-hint {
    display: block;
    margin-top: 4px;
    font-size: 0.65rem;
    color: var(--text-secondary);
    opacity: 0.8;
  }

  /* ========== 多模态开关样式 ========== */

  .toggle-group {
    margin-bottom: 18px;
  }

  .toggle-wrapper {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }

  .toggle-btn {
    /* iOS 风格开关按钮 */
    width: 40px;
    height: 22px;
    border-radius: 11px;
    border: none;
    background: var(--border);
    cursor: pointer;
    position: relative;
    transition: background 0.2s;
    padding: 0;
  }

  .toggle-btn.active {
    background: var(--accent);
  }

  .toggle-indicator {
    /* 开关指示圆点 */
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    position: absolute;
    top: 3px;
    left: 3px;
    transition: transform 0.2s;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }

  .toggle-btn.active .toggle-indicator {
    transform: translateX(18px);
  }

  .toggle-label {
    font-size: 0.72rem;
    color: var(--text-secondary);
  }

  /* ========== 按钮样式 ========== */

  .btn-group {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 18px;
  }

  .btn {
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 0.78rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s;
    border: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
  }

  .btn-primary {
    background-color: var(--accent);
    /* 文字反色，暗黑模式下才可读 */
    color: var(--bg-primary);
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
    background-color: var(--bg-primary);
    color: var(--text-primary);
    border: 1px solid var(--border);
  }

  .btn-secondary:hover {
    border-color: #ccc;
  }

  /* ========== Toast 提示 ========== */

  .toast {
    position: fixed;
    bottom: 16px;
    right: 16px;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 0.78rem;
    z-index: 300;
    animation: toastSlideIn 0.25s ease;
    box-shadow: var(--shadow-lg);
  }

  .toast-success {
    background-color: var(--success);
    color: white;
  }

  .toast-error {
    background-color: var(--error);
    color: white;
  }

  @keyframes toastSlideIn {
    from {
      transform: translateX(100%);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }
</style>