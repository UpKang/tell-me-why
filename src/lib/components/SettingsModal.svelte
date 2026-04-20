<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  /**
   * SettingsModal 组件 - 用于配置 API Key、Base URL 和 Model
   *
   * 功能：
   * - API Key 输入（密码类型，可切换显示）
   * - Base URL 输入（默认 OpenAI API 地址）
   * - Model 选择（预设常用模型 + 自定义输入）
   * - 保存按钮（保存到系统 keyring 和本地存储）
   */

  // Props
  interface Props {
    isOpen: boolean;
    onClose: () => void;
  }

  let { isOpen, onClose }: Props = $props();

  // State
  let apiKey = $state('');
  let baseUrl = $state('');
  let model = $state('glm-5.1');  // 文本模型配置
  let multimodalModel = $state('qwen-vl-plus');  // 多模态模型配置（处理图片等）
  let customModel = $state('');
  let customMultimodalModel = $state('');
  let useCustomModel = $state(false);
  let useCustomMultimodalModel = $state(false);
  let showApiKey = $state(false);
  let isSaving = $state(false);
  let toastMessage = $state('');
  let toastType = $state<'success' | 'error' | ''>('');

  // 预设常用文本模型列表
  const presetModels = [
    { value: 'glm-5.1', label: 'GLM-5.1 (推荐)', description: '智谱最新模型，综合能力强' },
    { value: 'glm-4', label: 'GLM-4', description: '智谱上一代主力模型' },
    { value: 'qwen-plus', label: 'Qwen-Plus', description: '阿里通义千问 Plus 版本' },
    { value: 'qwen-turbo', label: 'Qwen-Turbo', description: '阿里通义千问 Turbo 版本（快速）' },
    { value: 'qwen-max', label: 'Qwen-Max', description: '阿里通义千问 Max 版本（最强）' },
  ];

  // 预设常用多模态模型列表（支持图片输入）
  const presetMultimodalModels = [
    { value: 'qwen-vl-plus', label: 'Qwen-VL-Plus (推荐)', description: '通义千问多模态 Plus，支持图片理解' },
    { value: 'qwen-vl-max', label: 'Qwen-VL-Max', description: '通义千问多模态 Max，最强图片理解' },
    { value: 'glm-4v', label: 'GLM-4V', description: '智谱多模态模型，支持图片理解' },
  ];

  // 初始化：加载已保存的配置
  onMount(async () => {
    try {
      // 从 Rust 后端加载配置
      const savedApiKey = await invoke<string>('get_api_key');
      const savedBaseUrl = await invoke<string>('get_base_url');
      const savedModel = await invoke<string>('get_model');
      const savedMultimodalModel = await invoke<string>('get_multimodal_model');

      apiKey = savedApiKey || '';
      // 默认使用 DashScope OpenAI 兼容端点（支持 GLM-5、Qwen 等模型）
      baseUrl = savedBaseUrl || 'https://dashscope.aliyuncs.com/compatible-mode/v1';

      // 检查 savedModel 是否在预设文本模型列表中
      const isPresetModel = presetModels.some(m => m.value === savedModel);
      if (isPresetModel) {
        model = savedModel;
        useCustomModel = false;
      } else if (savedModel) {
        model = 'custom';
        customModel = savedModel;
        useCustomModel = true;
      } else {
        model = 'glm-5.1';
      }

      // 检查 savedMultimodalModel 是否在预设多模态模型列表中
      const isPresetMultimodal = presetMultimodalModels.some(m => m.value === savedMultimodalModel);
      if (isPresetMultimodal) {
        multimodalModel = savedMultimodalModel;
        useCustomMultimodalModel = false;
      } else if (savedMultimodalModel) {
        multimodalModel = 'custom';
        customMultimodalModel = savedMultimodalModel;
        useCustomMultimodalModel = true;
      } else {
        multimodalModel = 'qwen-vl-plus';
      }
    } catch (error) {
      console.error('加载配置失败:', error);
      baseUrl = 'https://dashscope.aliyuncs.com/compatible-mode/v1';
      model = 'glm-5.1';
      multimodalModel = 'qwen-vl-plus';
    }
  });

  /**
   * 切换 API Key 显示/隐藏
   */
  function toggleApiKeyVisibility(): void {
    showApiKey = !showApiKey;
  }

  /**
   * 显示 Toast 提示
   * @param message - 提示消息
   * @param type - 提示类型（success/error）
   */
  function showToast(message: string, type: 'success' | 'error'): void {
    toastMessage = message;
    toastType = type;

    // 3秒后自动消失
    setTimeout(() => {
      toastMessage = '';
      toastType = '';
    }, 3000);
  }

  /**
   * 保存配置到本地存储
   */
  async function saveSettings(): Promise<void> {
    isSaving = true;

    // 获取实际要保存的 model 值
    const actualModel = useCustomModel ? customModel : model;
    // 获取实际要保存的多模态模型值
    const actualMultimodalModel = useCustomMultimodalModel ? customMultimodalModel : multimodalModel;

    // 调试日志：确认保存前的值
    console.log('[SettingsModal] 开始保存配置');
    console.log('[SettingsModal] API Key 长度:', apiKey.length);
    console.log('[SettingsModal] Base URL:', baseUrl);
    console.log('[SettingsModal] 文本模型:', actualModel);
    console.log('[SettingsModal] 多模态模型:', actualMultimodalModel);

    try {
      // 保存 API Key 到本地存储
      if (apiKey) {
        const result = await invoke('save_api_key', { key: apiKey });
      } else {
        await invoke('delete_api_key');
      }

      // 保存 Base URL 到本地存储
      const urlResult = await invoke('save_base_url', { url: baseUrl });

      // 保存文本模型到本地存储
      if (actualModel) {
        const modelResult = await invoke('save_model', { model: actualModel });
      }

      // 保存多模态模型到本地存储
      if (actualMultimodalModel) {
        const multimodalResult = await invoke('save_multimodal_model', { model: actualMultimodalModel });
      }

      showToast('配置已保存', 'success');
      onClose();
    } catch (error) {
      console.error('[SettingsModal] 保存配置失败:', error);
      showToast(`保存失败: ${error}`, 'error');
    } finally {
      isSaving = false;
    }
  }

  /**
   * 处理模态框关闭
   */
  function handleClose(): void {
    onClose();
  }

  /**
   * 处理背景点击关闭
   * @param event - 点击事件
   */
  function handleOverlayClick(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      handleClose();
    }
  }

  /**
   * 处理背景键盘事件
   * @param event - 键盘事件
   */
  function handleOverlayKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      handleClose();
    }
  }

  /**
   * 处理键盘事件（ESC 关闭）
   */
  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      handleClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isOpen}
  <!-- 模态框背景遮罩 -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="modal-overlay"
    onclick={handleOverlayClick}
    onkeydown={handleOverlayKeydown}
    role="dialog"
    aria-modal="true"
    aria-labelledby="settings-modal-title"
    tabindex="-1"
  >
    <!-- 模态框内容 -->
    <div class="modal-content">
      <!-- 模态框头部 -->
      <div class="modal-header">
        <h2 id="settings-modal-title" class="modal-title">设置</h2>
        <button
          class="modal-close-btn"
          onclick={handleClose}
          aria-label="关闭"
          type="button"
        >
          ×
        </button>
      </div>

      <!-- 模态框主体 -->
      <div class="modal-body">
        <!-- API Key 输入 -->
        <div class="form-group">
          <label for="api-key-input" class="form-label">API Key</label>
          <div class="password-wrapper">
            <input
              id="api-key-input"
              type={showApiKey ? 'text' : 'password'}
              class="form-input"
              bind:value={apiKey}
              placeholder="sk-..."
              autocomplete="off"
            />
            <button
              class="password-toggle-btn"
              onclick={toggleApiKeyVisibility}
              aria-label={showApiKey ? '隐藏 API Key' : '显示 API Key'}
              type="button"
            >
              {showApiKey ? '🙈' : '👁️'}
            </button>
          </div>
          <small class="form-hint">
            API Key 将安全存储在系统 keyring 中
          </small>
        </div>

        <!-- Base URL 输入 -->
        <div class="form-group">
          <label for="base-url-input" class="form-label">Base URL</label>
          <input
            id="base-url-input"
            type="text"
            class="form-input"
            bind:value={baseUrl}
            placeholder="https://dashscope.aliyuncs.com/compatible-mode/v1"
            autocomplete="off"
          />
          <small class="form-hint">
            默认为 DashScope OpenAI 兼容端点，支持 GLM-5、Qwen 等模型
          </small>
        </div>

        <!-- 文本模型选择 -->
        <div class="form-group">
          <label for="model-select" class="form-label">文本模型</label>
          <select
            id="model-select"
            class="form-input form-select"
            bind:value={model}
            onchange={() => {
              useCustomModel = model === 'custom';
            }}
          >
            {#each presetModels as m}
              <option value={m.value}>{m.label}</option>
            {/each}
            <option value="custom">自定义文本模型...</option>
          </select>
          {#if model !== 'custom'}
            <small class="form-hint model-description">
              {presetModels.find(m => m.value === model)?.description || ''}
            </small>
          {:else}
            <div class="custom-model-input">
              <input
                id="custom-model-input"
                type="text"
                class="form-input"
                bind:value={customModel}
                placeholder="输入文本模型名称（如 gpt-4、deepseek-chat）"
                autocomplete="off"
              />
              <small class="form-hint">
                输入其他文本模型名称，需确保 Base URL 支持
              </small>
            </div>
          {/if}
          <small class="form-hint hint-note">用于纯文本分析、人物提取、深度推理</small>
        </div>

        <!-- 多模态模型选择 -->
        <div class="form-group">
          <label for="multimodal-model-select" class="form-label">多模态模型</label>
          <select
            id="multimodal-model-select"
            class="form-input form-select"
            bind:value={multimodalModel}
            onchange={() => {
              useCustomMultimodalModel = multimodalModel === 'custom';
            }}
          >
            {#each presetMultimodalModels as m}
              <option value={m.value}>{m.label}</option>
            {/each}
            <option value="custom">自定义多模态模型...</option>
          </select>
          {#if multimodalModel !== 'custom'}
            <small class="form-hint model-description">
              {presetMultimodalModels.find(m => m.value === multimodalModel)?.description || ''}
            </small>
          {:else}
            <div class="custom-model-input">
              <input
                id="custom-multimodal-model-input"
                type="text"
                class="form-input"
                bind:value={customMultimodalModel}
                placeholder="输入多模态模型名称（如 gpt-4o、claude-3-vision）"
                autocomplete="off"
              />
              <small class="form-hint">
                输入支持图片输入的多模态模型名称
              </small>
            </div>
          {/if}
          <small class="form-hint hint-note">仅在上传图片时自动切换使用，处理图片理解</small>
        </div>
      </div>

      <!-- 模态框底部按钮 -->
      <div class="btn-group">
        <button
          class="btn btn-secondary"
          onclick={handleClose}
          type="button"
        >
          取消
        </button>
        <button
          class="btn btn-primary"
          onclick={saveSettings}
          disabled={isSaving}
          type="button"
        >
          {isSaving ? '保存中...' : '保存'}
        </button>
      </div>
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
  .modal-body {
    padding: 0;
  }

  .form-hint {
    display: block;
    margin-top: 4px;
    font-size: 0.68rem;
    color: var(--text-secondary);
    opacity: 0.8;
  }

  /* Model 选择下拉框样式 */
  .form-select {
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 12 12'%3E%3Cpath fill='%23999' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 10px center;
    padding-right: 30px;
    cursor: pointer;
  }

  .model-description {
    color: var(--text-secondary);
    font-weight: 500;
  }

  .custom-model-input {
    margin-top: 6px;
  }

  .hint-note {
    display: block;
    margin-top: 2px;
    color: var(--text-muted);
    font-size: 0.62rem;
    opacity: 0.6;
  }
</style>