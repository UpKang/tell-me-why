/**
 * 吃瓜结构化结果类型定义
 *
 * 这些类型必须与 src-tauri/src/lib.rs 里的 TruthExtras / TruthAnalysisResult
 * 保持形状一致（serde rename_all = "camelCase"）。字段语义见 Rust 端注释。
 *
 * Phase 1 阶段：后端会尝试让大模型同时输出"吃瓜包 JSON + Markdown 长文"，
 * 解析成功则 extras 有值，失败则 extras = null（仅退化到只渲染 longform）。
 */

/** 金句卡：事件中某人说的"爆点话" */
export interface HotQuote {
  /** 发言人：当事人、官方、媒体、网友代表等 */
  speaker: string;
  /** 金句原文 */
  quote: string;
  /** 一句话背景（在什么场合、针对谁说的） */
  context: string;
}

/** 阵营划分：把人物按主张/立场分堆 */
export interface Faction {
  /** 阵营名，如"挺 A 派" */
  name: string;
  /** 核心立场（一句话） */
  stance: string;
  /** 代表人物姓名（需与 CharacterProfile.name 一致） */
  members: string[];
  /** 阵营主张/口号 */
  keyClaim: string;
}

/** 时间线节点：一次关键事件 */
export interface TimelineEvent {
  /** 时间描述：日期 / "事发当晚" / "初期" 等 */
  time: string;
  /** ≤ 20 字短标题 */
  title: string;
  /** 1-3 句话详细描述 */
  desc: string;
  /** 是否是反转点（前端会高亮） */
  isTwist: boolean;
  /** 是否是爆点/高潮 */
  isHot: boolean;
}

/** 反转点说明 */
export interface PlotTwist {
  /** 对应的 TimelineEvent.title */
  eventRef: string;
  /** 为什么这是反转 */
  whyTwist: string;
}

/** 赢家/输家榜单条目 */
export interface WinLoseItem {
  /** 人物名（需与 CharacterProfile.name 一致） */
  name: string;
  /** 一句话理由 */
  reason: string;
}

/** 赢家输家榜 */
export interface WinnersLosers {
  winners: WinLoseItem[];
  losers: WinLoseItem[];
}

/** 关系边的情感倾向：正向/负向/中性 */
export type RelationSentiment = 'positive' | 'negative' | 'neutral';

/** 人物关系图的一条边 */
export interface RelationEdge {
  /** 起点人物名 */
  from: string;
  /** 终点人物名 */
  to: string;
  /** 关系标签：前同事 / 情侣 / 对手 等 */
  label: string;
  /** 情感倾向 */
  sentiment: RelationSentiment;
}

/** 完整吃瓜包（后端 TruthExtras） */
export interface TruthExtras {
  /** ≤ 30 字锐评（Hero 大标题） */
  oneLinerVerdict: string;
  hotQuotes: HotQuote[];
  factions: Faction[];
  timelineEvents: TimelineEvent[];
  plotTwists: PlotTwist[];
  winnersLosers: WinnersLosers;
  characterRelations: RelationEdge[];
}

/**
 * analyze_truth 的新返回类型。
 * extras 可能为 null（大模型没吐 JSON / JSON 解析失败时）
 */
export interface TruthAnalysisResult {
  extras?: TruthExtras | null;
  longform: string;
}

/**
 * 构造一个全空的 TruthExtras 占位符。
 * 用于：旧历史记录没存 extras 时兜底，或首次进入结果页 extras 未就绪时。
 */
export function emptyExtras(): TruthExtras {
  return {
    oneLinerVerdict: '',
    hotQuotes: [],
    factions: [],
    timelineEvents: [],
    plotTwists: [],
    winnersLosers: { winners: [], losers: [] },
    characterRelations: [],
  };
}

/**
 * 判断 extras 是否"有料"（至少一个核心模块有内容）
 * 用来决定是否展示吃瓜版 UI，还是退化到只展示 Markdown 长文。
 */
export function hasRichExtras(extras: TruthExtras | null | undefined): extras is TruthExtras {
  if (!extras) return false;
  if (extras.oneLinerVerdict && extras.oneLinerVerdict.trim()) return true;
  if (extras.timelineEvents && extras.timelineEvents.length > 0) return true;
  if (extras.hotQuotes && extras.hotQuotes.length > 0) return true;
  if (extras.factions && extras.factions.length > 0) return true;
  return false;
}
