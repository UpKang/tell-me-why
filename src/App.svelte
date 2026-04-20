<script lang="ts">
  import MaterialInputPanel from './lib/components/MaterialInputPanel.svelte';
  import AnalysisConfigPanel from './lib/components/AnalysisConfigPanel.svelte';
  import type { CharacterProfile, CharacterGenerationResult } from './lib/components/CharacterProfilePanel.svelte';
  import ThemeToggle from './lib/components/ThemeToggle.svelte';
  import BackButton from './lib/components/BackButton.svelte';
  import type { AnalysisConfig } from './lib/components/AnalysisConfigPanel.svelte';
  // Phase 1 新增：新的吃瓜版结果页容器
  import ResultView from './lib/components/result/ResultView.svelte';
  import type { TruthExtras, TruthAnalysisResult } from './lib/components/result/types';
  // 历史记录列表用：把 session 浓缩成一句话精华（含老记录回退逻辑）
  import { extractEssence } from './lib/utils/extractEssence';
  import { onMount } from 'svelte';
  // Channel：用于接收 Rust 后端推送的流式事件（Phase 1 新增）
  import { invoke, Channel } from '@tauri-apps/api/core';
  // 导入 DOMPurify 用于 HTML 安全净化（防止 XSS 攻击）
  import DOMPurify from 'dompurify';
  // Phase 2-A：marked 用于把大模型流式输出的 Markdown 实时转成 HTML
  // async:false 保证同步返回 string（流式场景每帧都要渲染，异步会把顺序打乱）
  import { marked } from 'marked';
  // 导入 Tauri 对话框插件，用于弹出原生保存文件对话框
  import { save } from '@tauri-apps/plugin-dialog';
  // 导入 Tauri 文件系统插件，用于将文本内容写入文件
  import { writeTextFile } from '@tauri-apps/plugin-fs';

  // marked 全局配置：
  // - gfm: 支持 GitHub 风格（表格、删除线、任务列表）
  // - breaks: 单换行也换行（中文用户写作习惯）
  marked.setOptions({ gfm: true, breaks: true });

  // ========== Toast 提示状态（Svelte 5 $state rune） ==========
  // Toast 是否可见
  let toastVisible = $state(false);
  // Toast 消息内容
  let toastMsg = $state('');
  // Toast 类型：success、error、info
  let toastType = $state<'success' | 'error' | 'info' | ''>('');

  /**
   * 显示 Toast 提示
   *
   * @param msg - 提示消息内容
   * @param type - 提示类型（success / error / info）
   * @param duration - 显示时长（毫秒），默认 3000
   */
  function showToast(msg: string, type: 'success' | 'error' | 'info', duration = 3000): void {
    toastMsg = msg;
    toastType = type;
    toastVisible = true;
    setTimeout(() => {
      toastVisible = false;
      toastMsg = '';
      toastType = '';
    }, duration);
  }

  /**
   * TellMeWhy 应用主组件 - 真相推测分析平台
   *
   * 功能：
   * - 左侧：添加信息资料（文字、网页 URL）
   * - 右侧：设置分析参数（温度、公正严明程度、道德底线等）
   * - 开始分析：调用大模型进行真相推测
   * - 人物画像：提取人物并选择动机
   * - 结果展示：显示分析结果（含人物画像卡片）
   * - 历史记录：查看过往分析记录
   *
   * 页面流转逻辑：
   * 1. 首页 (home): 添加信息 → 设置参数 → 开始分析
   * 2. 人物画像 (characters): 显示人物 → 选择动机 → 开始深度分析
   * 3. 分析中 (analyzing): 显示加载动画
   * 4. 结果页 (result): 显示分析结果 → 可返回重新选择
   */

  // ========== 类型定义 ==========

  /**
   * 输入项类型：文字、链接、文件
   */
  type InputItemType = 'text' | 'url' | 'file';

  /**
   * 输入项数据结构
   */
  interface InputItem {
    id: string;
    type: InputItemType;
    content: string;
    fileName?: string;
    /** 图片 base64 数据（仅图片文件有，传给后端多模态 API） */
    base64Data?: string;
    timestamp: number;
  }

  /**
   * 输入材料结构（传给后端）
   */
  interface Material {
    type: string;
    content: string;
    /** 文件名（仅文件/图片类型有） */
    file_name?: string;
  }

  /**
   * 历史记录会话结构
   */
  interface AnalysisSession {
    sessionId: string;
    materials: Material[];
    characters?: CharacterProfile[];
    selectedMotivations?: Record<string, string>;
    analysisResult: string;
    /** Phase 1 新增：吃瓜结构化包；老历史记录没有此字段（undefined） */
    truthExtras?: TruthExtras | null;
    createdAt: number;
    config: AnalysisConfig;
  }

  // ========== 页面状态管理 ==========

  /**
   * 当前页面状态
   * Phase 2-A：合并 'characters' 和 'motivation' 为单一 'analyzing' 流式页。
   * 用户从 home 点发送后，一路流到 result，中间不再中断选动机。
   * 'characters' / 'motivation' 仍保留在类型里仅为历史记录向后兼容，不再使用。
   *
   * - home:      首页（添加材料）
   * - analyzing: 流式分析进行中（人物抽取 + 真相推演，同一页面）
   * - result:    结果页
   * - history:   历史记录页
   */
  type PageState = 'home' | 'characters' | 'motivation' | 'analyzing' | 'result' | 'history';
  let currentPage: PageState = $state('home');

  /**
   * 详情页的来源记忆
   * --------------
   * 用户进入 result 页的路径有两条：
   *   1. 首页 → 开始分析 → result （source = 'home'）
   *   2. 历史页 → 查看详情 → result （source = 'history'）
   *
   * 返回按钮需要按来源给出正确的目的地：
   *   - 从首页来的 → 返回首页（重置所有数据）
   *   - 从历史来的 → 返回历史页（保留历史列表，方便继续浏览其他记录）
   *
   * 之前没这个状态，导致从历史页看完详情点"返回"会直接清空历史上下文，
   * 回到空白首页，用户体验很挫。
   */
  let resultEntrySource: 'home' | 'history' = $state('home');

  // ========== 输入项管理 ==========

  /** 已添加的输入项列表（由 MaterialInputPanel 内部管理） */
  let inputItems: InputItem[] = $state([]);

  /**
   * 发送分析回调：MaterialInputPanel 触发时，直接开始真相分析
   */
  function handleSubmit(items: InputItem[]): void {
    inputItems = items; // 保存当前输入项（供后续流程使用）
    generateCharacterProfiles();
  }

  // ========== 分析参数管理 ==========

  /** 默认分析参数配置 */
  const defaultConfig: AnalysisConfig = {
    temperature: 0.4,
    fairness: 70,
    morality: 60,
    outputFormat: 'detailed',
    analysisDepth: 'medium',
  };

  /** 当前分析参数配置 */
  let analysisConfig: AnalysisConfig = $state(defaultConfig);

  /**
   * 更新分析参数配置
   */
  function handleConfigChange(config: AnalysisConfig): void {
    analysisConfig = config;
  }

  // ========== 人物画像管理 ==========

  /** 人物画像列表 */
  let characters: CharacterProfile[] = $state([]);

  /** 事件梳理摘要（从人物画像步骤提取，传给深度分析） */
  let eventSummary = $state('');

  // ========== 分析状态 ==========

  /** 是否正在分析 */
  let isAnalyzing = $state(false);

  /** 分析结果（原始 HTML） */
  let analysisResult = $state('');

  /**
   * Phase 1 新增：吃瓜结构化包
   * - 由后端 analyze_truth 返回的 TruthAnalysisResult.extras 写入
   * - 流式阶段期间保持 null（流完才解析）
   * - 老历史记录没有 extras 字段时也保持 null
   */
  let truthExtras: TruthExtras | null = $state(null);

  /** 错误信息 */
  let errorMessage = $state('');

  /** 会话 ID（用于历史记录） */
  let currentSessionId = $state('');

  // ========== 流式分析状态（Phase 1 新增） ==========
  // 与 Rust 端 StreamEvent 枚举一一对应（serde #[serde(tag = "type", rename_all = "camelCase")]）
  type StreamEvent =
    | { type: 'phase'; phase: string; label: string }
    | { type: 'delta'; text: string }
    | { type: 'done'; fullText: string }
    | { type: 'error'; message: string };

  /** 当前流式阶段标识，用于进度条高亮（reading / extracting / preparing / reasoning） */
  let streamPhase = $state('');
  /** 当前流式阶段对用户可读的文案，显示在 loading 页副标题 */
  let streamPhaseLabel = $state('');
  /** 实时拼接的流式文本。人物抽取阶段用于 sentinel 扫描，分析阶段同时充当 markdown 源 */
  let streamingText = $state('');
  /** 分析阶段流式 markdown → 净化 HTML 后的结果（用于实时渲染，节流到 ~80ms 刷新一次） */
  let streamingRenderedHtml = $state('');

  /**
   * Phase 3：提前水合的"一句话锐评预告"
   *
   * 原理：
   * - 大模型按 prompt 约定会最先吐出 <<<GOSSIP_JSON>>>{...}<<<END_GOSSIP>>> 包
   * - 其中 oneLinerVerdict 往往是 JSON 的第一个字段
   * - 只要流里出现 `"oneLinerVerdict": "xxx"` 片段，我们就提前抽出来在分析中页预览
   *
   * 体验收益：用户刚进 analyzing 页几秒就能看到"事件定调"的那一句，
   * 不用干等到整篇长文跑完。
   *
   * 实现细节：
   * - 用宽松正则匹配（允许未闭合的右引号——半截也展示）
   * - 匹配到就更新 streamingVerdict，同一轮反复匹配最后一次胜出（覆盖）
   */
  let streamingVerdict = $state('');
  /** 锐评抓取正则：匹配 "oneLinerVerdict": "（截止到下一对双引号或流结束）" */
  const VERDICT_RE = /"oneLinerVerdict"\s*:\s*"((?:\\.|[^"\\])*)(?:"|$)/;

  /**
   * 从流式文本里尝试抽出一句话锐评
   * 抽取到非空值且与旧值不同时才更新（避免频繁 DOM 刷新）
   */
  function tryHydrateVerdict(buffer: string): void {
    const m = buffer.match(VERDICT_RE);
    if (!m) return;
    // JSON 字符串转义解码（\" → "，\\ → \，\n → 实际换行等）
    let raw = m[1] ?? '';
    try {
      raw = JSON.parse('"' + raw.replace(/\n/g, '\\n') + '"');
    } catch {
      // 半截 JSON 转义可能不完整——直接取原文
    }
    raw = raw.trim();
    if (raw && raw !== streamingVerdict) {
      streamingVerdict = raw;
    }
  }

  // ========== Markdown 渲染管线（Phase 2-A 新增） ==========
  // 流式阶段我们每 80ms 重新 parse 一次 markdown。非流式（结果页、历史记录）
  // 一次性 parse 即可。所有输出都过 DOMPurify 再插入 DOM，防 XSS。

  /** DOMPurify 净化白名单（覆盖 marked 可能产出的所有标签 + 旧 HTML 报告用的自定义类） */
  const SANITIZE_CONFIG = {
    ALLOWED_TAGS: [
      'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
      'p', 'span', 'div', 'br', 'hr',
      'ul', 'ol', 'li',
      'strong', 'em', 'b', 'i', 'del', 's',
      'blockquote', 'code', 'pre',
      'a', 'table', 'thead', 'tbody', 'tr', 'th', 'td',
    ],
    ALLOWED_ATTR: ['class', 'href', 'title'],
    FORBID_TAGS: ['script', 'iframe', 'style', 'object', 'embed', 'form', 'input'],
    FORBID_ATTR: ['onclick', 'onerror', 'onload', 'onmouseover', 'onfocus', 'onblur'],
  };

  /**
   * 检测一段文本是否是老版 HTML（Phase 2-A 之前存的历史记录是 HTML）。
   * 逻辑：以 < 开头或包含 <h2> / <p> 等结构标签即视为 HTML；否则走 markdown。
   */
  function looksLikeHtml(text: string): boolean {
    const t = text.trimStart();
    if (!t.startsWith('<')) return false;
    return /<(h[1-6]|p|ul|ol|div|blockquote|span)\b/i.test(t);
  }

  /**
   * Phase 1：剥离"吃瓜包 sentinel"块。
   *
   * 后端 prompt 会让大模型先输出：
   *   <<<GOSSIP_JSON>>>{...}<<<END_GOSSIP>>>
   * 再继续输出 Markdown 正文。流式渲染期间若不剥离，用户会在页面上
   * 看到一坨 JSON 原文，非常破坏体验；最终 analyze_truth 的 Rust 端也会
   * 剥离后再返回，但 Rust 的剥离只作用于"完整拼接文本"，无法影响我们
   * 前端边流边 parse 的那份 streamingText。
   *
   * 实现：从文本里把第一对 sentinel 中间（含 sentinel 本身）整段去掉；
   * 若只有开头没有结尾（说明 JSON 还在吐），也把它从开头到文本末尾裁掉，
   * 避免半截 JSON 被当作 markdown 渲染。
   */
  function stripGossipSentinel(text: string): string {
    if (!text) return text;
    const START = '<<<GOSSIP_JSON>>>';
    const END = '<<<END_GOSSIP>>>';
    const startIdx = text.indexOf(START);
    if (startIdx === -1) return text;
    const endIdx = text.indexOf(END, startIdx + START.length);
    if (endIdx === -1) {
      // JSON 还在输出中：先把 startIdx 之后全部藏掉
      return text.slice(0, startIdx).trimEnd();
    }
    // 完整剥离 sentinel 块
    return (text.slice(0, startIdx) + text.slice(endIdx + END.length)).trim();
  }

  /**
   * 将 markdown（或老 HTML）渲染为 **已净化** 的 HTML 字符串。
   * 这是 {@html ...} 的唯一入口，所有展示型文本都从这里过。
   */
  function renderContent(text: string): string {
    if (!text) return '';
    const stripped = stripGossipSentinel(text);
    if (!stripped) return '';
    const raw = looksLikeHtml(stripped)
      ? stripped
      : (marked.parse(stripped, { async: false }) as string);
    return DOMPurify.sanitize(raw, SANITIZE_CONFIG);
  }

  // ========== 人物 sentinel 流式抽取 ==========
  // 后端 prompt 要求大模型在最终 JSON 之前，每识别出一个人物就先吐一行：
  //   <<CHAR>>{...}<<END>>
  // 前端边流边扫这些 sentinel，把人物卡片逐张贴到侧栏，体验比"等全部 JSON 完成"好很多。
  // `sentinelCursor` 记录已扫描到的位置，避免每次 delta 都重扫全部。
  const CHAR_SENTINEL_RE = /<<CHAR>>([\s\S]*?)<<END>>/g;

  /**
   * 扫描 streamingText 中新增的部分，解出完整 sentinel 人物块，追加到 characters 数组。
   * 重复 id 会被忽略，保证不会因 sentinel 和最终 JSON 撞车而重复渲染。
   */
  function extractCharactersFromStream(buffer: string, cursor: number): number {
    CHAR_SENTINEL_RE.lastIndex = cursor;
    let match: RegExpExecArray | null;
    let newCursor = cursor;
    while ((match = CHAR_SENTINEL_RE.exec(buffer)) !== null) {
      const jsonStr = match[1].trim();
      newCursor = match.index + match[0].length;
      try {
        const parsed = JSON.parse(jsonStr) as CharacterProfile;
        if (!parsed?.id) continue;
        // 去重：同 id 已在列表中就跳过
        if (characters.some(c => c.id === parsed.id)) continue;
        // 最简兜底：motivations 必须是数组
        if (!Array.isArray(parsed.motivations)) parsed.motivations = [];
        characters = [...characters, parsed];
      } catch (e) {
        // 不完整或损坏的 sentinel：直接跳过，下一轮再处理
        console.warn('[stream] sentinel JSON 解析失败，跳过:', e, jsonStr);
      }
    }
    return newCursor;
  }

  /** markdown 节流计时器句柄。延迟 80ms 合并多次 delta，避免每帧都 parse。 */
  let markdownRenderTimer: number | null = null;
  /** 记录 sentinel 扫描游标（跨 delta 增量扫描） */
  let sentinelCursor = 0;

  /**
   * 创建一个带默认处理器的 Channel，统一订阅 4 种事件到组件状态。
   * 每次调用分析都要新建一个——Channel 不应跨请求复用。
   */
  function createAnalysisChannel(): Channel<StreamEvent> {
    const ch = new Channel<StreamEvent>();
    // 重置所有流式状态（避免上次分析残留干扰这次 UI）
    streamPhase = '';
    streamPhaseLabel = '';
    streamingText = '';
    streamingRenderedHtml = '';
    streamingVerdict = ''; // Phase 3：重置锐评预览
    sentinelCursor = 0;
    if (markdownRenderTimer !== null) {
      clearTimeout(markdownRenderTimer);
      markdownRenderTimer = null;
    }

    ch.onmessage = (evt: StreamEvent) => {
      switch (evt.type) {
        case 'phase':
          streamPhase = evt.phase;
          streamPhaseLabel = evt.label;
          break;

        case 'delta':
          streamingText += evt.text;
          // 人物抽取阶段：从 sentinel 中增量解出人物卡片
          if (streamPhase === 'extracting' || streamPhase === 'reading') {
            sentinelCursor = extractCharactersFromStream(streamingText, sentinelCursor);
          }
          // 推演阶段：节流渲染 markdown + Phase 3 提前水合锐评
          if (streamPhase === 'reasoning') {
            tryHydrateVerdict(streamingText);
            if (markdownRenderTimer === null) {
              markdownRenderTimer = window.setTimeout(() => {
                streamingRenderedHtml = renderContent(streamingText);
                markdownRenderTimer = null;
              }, 80);
            }
          }
          break;

        case 'done':
          // 立刻 flush 一次最终渲染（取消 pending timer，保证最后一帧是完整的）
          if (markdownRenderTimer !== null) {
            clearTimeout(markdownRenderTimer);
            markdownRenderTimer = null;
          }
          if (streamPhase === 'reasoning') {
            streamingRenderedHtml = renderContent(streamingText);
          }
          break;

        case 'error':
          console.warn('[stream] 后端报告错误:', evt.message);
          break;
      }
    };
    return ch;
  }

  // ========== 生命周期 ==========

  onMount(async () => {
    console.log('TellMeWhy 应用已加载');
  });

  // ========== 页面跳转辅助 ==========

  /**
   * 判断是否应该隐藏顶部导航栏
   * 在人物画像页、分析中页、结果页、历史页时隐藏
   * 让用户专注当前流程
   */
  function shouldHideHeader(): boolean {
    return currentPage === 'characters' || currentPage === 'motivation' || currentPage === 'analyzing' || currentPage === 'result' || currentPage === 'history';
  }

  // ========== 分析方法 ==========

  /**
   * 主分析流程入口（Phase 2-A 合并版）
   *
   * 从首页发送材料后：
   * 1. 进入统一的流式 analyzing 页
   * 2. 第一步：人物抽取（generate_characters）——sentinel 让人物卡片流式进侧栏
   * 3. 第二步：真相推演（analyze_truth）——流式 markdown 进主区
   * 4. 完成后切到 result 页
   *
   * 不再中途停在"选动机"页——动机仍会传给深度分析，但由大模型自行判断组合。
   */
  async function generateCharacterProfiles(): Promise<void> {
    // 清空之前的结果（防止显示旧数据）
    characters = [];
    eventSummary = '';
    analysisResult = '';
    truthExtras = null; // Phase 1 新增：清空上次的吃瓜包
    errorMessage = '';
    // 这条流程是"从首页出发"的，结果页返回时应该回到首页
    resultEntrySource = 'home';

    // 验证是否有输入
    if (inputItems.length === 0) {
      errorMessage = '请至少添加一条信息资料（文字或网页链接）';
      currentPage = 'result';
      return;
    }

    // 切换到统一流式页
    currentPage = 'analyzing';
    isAnalyzing = true;
    console.log('开始分析流程，输入项数量:', inputItems.length);

    try {
      // 构建材料列表（图片文件使用 base64Data 作为 content，文本文件使用原文本）
      const materials: Material[] = inputItems.map((item) => {
        const isImageWithBase64 = item.type === 'file' && item.base64Data;
        return {
          type: item.type,
          content: isImageWithBase64 ? item.base64Data! : item.content,
          file_name: item.fileName,
        };
      });

      // ---------- 第一步：人物画像（带 sentinel 流式）----------
      const charChannel = createAnalysisChannel();
      const result: CharacterGenerationResult = await invoke('generate_characters', {
        materials,
        config: analysisConfig,
        onEvent: charChannel,
      });

      if (!isAnalyzing) {
        // 用户在等待期间点击了"取消分析"，isAnalyzing 已被重置为 false
        // 此时不再跳转 result 页面，直接返回首页
        console.log('人物画像生成完成，但用户已取消分析，不再跳转结果页');
        return;
      }

      if (!result.hasCharacters || result.characters.length === 0) {
        errorMessage = result.errorMessage || '材料中没有提取到人物信息，无法进行人物画像分析。';
        currentPage = 'result';
        isAnalyzing = false;
        console.warn('人物画像生成失败:', errorMessage);
        return;
      }

      // 用最终 JSON 覆盖 sentinel 累积的结果：
      // - sentinel 提供"流式出现"的体验
      // - 最终 JSON 是唯一可信来源（字段完整、顺序稳定）
      characters = result.characters;
      eventSummary = result.eventSummary || '';
      console.log('人物画像生成成功，共', characters.length, '个人物，自动进入深度分析');

      // 再次检查：用户可能在人物画像完成后的瞬间点击了取消
      if (!isAnalyzing) {
        console.log('人物画像完成但用户已取消，不再进入深度分析');
        return;
      }

      // ---------- 第二步：自动进入深度分析（不停留在中间页）----------
      await startDeepAnalysis(materials);
    } catch (error) {
      // 如果用户已取消分析，不再跳转到 result 页面显示错误
      if (!isAnalyzing) {
        console.log('分析流程失败但用户已取消，忽略错误跳转');
        return;
      }
      errorMessage = String(error);
      console.error('分析流程失败:', error);
      currentPage = 'result';
      isAnalyzing = false;
    }
  }

  /**
   * 第二步：深度分析（Phase 2-A：由 generateCharacterProfiles 直接续调）
   *
   * 不再切换 currentPage —— 整个流程共用同一个 analyzing 页面，
   * 用户看到的只是顶部进度条从"抽取人物"滑到"推演真相"。
   *
   * @param materials 上一步已组装的材料数组，避免重复构建
   */
  async function startDeepAnalysis(materials: Material[]): Promise<void> {
    // 清空上次的分析正文（保留 characters / eventSummary）
    analysisResult = '';
    errorMessage = '';

    console.log('开始深度分析，人物数量:', characters.length);

    // 生成会话 ID
    currentSessionId = `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

    try {
      // 构建所有动机数据：人物名称 -> 动机内容列表
      // 大模型会自行分析并判断最可能的动机组合（Phase 2-A 不让用户手选）
      const allMotivations: Record<string, string[]> = {};
      for (const character of characters) {
        allMotivations[character.name] = character.motivations.map(m => m.content);
      }
      console.log('传入所有动机:', allMotivations);

      // 调用深度分析 API（复用同一个 analyzing 页的流式 UI）
      // Phase 1 起：后端返回结构化的 TruthAnalysisResult { extras, longform }
      const channel = createAnalysisChannel();
      const result = await invoke<TruthAnalysisResult>('analyze_truth', {
        materials,
        config: analysisConfig,
        allMotivations: allMotivations,
        eventSummary,
        onEvent: channel,
      });

      // longform 是 Markdown 长文（sentinel 已被后端剥离）
      analysisResult = result.longform;
      // extras 可能为 null（模型没吐合法 JSON 时）
      truthExtras = result.extras ?? null;

      // 检查用户是否已取消分析——取消后不再跳转结果页
      if (!isAnalyzing) {
        console.log('深度分析完成但用户已取消，不再跳转结果页');
        return;
      }

      currentPage = 'result';
      console.log('深度分析完成，吃瓜包:', truthExtras ? 'ok' : 'missing');

      // 保存历史记录
      await saveHistory();
    } catch (error) {
      // 如果用户已取消分析，不再跳转到 result 页面显示错误
      if (!isAnalyzing) {
        console.log('深度分析失败但用户已取消，忽略错误跳转');
        return;
      }
      errorMessage = String(error);
      console.error('深度分析失败:', error);
      currentPage = 'result';
    } finally {
      isAnalyzing = false;
    }
  }

  /**
   * 返回首页（全新开始）
   * 清空所有数据，让用户从零开始
   *
   * 关键：同时设置 isAnalyzing = false，阻止后台仍在运行的 invoke() Promise
   * resolve 后意外触发 currentPage = 'result' 跳转。
   * 这就是"取消分析"按钮的核心逻辑——前端状态重置 + 阻止后续回调执行。
   */
  function goBackToHome(): void {
    isAnalyzing = false; // 阻止后台 Promise resolve 后的页面跳转
    currentPage = 'home';
    inputItems = [];
    characters = [];
    eventSummary = '';
    analysisResult = '';
    truthExtras = null; // Phase 1：清空吃瓜包，避免下次首页残留
    errorMessage = '';
  }

  /**
   * 重新分析（清空所有数据，让用户从零开始重新输入材料）
   * 每次开始分析前必须清空旧数据，保证只传递当前输入的内容
   */
  function restartAnalysis(): void {
    isAnalyzing = false; // 与 goBackToHome 一致，阻止后台 Promise 回调
    inputItems = [];     // 清空旧输入项，确保重新分析时不会残留旧材料
    characters = [];
    eventSummary = '';
    analysisResult = '';
    truthExtras = null;
    errorMessage = '';
    currentPage = 'home';
  }

  /**
   * Phase 2-A：取消分析统一返回首页（原 cancelToMotivation 依赖的中间页已删除）
   * 保留函数名仅为减小 diff，实际语义等同于 goBackToHome
   */
  function cancelToMotivation(): void {
    goBackToHome();
  }

  /**
   * 保存历史记录
   */
  async function saveHistory(): Promise<void> {
    try {
      const session: AnalysisSession = {
        sessionId: currentSessionId,
        materials: inputItems.map((item) => ({
          type: item.type,
          content: item.type === 'file' && item.base64Data ? item.base64Data : item.content,
          file_name: item.fileName,
        })),
        characters: characters.length > 0 ? characters : undefined,
        // selectedMotivations 字段保留为空（向后兼容）
        selectedMotivations: {},
        analysisResult: analysisResult,
        // Phase 1 新增：把吃瓜包一起存，方便"查看历史详情"能完整还原
        truthExtras: truthExtras,
        createdAt: Date.now(),
        config: analysisConfig,
      };

      await invoke('save_history', { session });
      console.log('历史记录已保存');
    } catch (error) {
      console.error('保存历史记录失败:', error);
    }
  }

  // ========== 历史记录功能 ==========

  /** 历史记录列表 */
  let historyList: AnalysisSession[] = $state([]);

  /** 是否显示历史记录模态框 */
  let showHistoryModal = $state(false);

  /**
   * 打开历史记录页面
   *
   * 方案：在当前窗口内跳转到历史记录页面（全屏显示）
   * 原因：Tauri 多窗口需要额外配置多页面路由，当前方案更简单可靠
   */
  async function openHistoryWindow(): Promise<void> {
    // 加载历史记录数据
    await loadHistory();
    // 跳转到历史记录页面
    currentPage = 'history';
    console.log('已跳转到历史记录页面');
  }

  /**
   * 加载历史记录列表（用于首页模态框显示）
   */
  async function loadHistory(): Promise<void> {
    try {
      historyList = await invoke<AnalysisSession[]>('get_history');
      console.log('加载历史记录，共', historyList.length, '条');
    } catch (error) {
      console.error('加载历史记录失败:', error);
    }
  }

  /**
   * 打开历史记录模态框（首页用）
   */
  function openHistoryModal(): void {
    loadHistory();
    showHistoryModal = true;
  }

  /**
   * 关闭历史记录模态框
   */
  function closeHistoryModal(): void {
    showHistoryModal = false;
  }

  /**
   * 查看历史记录详情
   */
  function viewHistoryItem(session: AnalysisSession): void {
    // 恢复历史记录的状态
    inputItems = session.materials.map((m, i) => ({
      id: `history_${i}`,
      type: m.type as InputItemType,
      content: m.content,
      timestamp: session.createdAt,
    }));
    analysisConfig = session.config;
    characters = session.characters || [];
    analysisResult = session.analysisResult;
    // Phase 1：老记录没有 truthExtras 字段 → 回退为 null，结果页自动展示长文
    truthExtras = session.truthExtras ?? null;
    currentSessionId = session.sessionId;

    // 标记来源为"历史"，这样详情页的返回按钮会指向历史页而非首页
    resultEntrySource = 'history';

    // 直接跳转到结果页
    currentPage = 'result';
    showHistoryModal = false;
  }

  /**
   * 详情页返回按钮的统一入口
   *
   * 根据 resultEntrySource 决定返回目的地：
   *   - 'home'    → 清空数据回到首页
   *   - 'history' → 返回历史页（保留 historyList，免得重新加载）
   *
   * 为什么不复用 goBackToHome？
   *   goBackToHome 会重置 inputItems / characters 等首页上下文字段；
   *   从历史页来看详情的用户可能只想回去继续浏览其他条目，不应该被清空。
   */
  async function goBackFromResult(): Promise<void> {
    if (resultEntrySource === 'history') {
      // 保证历史列表是最新的（用户可能在其他地方删过记录）
      await loadHistory();
      currentPage = 'history';
      return;
    }
    // 默认行为：回首页，重置所有上下文
    goBackToHome();
  }

  /**
   * 删除历史记录项
   */
  async function deleteHistoryItem(sessionId: string): Promise<void> {
    try {
      await invoke('delete_history_item', { sessionId });
      await loadHistory();
      console.log('已删除历史记录:', sessionId);
    } catch (error) {
      console.error('删除历史记录失败:', error);
    }
  }

  // ========== HTML 安全净化 ==========
  // Phase 2-A：原 sanitizeHtml 被更通用的 renderContent 取代
  // （renderContent = markdown/HTML 自动识别 + DOMPurify 净化）

  // ========== 导出功能 ==========

  /** 导出状态 */
  let isExporting = $state(false);
  let exportError = $state('');

  /**
   * 导出为 HTML 文件
   *
   * 流程：
   * 1. 弹出 Tauri 原生保存对话框，让用户选择保存路径
   * 2. 构建 HTML 内容并写入用户指定位置
   */
  async function exportToHtml(): Promise<void> {
    isExporting = true;
    exportError = '';

    try {
      // 弹出原生"另存为"对话框，让用户选择保存路径和文件名
      const timestamp = new Date().toISOString().slice(0, 10);
      const filePath = await save({
        defaultPath: `真相分析报告-${timestamp}.html`,
        filters: [
          {
            name: 'HTML 文件',
            extensions: ['html']
          }
        ]
      });

      // 用户取消了保存对话框，不做任何操作
      if (!filePath) {
        isExporting = false;
        return;
      }

      // 构建完整的 HTML 内容
      const htmlContent = buildExportHtml();

      // 将 HTML 内容写入用户指定的文件路径
      await writeTextFile(filePath, htmlContent);

      // 导出成功，弹出 Toast 提示
      showToast('HTML 导出成功', 'success');
    } catch (error) {
      exportError = `导出失败: ${String(error)}`;
      showToast(`导出失败: ${String(error)}`, 'error');
    } finally {
      isExporting = false;
    }
  }

  /**
   * 复制分析结果到剪贴板
   *
   * 复制成功后弹出 Toast 提示用户，
   * 复制失败时弹出错误 Toast
   */
  async function copyResult(): Promise<void> {
    try {
      await navigator.clipboard.writeText(analysisResult);
      showToast('已复制到剪贴板', 'success');
    } catch (error) {
      showToast('复制失败，请手动复制', 'error');
    }
  }

  /**
   * 构建导出的 HTML 内容
   *
   * 包含：
   * - 人物画像卡片区域
   * - 分析结果内容
   * - 样式定义
   */
  function buildExportHtml(): string {
    // 人物画像卡片 HTML
    const characterCardsHtml = characters.length > 0
      ? characters.map(c => `
        <div class="character-card-export">
          <div class="character-avatar-export">${c.name.charAt(0)}</div>
          <div class="character-info-export">
            <div class="character-name-export">${c.name}</div>
            <div class="character-role-export">${c.role}</div>
            <div class="character-motivation-export">
              <span class="label">可能动机：</span>
              <span class="content">${c.motivations.map(m => m.content).join('；') || '暂无'}</span>
            </div>
          </div>
        </div>
      `).join('')
      : '<div class="no-characters">暂无人物信息</div>';

    // 完整 HTML 文件
    return `
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>真相分析报告</title>
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
      max-width: 800px;
      margin: 0 auto;
      padding: 24px;
      background-color: #f5f5f5;
      color: #333;
    }
    h1 { text-align: center; color: #1a1a1a; margin-bottom: 24px; }
    h2 { color: #2a2a2a; border-bottom: 1px solid #ddd; padding-bottom: 8px; margin-top: 24px; }
    h3 { color: #3a3a3a; margin-top: 16px; }
    h4 { color: #4a4a4a; margin-top: 12px; }
    p { line-height: 1.6; margin: 8px 0; }
    ul, li { margin: 4px 0; }
    strong { color: #1a1a1a; }
    blockquote { background: #f9f9f9; padding: 12px; border-left: 4px solid #ddd; margin: 12px 0; }
    .credibility-score { background: #10b981; color: white; padding: 2px 6px; border-radius: 4px; font-size: 0.8em; }
    .source-tag { color: #666; font-size: 0.85em; }
    .conflict-point { background: #fff3cd; padding: 8px; border-radius: 4px; margin: 8px 0; }
    .key-suspicion { background: #f8d7da; padding: 8px; border-radius: 4px; margin: 8px 0; }

    /* 人物画像卡片样式 */
    .characters-section-export {
      margin-bottom: 32px;
      padding: 16px;
      background: white;
      border-radius: 12px;
      border: 1px solid #ddd;
    }
    .characters-title-export {
      font-size: 1.2rem;
      color: #1a1a1a;
      margin-bottom: 16px;
    }
    .character-cards-export {
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
    }
    .character-card-export {
      display: flex;
      gap: 12px;
      padding: 12px;
      background: #f9f9f9;
      border-radius: 8px;
      border: 1px solid #eee;
      min-width: 200px;
    }
    .character-avatar-export {
      width: 40px;
      height: 40px;
      border-radius: 50%;
      background: linear-gradient(135deg, #3b82f6, #93c5fd);
      display: flex;
      align-items: center;
      justify-content: center;
      color: white;
      font-weight: 600;
      font-size: 1.1rem;
    }
    .character-info-export { flex: 1; }
    .character-name-export { font-weight: 600; color: #1a1a1a; margin-bottom: 4px; }
    .character-role-export { font-size: 0.8rem; color: #666; margin-bottom: 8px; }
    .character-motivation-export { font-size: 0.85rem; }
    .character-motivation-export .label { color: #888; }
    .character-motivation-export .content { color: #3b82f6; font-weight: 500; }
    .no-characters { color: #888; text-align: center; padding: 16px; }

    /* 分析内容区域 */
    .analysis-content-export {
      background: white;
      padding: 24px;
      border-radius: 12px;
      border: 1px solid #ddd;
    }

    /* 配置摘要 */
    .config-summary-export {
      margin-top: 24px;
      padding: 12px;
      background: #f9f9f9;
      border-radius: 8px;
    }
    .config-summary-export h4 { margin: 0 0 8px 0; }
    .config-grid-export { display: flex; gap: 12px; flex-wrap: wrap; }
    .config-grid-export span {
      padding: 4px 8px;
      background: #eee;
      border-radius: 4px;
      font-size: 0.85rem;
    }
  </style>
</head>
<body>
  <h1>真相分析报告</h1>
  <p style="text-align: center; color: #888;">生成时间：${new Date().toLocaleString('zh-CN')}</p>

  <!-- 人物画像卡片区域 -->
  ${characters.length > 0 ? `
  <section class="characters-section-export">
    <div class="characters-title-export">人物画像与动机</div>
    <div class="character-cards-export">
      ${characterCardsHtml}
    </div>
  </section>
  ` : ''}

  <!-- 分析结果内容 -->
  <section class="analysis-content-export">
    ${renderContent(analysisResult)}
  </section>

  <!-- 配置摘要 -->
  <div class="config-summary-export">
    <h4>分析参数</h4>
    <div class="config-grid-export">
      <span>温度: ${analysisConfig.temperature}</span>
      <span>公正度: ${analysisConfig.fairness}%</span>
      <span>道德底线: ${analysisConfig.morality}%</span>
      <span>格式: ${analysisConfig.outputFormat}</span>
      <span>深度: ${analysisConfig.analysisDepth}</span>
    </div>
  </div>
</body>
</html>
    `;
  }
</script>

<main class="app-container">
  <!-- 顶部极简导航栏（含主题切换按钮和历史按钮，无分割线）
       sticky-topbar：粘在窗口顶部，滚动时依然可见。
       home 页以外的页面会整体隐藏 header（shouldHideHeader），各自有自己的 sticky 顶栏。 -->
  {#if !shouldHideHeader()}
    <header class="app-header sticky-topbar">
      <div class="header-right">
        <!-- 主题切换按钮：放在历史按钮左侧，仅首页（非 home 页 header 会整体隐藏）显示 -->
        <ThemeToggle />
        <button
          class="history-btn"
          onclick={openHistoryWindow}
          aria-label="查看历史记录"
          title="历史记录"
        >
          历史
        </button>
      </div>
    </header>
  {/if}

  <!-- 主内容区域 -->
  <div class="app-content" class:result-page-scroll={currentPage === 'result' || currentPage === 'history'}>
    <!-- ========== 首页：居中单列对话式布局 ========== -->
    {#if currentPage === 'home'}
      <div class="home-page">
        <!-- 输入框组件（内含模型选择按钮+发送按钮+文件上传+参数配置） -->
        <MaterialInputPanel
          onSubmit={handleSubmit}
          isAnalyzing={isAnalyzing}
          initialItems={inputItems}
          config={analysisConfig}
          onConfigChange={handleConfigChange}
        />
      </div>

    <!-- ========== 统一流式分析页（Phase 2-A）==========
         合并原 characters / motivation / analyzing 三页：
         - 顶部：4 段式进度条（读取 → 抽取 → 汇总 → 推演）
         - 左侧：人物卡片侧栏（sentinel 流式填充，抽取阶段开始出现）
         - 主区：markdown 流式渲染（推演阶段开始展示；抽取阶段显示进度文案）
         用户看到的是"一次连续体验"，不再有中间点击 -->
    {:else if currentPage === 'analyzing'}
      <div class="stream-page">
        <!-- 顶部进度条 + 阶段文案区：sticky-topbar 让它们在推演阶段长文滚动时依然可见
             - data-phase 用于子元素高亮（按 CSS 规则匹配）
             - 进度条和状态文案一起粘顶，保证用户始终知道当前在哪一阶段 -->
        <div class="stream-sticky-top sticky-topbar">
          <div class="phase-progress" data-phase={streamPhase}>
            <div class="phase-step"
              class:active={streamPhase === 'reading'}
              class:done={['extracting','preparing','reasoning'].includes(streamPhase)}>
              <span class="phase-dot"></span>
              <span class="phase-name">读取材料</span>
            </div>
            <div class="phase-line"></div>
            <div class="phase-step"
              class:active={streamPhase === 'extracting'}
              class:done={['preparing','reasoning'].includes(streamPhase)}>
              <span class="phase-dot"></span>
              <span class="phase-name">抽取人物</span>
            </div>
            <div class="phase-line"></div>
            <div class="phase-step"
              class:active={streamPhase === 'preparing'}
              class:done={streamPhase === 'reasoning'}>
              <span class="phase-dot"></span>
              <span class="phase-name">汇总动机</span>
            </div>
            <div class="phase-line"></div>
            <div class="phase-step" class:active={streamPhase === 'reasoning'}>
              <span class="phase-dot"></span>
              <span class="phase-name">推演真相</span>
            </div>
          </div>

          <!-- 阶段文案：和进度条一起粘顶，避免用户滚下去就看不到"当前在做什么" -->
          <div class="stream-status">
            <span class="stream-status-label">{streamPhaseLabel || '准备中...'}</span>
            {#if characters.length > 0}
              <span class="stream-status-meta">已抽取 {characters.length} 位人物</span>
            {/if}
          </div>
        </div>

        <!-- Phase 3：一句话锐评预告（推演阶段 JSON 里一出现就抢先展示） -->
        {#if streamingVerdict}
          <div class="stream-verdict-teaser" aria-live="polite">
            <span class="teaser-kicker">锐评抢先看</span>
            <p class="teaser-body">
              <span class="teaser-q" aria-hidden="true">「</span>{streamingVerdict}<span class="teaser-q" aria-hidden="true">」</span>
            </p>
          </div>
        {/if}

        <!-- 主体双栏：左侧人物卡 / 右侧 markdown 流式 -->
        <div class="stream-body">
          <!-- 左栏：人物卡片（为空时占位） -->
          <aside class="stream-sidebar">
            <h4 class="stream-sidebar-title">人物画像</h4>
            {#if characters.length === 0}
              <p class="stream-sidebar-empty">
                {streamPhase === 'reading' ? '正在读取材料...' : '等待 AI 抽取人物...'}
              </p>
            {:else}
              <div class="stream-character-list">
                {#each characters as character (character.id)}
                  <div class="stream-character-card">
                    <div class="stream-char-head">
                      <div class="mini-avatar">{character.name.charAt(0)}</div>
                      <div class="stream-char-meta">
                        <div class="stream-char-name">{character.name}</div>
                        <div class="stream-char-role">{character.role}</div>
                      </div>
                    </div>
                    {#if character.motivations?.length > 0}
                      <ul class="stream-char-motivations">
                        {#each character.motivations.slice(0, 3) as m}
                          <li>{m.content}</li>
                        {/each}
                      </ul>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </aside>

          <!-- 右栏：分析正文（markdown 流式） -->
          <section class="stream-main">
            {#if streamPhase === 'reasoning' && streamingRenderedHtml}
              <!-- 推演阶段：渲染 markdown，底部呼吸光标 -->
              <article class="stream-article result-html-content">
                {@html streamingRenderedHtml}
                <span class="typewriter-caret inline-caret"></span>
              </article>
            {:else if streamPhase === 'reasoning'}
              <!-- 推演刚开始还没收到字：显示占位 -->
              <div class="stream-placeholder">
                <span class="typewriter-caret"></span>
                <p>AI 正在组织真相分析的第一段文字...</p>
              </div>
            {:else}
              <!-- 人物抽取阶段：展示思考指示 + 字数计数 -->
              <div class="stream-placeholder">
                <div class="loading-dots">
                  <span class="dot dot-1"></span>
                  <span class="dot dot-2"></span>
                  <span class="dot dot-3"></span>
                </div>
                {#if streamingText.length > 0}
                  <p>AI 已生成 {streamingText.length} 个字符，正在识别核心人物...</p>
                {:else}
                  <p>AI 正在阅读材料，稍后会陆续把识别到的人物贴到左侧...</p>
                {/if}
              </div>
            {/if}
          </section>
        </div>

        <!-- 底部操作栏：sticky-bottombar 让"取消分析"永远停在窗口底部可见
             用户长时间推演时不用滚到底才能找到取消按钮 -->
        <div class="stream-footer sticky-bottombar">
          <button class="cancel-btn" onclick={goBackToHome}>取消分析</button>
        </div>
      </div>

    <!-- ========== 结果页面 (Phase 1 起使用 ResultView 容器) ========== -->
    {:else if currentPage === 'result'}
      {#if analysisResult}
        <!--
          吃瓜版结果页：ResultView 自己铺满宽度，不再套旧的 .result-page 800px 限宽。
          - Hero 一句话锐评（来自 truthExtras.oneLinerVerdict）
          - 时间线（truthExtras.timelineEvents）
          - 长文回退区（Markdown 长文，一直保留）
          - Phase 2 会在 ResultView 内部追加：关系网 / 阵营 / 金句 / 赢家输家
          旧历史记录没有 truthExtras 时，ResultView 会把长文默认展开，保持可读性。
        -->
        <ResultView
          extras={truthExtras}
          longformHtml={renderContent(analysisResult)}
          characters={characters}
          materialCount={inputItems.length}
          onRestart={restartAnalysis}
          onBackHome={goBackFromResult}
          onCopy={copyResult}
          onExport={exportToHtml}
          isExporting={isExporting}
          exportError={exportError}
          backLabel={resultEntrySource === 'history' ? '返回历史' : '返回首页'}
        />
      {:else if errorMessage}
        <!-- 失败分支仍然使用旧 .result-page 窄容器，保持原错误卡片排版 -->
        <div class="result-page">
          <div class="error-card">
            <div class="error-header">
              <h2>分析失败</h2>
            </div>

            <div class="error-content">
              <h3>错误信息</h3>
              <p class="error-text">{errorMessage}</p>
            </div>

            <div class="error-suggestions">
              <h4>可能的解决方案：</h4>
              <ul>
                <li>检查 API Key 是否正确配置（点击模型选择按钮 → 管理模型）</li>
                <li>确保添加了至少一条信息资料</li>
                <li>检查网络连接是否正常</li>
                <li>确认 API 服务是否可用</li>
                <li>如果材料中没有人物，请添加包含人物事件的材料</li>
              </ul>
            </div>

            <div class="result-actions">
              <button class="btn btn-primary" onclick={restartAnalysis}>
                返回重新尝试
              </button>
            </div>
          </div>
        </div>
      {/if}

    <!-- ========== 历史记录页面 ========== -->
    {:else if currentPage === 'history'}
      <div class="history-page">
        <!-- 顶栏：使用通用 sticky-topbar 工具类，固定在滚动容器顶部
             - 滚动祖先是 .app-content.result-page-scroll
             - 返回按钮用 BackButton 组件，保持全局一致 -->
        <header class="history-page-header sticky-topbar">
          <BackButton label="返回首页" onClick={goBackToHome} />
          <h2 class="history-page-title">历史记录</h2>
          <!-- 右侧占位（未来可放"全部删除/清空"之类的操作） -->
          <div class="history-header-spacer"></div>
        </header>

        <div class="history-page-content">
          {#if historyList.length > 0}
            <ul class="history-list-full">
              {#each historyList as session (session.sessionId)}
                <!--
                  整张卡片可点击（内部 div[role="button"]）:
                  - 把 click/keydown 放在 div 而非 li 上，li 不是可交互元素，
                    给它加 role="button" 会被 svelte-check 判为 a11y 违规
                  - 删除按钮放在 .history-item-main 外部 (sibling)，天然不在主点击区，
                    不用 stopPropagation 也不会误触跳转
                -->
                <li class="history-item-full">
                  <div
                    class="history-item-main"
                    role="button"
                    tabindex="0"
                    aria-label={`查看 ${new Date(session.createdAt).toLocaleString('zh-CN')} 的分析记录`}
                    onclick={() => viewHistoryItem(session)}
                    onkeydown={(e) => {
                      // 空格 / Enter 都算点击，符合原生 <button> 的无障碍行为
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        viewHistoryItem(session);
                      }
                    }}
                  >
                    <!-- 精华句：一句话概括事件，来自 extractEssence 的三级回退
                         左侧红色竖线增强"这是重点"的视觉指示 -->
                    <p class="history-item-essence">
                      {extractEssence(session)}
                    </p>

                    <!-- 元信息行：时间 + 材料数 + 人物数，灰色小字不抢戏 -->
                    <div class="history-item-meta">
                      <time class="history-item-time">
                        {new Date(session.createdAt).toLocaleString('zh-CN')}
                      </time>
                      <span class="history-item-dot" aria-hidden="true">·</span>
                      <span>{session.materials.length} 条材料</span>
                      <span class="history-item-dot" aria-hidden="true">·</span>
                      <span>{session.characters?.length || 0} 位人物</span>
                    </div>
                  </div>

                  <!-- 删除按钮：悬浮右上角，hover li 时才显示，避免静态列表视觉噪音
                       位于 main 区域外部（sibling），不与主点击事件冲突 -->
                  <button
                    type="button"
                    class="history-item-delete"
                    aria-label="删除这条记录"
                    title="删除"
                    onclick={() => deleteHistoryItem(session.sessionId)}
                  >
                    <!-- 内联 SVG 垃圾桶图标，跟随文字颜色 -->
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none"
                         stroke="currentColor" stroke-width="2"
                         stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                      <polyline points="3 6 5 6 21 6"></polyline>
                      <path d="M19 6l-2 14a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2L5 6"></path>
                      <path d="M10 11v6"></path>
                      <path d="M14 11v6"></path>
                    </svg>
                  </button>
                </li>
              {/each}
            </ul>
          {:else}
            <div class="no-history-full">
              <span class="no-history-icon"></span>
              <p>暂无历史记录</p>
              <p class="no-history-hint">开始一次分析后，记录将自动保存</p>
              <button class="btn btn-primary" onclick={goBackToHome}>
                开始分析
              </button>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  <!-- 历史记录模态框 -->
  {#if showHistoryModal}
    <div
      class="history-modal-overlay"
      onclick={closeHistoryModal}
      onkeydown={(e) => e.key === 'Escape' && closeHistoryModal()}
      role="button"
      tabindex="-1"
      aria-label="关闭历史记录"
    >
      <div class="history-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && closeHistoryModal()} role="dialog" aria-modal="true" tabindex="-1">
        <div class="history-modal-header">
          <h2>历史记录</h2>
          <button class="close-btn" onclick={closeHistoryModal}>✕</button>
        </div>

        <div class="history-modal-content">
          {#if historyList.length > 0}
            <ul class="history-list">
              {#each historyList as session (session.sessionId)}
                <li class="history-item">
                  <div class="history-item-info">
                    <span class="history-time">
                      {new Date(session.createdAt).toLocaleString('zh-CN')}
                    </span>
                    <span class="history-materials">
                      {session.materials.length} 条材料
                    </span>
                  </div>
                  <div class="history-item-actions">
                    <button
                      class="btn-small"
                      onclick={() => viewHistoryItem(session)}
                    >
                      查看
                    </button>
                    <button
                      class="btn-small btn-delete"
                      onclick={() => deleteHistoryItem(session.sessionId)}
                    >
                      删除
                    </button>
                  </div>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="no-history">暂无历史记录</p>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</main>

<!-- 全局 Toast 提示组件，固定在窗口右下角 -->
{#if toastVisible}
  <div class="toast toast-{toastType}">
    {toastMsg}
  </div>
{/if}

<style>
  /* ========== 全局布局样式 ========== */

  .app-container {
    width: 100%;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background-color: var(--bg-primary);
  }

  .app-header {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    padding: 10px 20px;
    /* 背景和磨砂由 .sticky-topbar 工具类提供，这里只留布局相关 */
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .history-btn {
    padding: 6px 12px;
    border-radius: 4px;
    background-color: transparent;
    color: var(--text-secondary);
    border: none;
    font-size: 0.78rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .history-btn:hover {
    color: var(--text-primary);
    background-color: var(--bg-card);
  }

  .app-content {
    flex: 1;
    padding: 20px;
    overflow: hidden;
  }

  .app-content.result-page-scroll {
    overflow-y: auto;
    /* 结果页 / 历史页的顶部由内部 sticky 顶栏接管。
       外层 padding-top 会导致 sticky 顶栏"悬浮在 padding 内侧"，
       看起来就像顶部多出一条空白带。去掉它让顶栏真正贴屏幕顶。
       左右 / 底部 padding 保留，维持内容区域的呼吸感。 */
    padding-top: 0;
  }

  /* ========== 首页居中单列布局 ========== */

  .home-page {
    /* 输入框自带宽度居中，首页只需垂直居中 */
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    padding: 20px 0;
    gap: 0;
  }

  /* ========== 分析中页面样式 ========== */

  .analyzing-page {
    max-width: 600px;
    margin: 0 auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 400px;
  }

  .analyzing-animation {
    margin-bottom: 32px;
  }

  .loading-dots {
    display: flex;
    gap: 12px;
    justify-content: center;
  }

  .dot {
    width: 10px;
    height: 10px;
    background-color: var(--accent);
    border-radius: 50%;
    animation: bounce 1.4s ease-in-out infinite;
  }

  .dot-1 { animation-delay: 0s; }
  .dot-2 { animation-delay: 0.2s; }
  .dot-3 { animation-delay: 0.4s; }

  @keyframes bounce {
    0%, 80%, 100% { transform: scale(0); }
    40% { transform: scale(1); }
  }

  .analyzing-info h2 {
    font-size: 1.1rem;
    color: var(--text-primary);
    margin-bottom: 10px;
    letter-spacing: -0.02em;
  }

  .analyzing-count {
    color: var(--accent);
    font-size: 0.85rem;
    margin-bottom: 6px;
  }

  .analyzing-hint {
    color: var(--text-secondary);
    font-size: 0.78rem;
    margin-bottom: 20px;
  }

  .cancel-btn {
    padding: 8px 16px;
    border-radius: var(--radius-sm);
    background-color: transparent;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    font-size: 0.78rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .cancel-btn:hover {
    border-color: var(--text-muted);
    color: var(--text-primary);
  }

  /* ========== 流式阶段进度条（Phase 1 新增） ========== */
  /* 设计目的：让用户在等待期间看到"读取 → 抽取/推演"两段式进度，
     而不是一个单调的 loading 点。被点亮的步骤用 accent 色高亮。 */
  .phase-progress {
    display: flex;
    align-items: center;
    gap: 10px;
    /* margin-bottom 去掉，交给外层 .stream-sticky-top 的 gap 管 */
    padding: 8px 14px;
    background: var(--bg-card);
    border-radius: 999px;
    border: 1px solid var(--border);
  }

  .phase-step {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
    font-size: 0.72rem;
    transition: color 0.2s;
  }

  .phase-step.active {
    color: var(--accent);
  }

  .phase-step.done {
    color: var(--text-secondary);
  }

  .phase-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: currentColor;
    flex-shrink: 0;
  }

  /* active 态的点呼吸：告诉用户这一步正在进行 */
  .phase-step.active .phase-dot {
    animation: phase-pulse 1.2s ease-in-out infinite;
  }

  @keyframes phase-pulse {
    0%, 100% { opacity: 0.4; transform: scale(1); }
    50%      { opacity: 1;   transform: scale(1.35); }
  }

  .phase-line {
    width: 36px;
    height: 1px;
    background: var(--border);
  }

  .phase-name {
    white-space: nowrap;
  }

  /* 深度分析页需要宽一些以容纳打字机预览 */
  .analyzing-page-wide {
    max-width: 760px;
    width: 100%;
  }

  /* ========== 打字机预览（深度分析流式文本） ========== */
  /* 只展示尾部 ~1200 字符，配合底部呼吸光标表示 AI 还在输出 */
  .typewriter-preview {
    width: 100%;
    margin-top: 24px;
    padding: 16px 18px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 6px;
    max-height: 240px;
    overflow: hidden;
    /* 底部淡出：让"流水"的视觉感觉更自然 */
    mask-image: linear-gradient(to top, black 60%, transparent 100%);
    -webkit-mask-image: linear-gradient(to top, black 60%, transparent 100%);
  }

  .typewriter-text {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: 0.72rem;
    line-height: 1.55;
    color: var(--text-secondary);
    white-space: pre-wrap;
    word-break: break-word;
    text-align: left;
  }

  .typewriter-caret {
    display: inline-block;
    width: 6px;
    height: 0.9em;
    margin-left: 2px;
    background: var(--accent);
    vertical-align: text-bottom;
    animation: caret-blink 0.9s steps(2) infinite;
  }

  @keyframes caret-blink {
    0%, 49%   { opacity: 1; }
    50%, 100% { opacity: 0; }
  }

  /* ========== 统一流式分析页（Phase 2-A） ========== */
  /* 整体布局：顶部进度条 + 阶段文案 + 主体双栏 + 底部取消按钮。
     与旧的 analyzing-page 共用 .phase-progress / .loading-dots / .typewriter-caret，
     此处只补充它新增的容器、侧栏、主区域样式。 */
  .stream-page {
    max-width: 1100px;
    margin: 0 auto;
    padding: 0 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: calc(100vh - 40px);
    box-sizing: border-box;
  }

  /* Sticky 顶部容器：裹住进度条 + 阶段文案
     - 应用 sticky-topbar 工具类后 position: sticky; top: 0
     - 这里补充内边距和堆叠布局 */
  .stream-sticky-top {
    padding: 16px 0 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* 原 .phase-progress 样式由下方 "流式阶段进度条" 区块继承，
     但它之前带 margin-bottom: 24px，在 sticky 容器里需要去掉以避免双倍间距 */

  .stream-status {
    display: flex;
    align-items: baseline;
    gap: 16px;
    padding: 0 4px;
  }

  .stream-status-label {
    color: var(--text-primary);
    font-size: 0.9rem;
    letter-spacing: -0.01em;
  }

  .stream-status-meta {
    color: var(--text-muted);
    font-size: 0.72rem;
  }

  /* Phase 3：一句话锐评预告 — 流式出现后高亮显示 */
  .stream-verdict-teaser {
    margin: 12px 4px 0;
    padding: 14px 18px;
    border-radius: 8px;
    background:
      linear-gradient(135deg, rgba(230, 83, 79, 0.08), transparent 60%),
      var(--bg-card);
    border: 1px solid rgba(230, 83, 79, 0.3);
    animation: teaser-pop 320ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes teaser-pop {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .teaser-kicker {
    display: inline-block;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.62rem;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    color: var(--accent-hot);
    margin-bottom: 6px;
  }

  .teaser-body {
    margin: 0;
    font-family: 'Source Serif Pro', 'Noto Serif SC', Georgia, serif;
    font-size: 1.05rem;
    line-height: 1.4;
    color: var(--text-primary);
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .teaser-q {
    color: var(--accent-hot);
    font-weight: 700;
    opacity: 0.7;
    margin: 0 0.05em;
  }

  @media (prefers-reduced-motion: reduce) {
    .stream-verdict-teaser {
      animation: none;
    }
  }

  .stream-body {
    flex: 1;
    display: grid;
    /* 左侧固定宽度 260，右侧自适应。太窄时单列堆叠（见 @media） */
    grid-template-columns: 260px 1fr;
    gap: 16px;
    min-height: 0; /* 允许子元素 overflow 内部滚动 */
  }

  .stream-sidebar {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 14px;
    overflow-y: auto;
    min-height: 0;
  }

  .stream-sidebar-title {
    margin: 0 0 12px 0;
    font-size: 0.78rem;
    color: var(--text-primary);
    letter-spacing: 0.01em;
  }

  .stream-sidebar-empty {
    color: var(--text-muted);
    font-size: 0.75rem;
    padding: 20px 6px;
    text-align: center;
  }

  .stream-character-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* 人物卡在出现瞬间有淡入动画，强化"逐张浮现"的感觉 */
  .stream-character-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px;
    animation: card-fade-in 0.3s ease-out;
  }

  @keyframes card-fade-in {
    from { opacity: 0; transform: translateY(6px); }
    to   { opacity: 1; transform: translateY(0);   }
  }

  .stream-char-head {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .stream-char-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .stream-char-name {
    font-size: 0.82rem;
    color: var(--text-primary);
    font-weight: 600;
  }

  .stream-char-role {
    font-size: 0.68rem;
    color: var(--text-muted);
  }

  .stream-char-motivations {
    list-style: disc;
    padding-left: 18px;
    margin: 8px 0 0 0;
    color: var(--text-secondary);
    font-size: 0.72rem;
    line-height: 1.5;
  }

  .stream-char-motivations li + li {
    margin-top: 3px;
  }

  /* 主区域：markdown 流式容器 */
  .stream-main {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 20px 24px;
    overflow-y: auto;
    min-height: 0;
    /* 顶部渐隐：让滚到顶的标题有层次感 */
    position: relative;
  }

  .stream-article {
    /* 复用结果页的 .result-html-content 排版，统一视觉 */
    font-size: 0.86rem;
  }

  /* 行内光标，用于 markdown 末尾跟随 */
  .inline-caret {
    display: inline-block;
    margin-left: 2px;
    vertical-align: baseline;
  }

  .stream-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 16px;
    color: var(--text-muted);
    font-size: 0.8rem;
    text-align: center;
    padding: 0 24px;
  }

  .stream-footer {
    display: flex;
    justify-content: center;
    /* sticky-bottombar 已提供背景 + 磨砂 + 上边线，这里只调内边距 */
    padding: 12px 0;
  }

  /* 窄屏堆叠：人物卡跑到主内容上方 */
  @media (max-width: 760px) {
    .stream-body {
      grid-template-columns: 1fr;
    }
    .stream-sidebar {
      max-height: 220px;
    }
  }

  /* ========== 动机选择页面样式（全屏利用） ========== */

  .motivation-page {
    max-width: 100%; /* 全宽度利用 */
    margin: 0;
    padding: 8px; /* 最小边距 */
    height: calc(100vh - 60px); /* 尽可能填满 */
  }

  /* ========== 结果页面样式 ========== */

  .result-page {
    max-width: 800px;
    margin: 0 auto;
  }

  .success-card,
  .error-card {
    background-color: var(--bg-secondary);
    border-radius: 0;
    padding: 20px;
    border: none;
    border-top: 1px solid var(--border);
    box-shadow: none;
  }

  .success-card {
    border-color: var(--border);
  }

  .error-card {
    border-color: var(--error);
  }

  .result-header,
  .error-header {
    margin-bottom: 16px;
    text-align: center;
  }

  .result-header h2,
  .error-header h2 {
    font-size: 1.1rem;
    color: var(--text-primary);
    margin: 0 0 6px 0;
    letter-spacing: -0.02em;
  }

  .analyzed-info {
    color: var(--text-secondary);
    font-size: 0.75rem;
    margin: 0;
  }

  /* ========== 人物画像卡片区域（顶部） ========== */
  .characters-summary {
    background-color: var(--bg-card);
    border-radius: 4px;
    padding: 14px;
    margin-bottom: 16px;
  }

  .characters-title {
    font-size: 0.78rem;
    color: var(--text-primary);
    margin: 0 0 10px 0;
    font-weight: 600;
  }

  .character-cards-inline {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .character-card-mini {
    display: flex;
    gap: 6px;
    padding: 6px 10px;
    background: var(--bg-secondary);
    border-radius: 4px;
    border: 1px solid var(--border);
  }

  .mini-avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: linear-gradient(135deg, #333, #888);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    font-weight: 600;
    font-size: 0.75rem;
  }

  .mini-info {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .mini-name {
    font-size: 0.78rem;
    color: var(--text-primary);
    font-weight: 500;
  }

  .mini-role {
    font-size: 0.65rem;
    color: var(--text-muted);
  }

  .mini-motivation {
    font-size: 0.65rem;
    color: var(--text-secondary);
  }

  .result-content,
  .error-content {
    background-color: var(--bg-card);
    border-radius: 4px;
    padding: 16px;
    margin-bottom: 16px;
  }

  .result-content h3,
  .error-content h3 {
    font-size: 0.85rem;
    color: var(--text-primary);
    margin: 0 0 10px 0;
    font-weight: 600;
  }

  .error-text {
    color: var(--error);
    line-height: 1.5;
    margin: 0;
    font-weight: 500;
    font-size: 0.8rem;
  }

  .config-summary {
    background-color: var(--bg-card);
    border-radius: 4px;
    padding: 14px;
    margin-bottom: 16px;
  }

  .config-summary h4 {
    font-size: 0.78rem;
    color: var(--text-primary);
    margin: 0 0 10px 0;
  }

  .config-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
    gap: 6px;
    font-size: 0.68rem;
    color: var(--text-secondary);
  }

  .config-grid span {
    padding: 3px 6px;
    background: var(--bg-secondary);
    border-radius: 3px;
  }

  .error-suggestions {
    background-color: var(--bg-secondary);
    border-radius: 4px;
    padding: 14px;
    margin-bottom: 16px;
    border: 1px solid var(--error);
  }

  .error-suggestions h4 {
    font-size: 0.78rem;
    color: var(--text-primary);
    margin: 0 0 6px 0;
  }

  .error-suggestions ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .error-suggestions li {
    color: var(--text-secondary);
    padding: 3px 0;
    position: relative;
    padding-left: 14px;
    font-size: 0.78rem;
  }

  .error-suggestions li::before {
    content: '→';
    position: absolute;
    left: 0;
    color: var(--text-muted);
  }

  .result-actions {
    display: flex;
    gap: 12px;
    justify-content: center;
  }

  .export-error {
    /* 用 color-mix 混合 error 色与背景，浅深两种主题下都能自适应 */
    background: color-mix(in srgb, var(--error) 12%, var(--bg-secondary));
    color: var(--error);
    padding: 6px;
    border-radius: 4px;
    margin-bottom: 10px;
    text-align: center;
    font-size: 0.75rem;
  }

  /* ========== 按钮样式 ========== */

  .btn {
    padding: 8px 18px;
    border-radius: 4px;
    font-size: 0.78rem;
    cursor: pointer;
    transition: all 0.15s;
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .btn-primary {
    background-color: var(--accent);
    /* 文字反色，暗黑模式下才可读 */
    color: var(--bg-primary);
    border: none;
  }

  .btn-primary:hover {
    background-color: var(--accent-hover);
    transform: translateY(-0.5px);
  }

  .btn-secondary {
    background-color: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
  }

  .btn-secondary:hover {
    border-color: var(--text-muted);
    background-color: var(--bg-card);
  }

  .btn-export {
    background-color: var(--bg-card);
    color: var(--text-primary);
    border: 1px solid var(--border);
  }

  .btn-export:hover {
    border-color: var(--text-muted);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }

  /* ========== 历史记录模态框样式 ========== */

  .history-modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.25);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    backdrop-filter: blur(4px);
  }

  .history-modal {
    background: var(--bg-secondary);
    border-radius: 8px;
    width: 560px;
    max-height: 80vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow-xl);
  }

  .history-modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 20px;
    border-bottom: 1px solid var(--border);
  }

  .history-modal-header h2 {
    font-size: 0.95rem;
    color: var(--text-primary);
    margin: 0;
  }

  .close-btn {
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
    font-size: 0.8rem;
    transition: all 0.15s;
  }

  .close-btn:hover {
    background: var(--error);
    color: #fff;
  }

  .history-modal-content {
    padding: 14px 20px;
    overflow-y: auto;
    flex: 1;
  }

  .history-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .history-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 0;
    background: transparent;
    border-radius: 0;
    margin-bottom: 0;
    border-bottom: 1px solid var(--border);
    transition: background 0.1s;
  }

  .history-item:hover {
    background: var(--bg-card);
  }

  .history-item-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .history-time {
    font-size: 0.78rem;
    color: var(--text-primary);
  }

  .history-materials {
    font-size: 0.68rem;
    color: var(--text-muted);
  }

  .history-item-actions {
    display: flex;
    gap: 8px;
  }

  .btn-small {
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 0.68rem;
    cursor: pointer;
    background: var(--accent);
    /* 文字色跟主色反色：浅色主题下 accent 是黑，文字白；暗色主题下 accent 是白，文字黑 */
    color: var(--bg-primary);
    border: none;
    transition: all 0.15s;
  }

  .btn-small:hover {
    background: var(--accent-hover);
  }

  .btn-delete {
    background: var(--bg-secondary);
    color: var(--error);
    border: 1px solid var(--border);
  }

  .btn-delete:hover {
    background: var(--error);
    color: #fff;
  }

  .no-history {
    text-align: center;
    color: var(--text-muted);
    padding: 28px;
    font-size: 0.8rem;
  }

  /* ========== 历史记录全屏页面样式（重构版） ========== */
  /* 设计原则：
     - 顶栏粘在可视区顶部（通过 .sticky-topbar 工具类），返回按钮永远可见
     - 每条记录是一张可点击卡片，整体作为点击热区
     - 卡片内容浓缩成"一句话精华 + 元信息行"，不再粗暴截 150 字
     - 删除按钮悬浮右上角，仅 hover/focus 时显现，平时不抢戏 */

  .history-page {
    max-width: 840px;
    margin: 0 auto;
    /* 去掉原来的整体 padding，让 sticky 顶栏能顶到容器边，避免出现顶栏上方的空白带 */
    padding: 0;
  }

  /* 顶栏：返回按钮 + 标题 + 右侧占位，三等分布局 */
  .history-page-header {
    display: flex;
    align-items: center;
    gap: 12px;
    /* 视觉水平内边距（左右留白），sticky-topbar 的磨砂背景占满整行 */
    padding: 14px clamp(12px, 3vw, 20px);
  }

  .history-page-title {
    font-size: 1.05rem;
    color: var(--text-primary);
    margin: 0;
    flex: 1;
    letter-spacing: -0.02em;
    font-weight: 600;
    /* 让标题不抢返回按钮的主次关系 */
    text-align: center;
  }

  /* 右侧占位：保持返回按钮和标题的水平平衡，宽度约等于返回按钮宽度 */
  .history-header-spacer {
    width: 92px;
    flex-shrink: 0;
  }

  .history-page-content {
    padding: 12px clamp(12px, 3vw, 20px) 24px;
    min-height: 360px;
  }

  .history-list-full {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* 列表项外壳（<li>）：纯卡片容器，负责背景、边框、阴影、hover 反馈
     真正的 click 监听在内部的 .history-item-main 上，删除按钮是它的 sibling */
  .history-item-full {
    position: relative;
    list-style: none;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    /* 动效只作用于视觉属性（transform / box-shadow / border-color / bg），
       不触发 layout，保证 hover 丝滑 */
    transition: transform 160ms cubic-bezier(0.16, 1, 0.3, 1),
                box-shadow 160ms ease,
                border-color 160ms ease,
                background-color 160ms ease;
  }

  /* hover 和内部 main 聚焦都触发卡片的上浮效果 */
  .history-item-full:hover,
  .history-item-full:has(.history-item-main:focus-visible) {
    transform: translateY(-1px);
    border-color: color-mix(in srgb, var(--text-muted) 40%, var(--border));
    box-shadow: var(--shadow-md);
    background: var(--bg-secondary);
  }

  /* 主点击区：整张卡片的 click / keyboard 入口
     padding 放在这里而非 li 上，让删除按钮能铺到 li 的边缘 */
  .history-item-main {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px 52px 14px 18px; /* 右侧留 52px 给删除按钮，不压内容 */
    cursor: pointer;
    border-radius: var(--radius-md);
    outline: none;
  }

  .history-item-main:focus-visible {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-hot) 50%, transparent) inset;
  }

  /* 精华句：核心内容，左侧红色竖线强化视觉重点
     字号略大（0.9rem）确保在整张卡片里是第一焦点 */
  .history-item-essence {
    margin: 0;
    padding-left: 12px;
    border-left: 3px solid var(--accent-hot);
    font-size: 0.9rem;
    line-height: 1.55;
    color: var(--text-primary);
    font-weight: 500;
    letter-spacing: -0.005em;
    /* 最多显示两行，超出省略号，避免某些超长兜底段落撑破卡片 */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    line-clamp: 2;
    overflow: hidden;
  }

  /* 元信息行：时间 + 材料数 + 人物数，全灰小字，用点分隔 */
  .history-item-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    font-size: 0.7rem;
    color: var(--text-muted);
    padding-left: 15px; /* 与精华句左红线对齐 */
  }

  .history-item-time {
    color: var(--text-secondary);
  }

  .history-item-dot {
    color: var(--text-muted);
    opacity: 0.6;
  }

  /* 删除按钮：悬浮在卡片右上角，平时半透明、hover 卡片或自己聚焦时完全显现
     这样静态列表视觉很干净，但删除入口永远存在 */
  .history-item-delete {
    position: absolute;
    top: 10px;
    right: 10px;
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    color: var(--text-muted);
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 160ms ease,
                color 160ms ease,
                background-color 160ms ease,
                border-color 160ms ease;
  }

  /* 父卡片 hover / focus 时，删除按钮淡入 */
  .history-item-full:hover .history-item-delete,
  .history-item-full:focus-visible .history-item-delete,
  .history-item-delete:focus-visible {
    opacity: 1;
  }

  .history-item-delete:hover,
  .history-item-delete:focus-visible {
    background: color-mix(in srgb, var(--error) 14%, transparent);
    color: var(--error);
    border-color: color-mix(in srgb, var(--error) 30%, transparent);
    outline: none;
  }

  /* 空状态卡片（保持原来的美学） */
  .no-history-full {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 40px;
    text-align: center;
    background: var(--bg-card);
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
  }

  .no-history-icon {
    font-size: 2rem;
    margin-bottom: 12px;
  }

  .no-history-full p {
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin: 0 0 6px 0;
  }

  .no-history-hint {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin-bottom: 20px;
  }
</style>