<script lang="ts">
  /**
   * MaterialInputPanel 组件 - 对话式输入面板
   *
   * 设计理念：
   * - 类似 ChatGPT/Claude 的对话输入框
   * - 单一输入框，自动识别文字/链接类型
   * - 输入框内部布局：顶部文本区，底部工具栏（模型按钮+文件上传+发送按钮）
   * - 模型选择按钮在发送按钮左边（框内底部）
   * - 多模态模型才能显示文件上传按钮
   * - 已添加条目（文字/链接/文件）在输入框上方展示
   *
   * 交互逻辑：
   * - 输入框有内容时点击发送 → 把输入内容+已上传文件一起提交分析
   * - Enter 发送，Shift+Enter 换行
   * - 文件上传按钮仅在多模态模型下可见
   */

  // ========== 静态导入 ==========

  // Tauri dialog 插件：用于打开系统原生文件选择对话框
  import { open } from '@tauri-apps/plugin-dialog';
  // Tauri fs 插件：用于读取选中文件的内容（文本和二进制）
  import { readTextFile, readFile } from '@tauri-apps/plugin-fs';
  // 模型选择组件
  import ModelSelector, { type ModelConfig } from './ModelSelector.svelte';
  // 参数配置组件（嵌入输入框底部）
  import AnalysisConfigPanel, { type AnalysisConfig } from './AnalysisConfigPanel.svelte';

  // ========== 类型定义 ==========

  /**
   * 输入项类型：文字、链接、文件
   */
  type InputItemType = 'text' | 'url' | 'file';

  /**
   * 输入项数据结构
   */
  interface InputItem {
    /** 唯一标识符 */
    id: string;
    /** 类型：文字/链接/文件 */
    type: InputItemType;
    /** 内容（文字/链接原文，文件为文件内容文本，图片为描述信息） */
    content: string;
    /** 文件名（仅文件类型有） */
    fileName?: string;
    /** 图片 base64 数据（仅图片文件有，用于传给后端多模态 API） */
    base64Data?: string;
    /** 添加时间戳 */
    timestamp: number;
  }

  // ========== Props ==========

  interface Props {
    /** 发送回调：点击发送按钮时触发，传入所有输入项 */
    onSubmit: (items: InputItem[]) => void;
    /** 是否正在分析中（分析中禁用发送） */
    isAnalyzing?: boolean;
    /** 初始输入项（用于重新分析时恢复已有材料） */
    initialItems?: InputItem[];
    /** 当前分析参数配置（由父组件管理） */
    config: AnalysisConfig;
    /** 配置变更回调 */
    onConfigChange: (config: AnalysisConfig) => void;
  }

  let { onSubmit, isAnalyzing = false, initialItems = [], config, onConfigChange }: Props = $props();

  // ========== 内部状态 ==========

  /** 当前输入框内容 */
  let inputValue = $state('');

  /** 输入框是否获得焦点（用于样式切换） */
  let isFocused = $state(false);

  /** 已添加的输入项列表（文字、链接、文件） */
  let inputItems: InputItem[] = $state(initialItems);

  // 【核心修复】监听 initialItems prop 变化，同步到内部状态
  // Svelte 5 中 $state(initialItems) 只在组件首次创建时初始化一次
  // 后续 initialItems prop 变化不会自动同步到内部 inputItems
  // 因此需要用 $effect 显式同步，确保父组件清空 inputItems 时子组件也能感知并清空
  $effect(() => {
    inputItems = initialItems;
  });

  /** ModelSelector 组件的引用（用于获取多模态状态） */
  let modelSelectorRef: ModelSelector;

  /** 当前模型是否支持多模态（由 ModelSelector 更新） */
  let currentModelMultimodal = $state(false);

  /**
   * 模型配置变更回调：ModelSelector 选中模型后更新多模态状态
   */
  function handleModelChange(config: ModelConfig): void {
    currentModelMultimodal = config.isMultimodal;
    console.log('输入面板：当前模型切换:', config.displayName, '多模态:', config.isMultimodal);
  }

  // ========== 方法 ==========

  /**
   * 生成唯一 ID
   */
  function generateId(): string {
    return `item-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
  }

  /**
   * 判断输入内容是否为 URL
   */
  function isUrl(content: string): boolean {
    const trimmed = content.trim();
    return trimmed.startsWith('http://') || trimmed.startsWith('https://');
  }

  /**
   * 添加输入框内容为一条输入项，然后触发发送分析
   */
  function submitAndAnalyze(): void {
    const trimmed = inputValue.trim();

    // 如果输入框有内容，先加入列表
    if (trimmed) {
      // URL 验证
      if (isUrl(trimmed)) {
        try {
          new URL(trimmed);
        } catch {
          alert('请输入有效的网址格式（如 https://example.com）');
          return;
        }
      }

      const item: InputItem = {
        id: generateId(),
        type: isUrl(trimmed) ? 'url' : 'text',
        content: trimmed,
        timestamp: Date.now(),
      };
      inputItems = [...inputItems, item];
      inputValue = ''; // 清空输入框
    }

    // 验证是否有材料
    if (inputItems.length === 0) {
      alert('请输入文字、粘贴链接或上传文件后再发送');
      return;
    }

    // 触发分析
    onSubmit(inputItems);
  }

  /**
   * 处理键盘事件：Enter 发送，Shift+Enter 换行
   */
  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      submitAndAnalyze();
    }
  }

  /**
   * 打开文件选择对话框并上传文件
   *
   * 实现思路：
   * - 方案 A：优先使用 Tauri dialog + fs 插件（桌面端最佳体验）
   * - 方案 B：Tauri 不可用时，使用浏览器原生 <input type="file"> 作为后备
   *
   * 跨系统兼容性：
   * - macOS/Linux/Windows：Tauri dialog 调用系统原生文件选择器
   * - 网页环境：降级为浏览器原生 file input
   */
  async function openFileDialog(): Promise<void> {
    // 先尝试 Tauri 方案（桌面端，调用系统原生文件选择器）
    try {
      // 不限制文件类型，用户可以选择任何文件
      const selected = await open({
        multiple: true,
      });

      // 用户取消选择
      if (!selected) return;

      // 处理选中的文件
      const filePaths: string[] = Array.isArray(selected) ? selected : [selected];

      for (const filePath of filePaths) {
        try {
          const fileName = filePath.split(/[\\/]/).pop() || filePath;
          const extension = fileName.split('.').pop()?.toLowerCase() || '';

          // 判断文件类型：文本类文件读取内容，二进制文件只传描述信息
          const textExtensions = ['txt', 'md', 'json', 'csv', 'html', 'xml', 'log', 'yaml', 'yml', 'ini', 'conf', 'cfg', 'rtf', 'tsv', 'toml'];
          const isTextFile = textExtensions.includes(extension);

          if (isTextFile) {
            // 文本文件：直接读取内容
            const content = await readTextFile(filePath);
            const maxContentLength = 1000000;
            const truncatedContent = content.length > maxContentLength
              ? content.slice(0, maxContentLength) + '\n\n[文件内容过长，已截断]'
              : content;

            const item: InputItem = {
              id: generateId(),
              type: 'file',
              content: truncatedContent,
              fileName,
              timestamp: Date.now(),
            };
            inputItems = [...inputItems, item];
          } else if (extension === 'pdf' || extension === 'doc' || extension === 'docx') {
            // PDF/Word 文档：读取为文本（readTextFile 会尝试解码，失败则传描述信息）
            try {
              const content = await readTextFile(filePath);
              const maxContentLength = 1000000;
              const truncatedContent = content.length > maxContentLength
                ? content.slice(0, maxContentLength) + '\n\n[文件内容过长，已截断]'
                : content;

              const item: InputItem = {
                id: generateId(),
                type: 'file',
                content: truncatedContent,
                fileName,
                timestamp: Date.now(),
              };
              inputItems = [...inputItems, item];
            } catch {
              // PDF/Word 是二进制格式，readTextFile 会失败
              // 传描述信息给大模型，让它知道用户上传了这个文件
              const item: InputItem = {
                id: generateId(),
                type: 'file',
                content: `[用户上传了文件: ${fileName}，文件类型为 ${extension.toUpperCase()} 格式。AI 请根据文件名和类型推断可能的内容。]`,
                fileName,
                timestamp: Date.now(),
              };
              inputItems = [...inputItems, item];
            }
          } else {
            // 图片及其他二进制文件：图片读取为 base64，其他传描述信息
            const imageExtensions = ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp', 'svg', 'ico', 'tiff', 'tif'];
            if (imageExtensions.includes(extension)) {
              // 图片文件：读取为二进制并转为 base64 编码，用于多模态 API
              try {
                const uint8Array = await readFile(filePath);
                // 将 Uint8Array 转为 base64 字符串
                let binary = '';
                for (let i = 0; i < uint8Array.length; i++) {
                  binary += String.fromCharCode(uint8Array[i]);
                }
                const base64Data = btoa(binary);

                const item: InputItem = {
                  id: generateId(),
                  type: 'file',
                  content: `[用户上传了图片文件: ${fileName}]`,
                  fileName,
                  base64Data, // 图片 base64 数据，传给后端多模态 API
                  timestamp: Date.now(),
                };
                inputItems = [...inputItems, item];
              } catch (imgError) {
                // 图片读取失败，传描述信息作为兜底
                console.error('图片读取为 base64 失败:', filePath, imgError);
                const item: InputItem = {
                  id: generateId(),
                  type: 'file',
                  content: `[用户上传了图片文件: ${fileName}，但读取失败，请根据文件名推断可能的内容]`,
                  fileName,
                  timestamp: Date.now(),
                };
                inputItems = [...inputItems, item];
              }
            } else {
              // 其他二进制文件：传描述信息
              const description = `[用户上传了文件: ${fileName}，文件类型为 ${extension.toUpperCase()} 格式]`;
              const item: InputItem = {
                id: generateId(),
                type: 'file',
                content: description,
                fileName,
                timestamp: Date.now(),
              };
              inputItems = [...inputItems, item];
            }
          }
        } catch (readError) {
          const fileName = filePath.split(/[\\/]/).pop() || filePath;
          alert(`文件 "${fileName}" 读取失败`);
          console.error('文件读取失败:', filePath, readError);
        }
      }
      return; // Tauri 方案成功，直接返回
    } catch (tauriError) {
      // Tauri 插件不可用（网页环境或插件未注册），降级为浏览器原生方案
      console.warn('Tauri dialog/fs 插件不可用，降级为浏览器原生文件选择:', tauriError);
    }

    // 方案 B：浏览器原生 file input 后备方案
    try {
      const fileInput = document.createElement('input');
      fileInput.type = 'file';
      fileInput.multiple = true;
      // 不限制文件类型
      fileInput.accept = '*';
      fileInput.style.display = 'none';

      fileInput.onchange = async () => {
        const files = fileInput.files;
        if (!files || files.length === 0) return;

        for (const file of files) {
          try {
            const fileName = file.name;
            const extension = fileName.split('.').pop()?.toLowerCase() || '';
            const textExtensions = ['txt', 'md', 'json', 'csv', 'html', 'xml', 'log', 'yaml', 'yml', 'ini', 'conf', 'cfg', 'rtf', 'tsv', 'toml'];
            const isTextFile = textExtensions.includes(extension);

            if (isTextFile) {
              // 文本文件：用 FileReader 读取内容
              const content = await readFileAsText(file);
              const maxContentLength = 1000000;
              const truncatedContent = content.length > maxContentLength
                ? content.slice(0, maxContentLength) + '\n\n[文件内容过长，已截断]'
                : content;

              const item: InputItem = {
                id: generateId(),
                type: 'file',
                content: truncatedContent,
                fileName,
                timestamp: Date.now(),
              };
              inputItems = [...inputItems, item];
            } else {
              // 图片/PDF/Word等二进制文件：图片读取为 base64，其他传描述信息
              const imageExtensions = ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp', 'svg', 'ico', 'tiff', 'tif'];
              if (imageExtensions.includes(extension)) {
                // 图片文件：使用 FileReader 读取为 base64
                try {
                  const base64Data = await readFileAsDataURL(file);
                  // dataURL 格式为 "data:image/jpeg;base64,XXXXX"，提取纯 base64 部分
                  const pureBase64 = base64Data.split(',')[1] || base64Data;

                  const item: InputItem = {
                    id: generateId(),
                    type: 'file',
                    content: `[用户上传了图片文件: ${fileName}]`,
                    fileName,
                    base64Data: pureBase64, // 图片 base64 数据
                    timestamp: Date.now(),
                  };
                  inputItems = [...inputItems, item];
                } catch (imgError) {
                  console.error('图片读取为 base64 失败:', file.name, imgError);
                  const item: InputItem = {
                    id: generateId(),
                    type: 'file',
                    content: `[用户上传了图片文件: ${fileName}，但读取失败]`,
                    fileName,
                    timestamp: Date.now(),
                  };
                  inputItems = [...inputItems, item];
                }
              } else {
                // 其他二进制文件：传描述信息
                const description = `[用户上传了文件: ${fileName}，文件类型为 ${extension.toUpperCase()} 格式]`;
                const item: InputItem = {
                  id: generateId(),
                  type: 'file',
                  content: description,
                  fileName,
                  timestamp: Date.now(),
                };
                inputItems = [...inputItems, item];
              }
            }
          } catch (readError) {
            alert(`文件 "${file.name}" 读取失败`);
            console.error('文件读取失败:', file.name, readError);
          }
        }

        document.body.removeChild(fileInput);
      };

      document.body.appendChild(fileInput);
      fileInput.click();
    } catch (browserError) {
      console.error('浏览器文件选择也失败:', browserError);
      alert('文件选择功能暂不可用');
    }
  }

  /**
   * 使用浏览器 FileReader 将 File 对象读取为文本
   * 用于 Tauri 不可用时的后备方案
   */
  function readFileAsText(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(new Error(`读取文件 ${file.name} 失败`));
      reader.readAsText(file);
    });
  }

  /**
   * 使用浏览器 FileReader 将 File 对象读取为 Data URL（base64 编码）
   * 用于图片文件的 base64 编码，作为 Tauri 不可用时的后备方案
   */
  function readFileAsDataURL(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.onerror = () => reject(new Error(`读取图片 ${file.name} 失败`));
      reader.readAsDataURL(file);
    });
  }

  /**
   * 删除单个输入项
   */
  function deleteItem(id: string): void {
    inputItems = inputItems.filter((item) => item.id !== id);
  }

  /**
   * 清空所有输入项
   */
  function clearAll(): void {
    inputItems = [];
  }

  /**
   * 截断显示内容
   */
  function truncateContent(content: string, maxLength: number = 80): string {
    if (content.length <= maxLength) return content;
    return content.slice(0, maxLength) + '...';
  }

  /**
   * 获取类型标签文本
   */
  function getTypeLabel(type: InputItemType): string {
    const labels = { text: '文字', url: '链接', file: '文件' };
    return labels[type];
  }

  /**
   * 判断发送按钮是否可用
   * 输入框有内容 或 已有条目 就可以发送
   */
  function canSubmit(): boolean {
    return (inputValue.trim().length > 0 || inputItems.length > 0) && !isAnalyzing;
  }
</script>

<!-- 居中标题（输入框上方） -->
<div class="app-title-area">
  <h1 class="app-title-text">TellMeWhy</h1>
</div>

<!-- 对话式输入框（整体框结构） -->
<div class="chat-input-box" class:focused={isFocused}>
  <!-- 已添加条目展示区（在 textarea 上方，框内） -->
  {#if inputItems.length > 0}
    <div class="items-area">
      {#each inputItems as item (item.id)}
        <div class="item-card" class:item-card-url={item.type === 'url'} class:item-card-file={item.type === 'file'} class:item-card-text={item.type === 'text'}>
          <!-- 卡片内容区域（正方形主体） -->
          <div class="item-card-body">
            {#if item.type === 'file'}
              <!-- 文件卡片：显示文件图标 + 文件名 -->
              <div class="item-card-icon">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                  <polyline points="14 2 14 8 20 8"/>
                </svg>
              </div>
              <span class="item-card-name" title={item.fileName || item.content}>
                {item.fileName || truncateContent(item.content, 12)}
              </span>
            {:else if item.type === 'url'}
              <!-- 链接卡片：显示链接图标 + 截断链接 -->
              <div class="item-card-icon item-card-icon-link">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
                  <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
                </svg>
              </div>
              <span class="item-card-name" title={item.content}>
                {truncateContent(item.content, 12)}
              </span>
            {:else}
              <!-- 文字卡片：显示文字图标 + 截断文字 -->
              <div class="item-card-icon item-card-icon-text">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
                  <path d="M7 8h10M7 12h10M7 16h6"/>
                </svg>
              </div>
              <span class="item-card-name" title={item.content}>
                {truncateContent(item.content, 12)}
              </span>
            {/if}
          </div>
          <!-- 删除按钮（右上角 × ） -->
          <button
            class="item-card-delete"
            onclick={() => deleteItem(item.id)}
            title="删除"
            aria-label="删除此输入项"
          >
            ✕
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- 文本输入区 -->
  <textarea
    class="chat-textarea"
    placeholder="输入文字，按 Enter 发送..."
    bind:value={inputValue}
    onfocus={() => isFocused = true}
    onblur={() => isFocused = false}
    onkeydown={handleKeydown}
    rows="2"
  ></textarea>

  <!-- 框内底部操作栏：左下角上传按钮，右下角模型+发送 -->
  <div class="input-bottom-bar">
    <!-- 左下角：上传文件按钮 -->
    {#if currentModelMultimodal}
      <button
        class="upload-btn"
        onclick={openFileDialog}
        title="上传文件"
        aria-label="上传文件"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48"/>
        </svg>
      </button>
    {:else}
      <!-- 无多模态模型时也显示上传按钮，但禁用 -->
      <button
        class="upload-btn upload-btn-disabled"
        onclick={openFileDialog}
        disabled
        title="当前模型不支持文件上传，请配置多模态模型"
        aria-label="上传文件（不可用）"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48"/>
        </svg>
      </button>
    {/if}

    <!-- 右下角：模型选择按钮 + 发送按钮，紧挨在一起 -->
    <div class="right-actions">
      <ModelSelector
        bind:this={modelSelectorRef}
        onModelChange={handleModelChange}
      />
      <button
        class="send-btn"
        onclick={submitAndAnalyze}
        disabled={!canSubmit()}
        title="发送分析"
        aria-label="发送分析"
      >
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M8 2L3 7h3v7h2V7h3L8 2z" fill="currentColor"/>
        </svg>
      </button>
    </div>
  </div>
</div>

<!-- 框外底部：参数配置条 -->
<div class="config-bar-outside">
  <AnalysisConfigPanel
    config={config}
    onChange={onConfigChange}
  />
</div>

<style>
  /* ========== 居中标题 ========== */

  .app-title-area {
    width: 700px;
    max-width: 700px;
    margin: -100px auto 20px;
    text-align: center;
  }

  .app-title-text {
    font-size: 2.5rem;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
    letter-spacing: -0.02em;
  }

  /* ========== 正方形卡片条目展示区（框内 textarea 上方） ========== */

  .items-area {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 10px 14px 4px;
    overflow: visible;
  }

  /* 单个正方形卡片 */
  .item-card {
    position: relative;
    width: 72px;
    height: 72px;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
    cursor: default;
    overflow: hidden;
    box-sizing: border-box;
    padding: 6px 4px;
  }

  /* 文件卡片：浅绿色背景 */
  .item-card-file {
    background: rgba(16, 163, 127, 0.06);
    border: 1px solid rgba(16, 163, 127, 0.2);
  }

  .item-card-file:hover {
    background: rgba(16, 163, 127, 0.12);
  }

  /* 链接卡片：浅蓝色背景 */
  .item-card-url {
    background: rgba(59, 130, 246, 0.06);
    border: 1px solid rgba(59, 130, 246, 0.2);
  }

  .item-card-url:hover {
    background: rgba(59, 130, 246, 0.12);
  }

  /* 文字卡片：浅灰色背景 */
  .item-card-text {
    background: var(--bg-card);
    border: 1px solid var(--border);
  }

  .item-card-text:hover {
    background: var(--bg-hover);
  }

  /* 卡片内容区域（图标 + 文件名） */
  .item-card-body {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    width: 100%;
  }

  /* 卡片图标 */
  .item-card-icon {
    color: #10a37f;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .item-card-icon-link {
    color: #3b82f6;
  }

  .item-card-icon-text {
    color: var(--text-secondary);
  }

  /* 卡片名称（文件名/链接/文字） */
  .item-card-name {
    font-size: 0.55rem;
    line-height: 1.2;
    color: var(--text-secondary);
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
    word-break: break-all;
  }

  /* 删除按钮（右上角 × ） */
  .item-card-delete {
    position: absolute;
    top: 2px;
    right: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.55rem;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
    opacity: 0;
    line-height: 1;
    padding: 0;
  }

  .item-card:hover .item-card-delete {
    opacity: 1;
  }

  .item-card-delete:hover {
    color: var(--error);
    background: rgba(220, 38, 38, 0.06);
  }

  /* ========== 对话式输入框 ========== */

  .chat-input-box {
    width: 700px;
    max-width: 700px;
    margin: 0 auto;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 14px;
    background: var(--bg-secondary);
    transition: border-color 0.15s, box-shadow 0.15s;
    box-shadow: var(--shadow-sm);
    overflow: visible;
  }

  .chat-input-box.focused {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  /* 文本输入区 */
  .chat-textarea {
    width: 100%;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text-primary);
    font-size: 0.82rem;
    font-family: inherit;
    line-height: 1.5;
    resize: none;
    padding: 10px 14px;
    min-height: 44px;
    max-height: 120px;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .chat-textarea::placeholder {
    color: var(--text-muted);
  }

  .chat-textarea::-webkit-scrollbar {
    width: 3px;
  }

  .chat-textarea::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 2px;
  }

  /* 框内底部操作栏 */
  .input-bottom-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 10px 8px;
    gap: 0;
  }

  /* 上传按钮 */
  .upload-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: transparent;
    color: var(--text-secondary);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
    flex-shrink: 0;
  }

  .upload-btn:hover {
    color: var(--accent);
    background: var(--accent-light);
  }

  /* 禁用状态的上传按钮 */
  .upload-btn-disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .upload-btn-disabled:hover {
    color: var(--text-secondary);
    background: transparent;
  }

  /* 右侧操作区：模型按钮 + 发送按钮，保持适当间距 */
  .right-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* 发送按钮 */
  .send-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--accent);
    /* 文字/图标跟主背景色反色：浅色主题下 accent 是黑 -> 图标白；暗色主题下 accent 是白 -> 图标黑 */
    color: var(--bg-primary);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
    flex-shrink: 0;
  }

  .send-btn:hover:not(:disabled) {
    background: var(--accent-hover);
    transform: scale(1.05);
  }

  .send-btn:disabled {
    background: var(--border);
    color: var(--text-muted);
    cursor: not-allowed;
  }

  /* 框外底部：参数配置条 */
  .config-bar-outside {
    width: 70%;
    max-width: 820px;
    margin: 0 auto;
    padding: 20px 0;
  }
</style>