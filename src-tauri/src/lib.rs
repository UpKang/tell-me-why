//! TellMeWhy 应用主入口 - 真相推测分析平台
//!
//! 该模块负责初始化 Tauri 应用，包括：
//! - 系统托盘功能
//! - API Key 存储（使用 tauri-plugin-store）
//! - 配置持久化存储
//! - 真相分析功能（调用大模型，支持多材料和多参数配置）

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    ipc::Channel,
    Emitter, Manager,
};
use tauri_plugin_store::StoreExt;
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

// ============================================================================
// 流式事件定义（Phase 1 新增）
// ----------------------------------------------------------------------------
// 通过 Tauri Channel 从 Rust 推送到前端的分析过程事件。
// 设计目的：让用户在等待期间看到阶段切换 + 实时 token，而不是一个黑盒。
//
// 事件类型（前端按 type 字段分发）：
// - phase: 进入某个阶段（读取材料 / 抽取人物 / 推理 / 完成），含人类可读 label
// - delta: LLM 流式吐出的增量文本
// - done:  整个流程结束，携带完整拼接文本（前端再按需解析 JSON / 渲染 HTML）
// - error: 过程中任意一步失败
// ============================================================================
#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum StreamEvent {
    /// 进入新阶段
    Phase {
        /// 阶段标识（供前端做进度条高亮）
        phase: String,
        /// 展示给用户的文案
        label: String,
    },
    /// 大模型吐出的增量文本
    Delta {
        /// 本次增量内容
        text: String,
    },
    /// 流程结束
    Done {
        /// 完整文本（拼接后的全部内容）
        full_text: String,
    },
    /// 流程失败
    Error {
        /// 错误消息
        message: String,
    },
}

/// 统一的"发事件"小工具：忽略 channel 已关闭的错误
/// 前端可能在分析中途切换页面关闭 channel，此时发送失败不应让后端崩溃
fn emit_event(ch: &Channel<StreamEvent>, evt: StreamEvent) {
    if let Err(e) = ch.send(evt) {
        log::warn!("流式事件发送失败（前端可能已断开）: {}", e);
    }
}

/// 存储文件名称
const STORE_FILE: &str = "settings.json";
/// API Key 存储键名
const API_KEY_KEY: &str = "api_key";
/// Base URL 存储键名
const BASE_URL_KEY: &str = "base_url";
/// Model 存储键名
const MODEL_KEY: &str = "model";
/// 多模态模型存储键名（用于处理图片等非文本材料）
const MULTIMODAL_MODEL_KEY: &str = "multimodal_model";
/// 模型配置列表存储键名（用于存储多个模型配置）
const MODEL_CONFIGS_KEY: &str = "model_configs";
/// 当前选中的模型配置 ID 存储键名
const CURRENT_MODEL_CONFIG_ID_KEY: &str = "current_model_config_id";
/// 默认 Base URL（DashScope OpenAI 兼容端点）
const DEFAULT_BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";
/// 默认模型
const DEFAULT_MODEL: &str = "glm-5.1";
/// 默认多模态模型（支持图片理解的模型）
const DEFAULT_MULTIMODAL_MODEL: &str = "qwen-vl-plus";

// ========== 数据结构定义 ==========

/// 模型配置结构（前端 ModelSelector 组件对应）
/// 一个配置项包含完整的 API 连接信息
/// serde 使用 camelCase 序列化，与前端 JS 保持一致
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    /// 配置唯一标识（用于区分不同配置）
    pub id: String,
    /// API 基础地址（如 https://dashscope.aliyuncs.com/compatible-mode/v1）
    pub base_url: String,
    /// API 密钥
    pub api_key: String,
    /// 模型名称（如 glm-5.1、qwen-plus）
    pub model: String,
    /// 是否支持多模态（图片输入）
    pub is_multimodal: bool,
    /// 配置显示名称（自动生成，如 "GLM-5.1 @ DashScope"）
    pub display_name: String,
}

/// 输入材料类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialType {
    /// 文字内容
    Text,
    /// 网页 URL
    Url,
    /// 上传的文件（内容已在前端读取为文本）
    File,
}

/// 为 MaterialType 实现 Display trait，方便日志输出
impl std::fmt::Display for MaterialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaterialType::Text => write!(f, "文字"),
            MaterialType::Url => write!(f, "链接"),
            MaterialType::File => write!(f, "文件"),
        }
    }
}

/// 输入材料结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    /// 材料类型（使用 r#type 因为 type 是 Rust 关键字）
    #[serde(rename = "type")]
    pub material_type: MaterialType,
    /// 材料内容（文字/链接原文；图片为 base64 编码；文件为提取的文本或描述）
    pub content: String,
    /// 文件名（仅文件/图片类型有值，用于大模型理解上下文）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
}

/// 输出格式类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Summary,
    Detailed,
    List,
    Table,
}

/// 分析深度类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisDepth {
    Surface,
    Medium,
    Deep,
}

/// 分析参数配置（字段名直接使用驼峰式，与前端完全一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// 温度：0.1（保守）- 1.0（创意）
    pub temperature: f32,
    /// 公正严明程度：0-100
    pub fairness: u8,
    /// 道德底线：0-100
    pub morality: u8,
    /// 输出格式
    pub outputFormat: OutputFormat,
    /// 分析深度
    pub analysisDepth: AnalysisDepth,
}

// ========== 人物画像相关数据结构 ==========

/// 动机选项结构（支持容错解析）
///
/// 每个人物可以有多个动机选项，用户需要选择一个最可能的动机
///
/// 注意：由于 AI 返回的 JSON 可能不稳定，使用自定义反序列化逻辑
/// 允许将字符串自动转换为结构体（当 AI 返回字符串而非对象时）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Motivation {
    /// 动机唯一 ID
    pub id: String,
    /// 动机描述文本（如"经济利益驱动"、"情感报复"等）
    pub content: String,
    /// AI 给出的可信度评分（0-100）
    #[serde(default = "default_confidence")]
    pub confidence: u8,
    /// 来源提示（如"基于当事人 A 的说法"、"基于网络报道 B"）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sourceHint: Option<String>,
}

/// 默认可信度值（当 AI 未提供时使用）
fn default_confidence() -> u8 {
    70
}

/// 自定义动机解析（支持容错处理）
///
/// 当 AI 返回的 JSON 中 motivations 包含字符串而非结构体时，
/// 自动将字符串转换为 Motivation 结构体
fn parse_motivations(value: serde_json::Value) -> Result<Vec<Motivation>, String> {
    let motivations = match value {
        serde_json::Value::Array(arr) => arr,
        _ => return Err("motivations 应为数组".to_string()),
    };

    let mut result = Vec::new();
    for (index, item) in motivations.iter().enumerate() {
        match item {
            // 正常情况：结构体对象
            serde_json::Value::Object(_) => {
                // 尝试直接解析
                if let Ok(m) = serde_json::from_value(item.clone()) {
                    result.push(m);
                } else {
                    // 如果解析失败，尝试从对象中提取 content 字段
                    if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
                        result.push(Motivation {
                            id: format!("m{}", index + 1),
                            content: content.to_string(),
                            confidence: default_confidence(),
                            sourceHint: None,
                        });
                    }
                }
            }
            // 容错情况：字符串（AI 有时会返回纯字符串）
            serde_json::Value::String(s) => {
                // 跳过纯注释字符串（以"注意"、"说明"等开头的）
                if s.starts_with("注意") || s.starts_with("说明") || s.starts_with("提示") {
                    log::warn!("跳过注释字符串: {}", s);
                    continue;
                }
                // 将内容字符串转换为 Motivation 结构体
                result.push(Motivation {
                    id: format!("m{}", index + 1),
                    content: s.clone(),
                    confidence: default_confidence(),
                    sourceHint: None,
                });
            }
            _ => {
                log::warn!("跳过无效的动机项: {:?}", item);
            }
        }
    }

    Ok(result)
}

/// 人物画像结构（支持容错解析）
///
/// 从材料中提取的人物信息，包括姓名、角色、描述和可选动机列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterProfile {
    /// 人物唯一 ID（UUID 格式）
    #[serde(default = "generate_char_id")]
    pub id: String,
    /// 人物名称
    pub name: String,
    /// 角色（当事人、目击者、旁观者、嫌疑人、受害者等）
    #[serde(default = "default_role")]
    pub role: String,
    /// 人物描述（性格、背景、行为特点等）
    #[serde(default)]
    pub description: String,
    /// 可选动机列表（3-5 个）
    #[serde(default, deserialize_with = "deserialize_motivations")]
    pub motivations: Vec<Motivation>,
}

/// 生成默认人物 ID
fn generate_char_id() -> String {
    format!("char_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("1"))
}

/// 默认角色
fn default_role() -> String {
    "相关人员".to_string()
}

/// 自定义动机反序列化函数
fn deserialize_motivations<'de, D>(deserializer: D) -> Result<Vec<Motivation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    parse_motivations(value).map_err(serde::de::Error::custom)
}

/// 人物画像生成结果
///
/// 包含提取的所有人物画像列表和事件梳理摘要
#[derive(Debug, Serialize, Deserialize)]
pub struct CharacterGenerationResult {
    /// 人物画像列表
    pub characters: Vec<CharacterProfile>,
    /// 是否成功提取人物（如果没有人物，此字段为 false）
    pub hasCharacters: bool,
    /// 错误提示（如果没有人物，给出提示信息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errorMessage: Option<String>,
    /// 事件梳理摘要（时间线、关键事实、信息冲突点等）
    /// 由大模型在提取人物时同时生成，用于后续深度分析
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eventSummary: Option<String>,
}

// ========== "吃瓜包" 结构化结果（Phase 1 新增） ==========
//
// 目的：让深度分析除了 Markdown 长文之外，再产出一份结构化 JSON（TruthExtras），
// 用来在新结果页驱动锐评 Hero / 时间线 / 金句 / 阵营 / 赢家输家 / 反转 / 人物关系网
// 等吃瓜模块的高密度展示。
//
// 落地方式（见 build_analysis_prompt）：
//   让大模型在正文 Markdown **之前**，先输出一段被下列 sentinel 包裹的 JSON：
//     <<<GOSSIP_JSON>>>{...}<<<END_GOSSIP>>>
//   parse_truth_extras_and_longform 负责把 JSON 块剥离，返回 (extras, 纯 markdown)。
//   解析失败时 extras=None、longform=原文，保证"结构化失败也不影响用户看到长文"。
// ==============================================================================

/// 吃瓜包 JSON 的开启 sentinel
const GOSSIP_START: &str = "<<<GOSSIP_JSON>>>";
/// 吃瓜包 JSON 的结束 sentinel
const GOSSIP_END: &str = "<<<END_GOSSIP>>>";

/// 金句卡片
///
/// 代表事件里某个人物/信源说过的一句有"爆点"的原话或经典概括。
/// 吃瓜群众视角下，这是最容易被传播、做表情包、做标题的素材。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HotQuote {
    /// 发言人（当事人 / 官方 / 媒体 / 网友代表等）
    pub speaker: String,
    /// 金句原文
    pub quote: String,
    /// 一句话背景（可选，解释"他在什么场合说的、针对谁"）
    pub context: String,
}

impl Default for HotQuote {
    fn default() -> Self {
        Self { speaker: String::new(), quote: String::new(), context: String::new() }
    }
}

/// 阵营/站队
///
/// 把事件里的人物按主张、利益、舆论倾向划分成 2-3 个阵营。
/// 吃瓜场景下比"中立综述"更有张力——用户一眼看清"谁是一伙的"。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Faction {
    /// 阵营名称（例如"挺 A 派"、"站 B 派"、"冷眼旁观"）
    pub name: String,
    /// 阵营核心立场（一句话）
    pub stance: String,
    /// 代表人物姓名列表（要与 CharacterProfile.name 一致才能连线）
    pub members: Vec<String>,
    /// 这个阵营的一句话主张/口号
    pub keyClaim: String,
}

impl Default for Faction {
    fn default() -> Self {
        Self {
            name: String::new(),
            stance: String::new(),
            members: Vec::new(),
            keyClaim: String::new(),
        }
    }
}

/// 时间线节点
///
/// 按时间顺序的一次关键事件。`isTwist` / `isHot` 用于前端高亮：
/// - isTwist = true：叙事反转点（例如"证据被推翻"）
/// - isHot   = true：爆点事件（流量爆发、关键冲突）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TimelineEvent {
    /// 时间（自由格式：可以是日期、相对时间"事发当晚"、或阶段名"初期"）
    pub time: String,
    /// 事件短标题（≤ 20 字，便于在节点上直接展示）
    pub title: String,
    /// 事件详细描述（1-3 句话）
    pub desc: String,
    /// 是否是反转点
    pub isTwist: bool,
    /// 是否是爆点/高潮点
    pub isHot: bool,
}

impl Default for TimelineEvent {
    fn default() -> Self {
        Self {
            time: String::new(),
            title: String::new(),
            desc: String::new(),
            isTwist: false,
            isHot: false,
        }
    }
}

/// 反转点说明
///
/// 比 `TimelineEvent.isTwist` 更详细——解释"为什么算反转"。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlotTwist {
    /// 引用的时间线事件标题（与 TimelineEvent.title 对应）
    pub eventRef: String,
    /// 为什么这是反转（改变了叙事方向 / 推翻了某说法 / 揭示了隐藏动机）
    pub whyTwist: String,
}

impl Default for PlotTwist {
    fn default() -> Self {
        Self { eventRef: String::new(), whyTwist: String::new() }
    }
}

/// 赢家/输家条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WinLoseItem {
    /// 人物名（与 CharacterProfile.name 对应便于关联）
    pub name: String,
    /// 为什么赢 / 输（一句话）
    pub reason: String,
}

impl Default for WinLoseItem {
    fn default() -> Self {
        Self { name: String::new(), reason: String::new() }
    }
}

/// 赢家输家榜
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WinnersLosers {
    /// 赢家列表（得到利益、名声、话语权的人）
    pub winners: Vec<WinLoseItem>,
    /// 输家列表（失去利益、名声、信任的人）
    pub losers: Vec<WinLoseItem>,
}

impl Default for WinnersLosers {
    fn default() -> Self {
        Self { winners: Vec::new(), losers: Vec::new() }
    }
}

/// 人物关系边
///
/// 构成"人物关系网"的一条边。`sentiment` 决定前端连线颜色：
///  - positive：冷蓝/同盟（合作、联盟）
///  - negative：爆点红/敌对（撕逼、对抗）
///  - neutral：灰/中性（一般关系）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RelationEdge {
    /// 起点人物名
    pub from: String,
    /// 终点人物名
    pub to: String,
    /// 关系标签（"前同事"、"情侣"、"对手"等）
    pub label: String,
    /// 关系情感倾向：positive / negative / neutral
    pub sentiment: String,
}

impl Default for RelationEdge {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            label: String::new(),
            sentiment: "neutral".to_string(),
        }
    }
}

/// 将大模型输出的非标准 sentiment 值规范化为前端认可的 three-value 枚举。
///
/// 大模型经常输出 "hostile"、"enemy"、"friendly" 等变体，
/// 前端 switch 只认 positive / negative / neutral，
/// 未被映射的值一律归为 neutral 并打 warn log。
fn normalize_sentiment(raw: &str) -> String {
    let lowered = raw.trim().to_lowercase();
    match lowered.as_str() {
        // 精确匹配 —— 直接通过
        "positive" | "negative" | "neutral" => lowered,
        // 敌对类变体 → negative
        "hostile" | "enemy" | "adversarial" | "antagonistic" | "oppositional" | "confrontational"
        | "against" | "rival" | "conflict" | "bad" | "negative_sentiment" => {
            log::warn!(
                "sentiment 值 '{}' 不在标准三值中，已映射为 'negative'",
                raw
            );
            "negative".to_string()
        }
        // 同盟类变体 → positive
        "friendly" | "cooperative" | "ally" | "alliance" | "supportive" | "good"
        | "positive_sentiment" | "collaborative" | "amicable" => {
            log::warn!(
                "sentiment 值 '{}' 不在标准三值中，已映射为 'positive'",
                raw
            );
            "positive".to_string()
        }
        // 空字符串 / null → neutral（兜底）
        "" => "neutral".to_string(),
        // 其余未知值 → neutral
        other => {
            log::warn!(
                "sentiment 值 '{}' 未知，已回退为 'neutral'",
                other
            );
            "neutral".to_string()
        }
    }
}

/// 对 TruthExtras 中的 characterRelations 做两道规范化清洗：
/// 1. sentiment 标准化（normalize_sentiment）
/// 2. from/to 人名与已知人物列表交叉匹配（trim 空格 + 去引号 + 首字模糊匹配）
///
/// 返回清洗后的 TruthExtras；无法匹配的边会被丢弃并打 warn log。
fn normalize_character_relations(
    extras: TruthExtras,
    known_names: &[String],
) -> TruthExtras {
    // 构建一个快速查找的 normalized name map：
    // key = 去空格去引号后的小写名 → value = 原始标准名
    let normalized_map: std::collections::HashMap<String, String> = known_names
        .iter()
        .map(|name| {
            let key = name
                .trim()
                .replace('"', "")
                .replace('\u{3000}', "") // 全角空格
                .replace(' ', "")        // 半角空格（对中文名无影响，对英文名可能有）
                .to_lowercase();
            (key, name.clone())
        })
        .collect();

    // 同时保留精确匹配 map（优先精确，再模糊）
    let exact_map: std::collections::HashMap<String, String> = known_names
        .iter()
        .map(|name| (name.trim().to_string(), name.clone()))
        .collect();

    let mut cleaned_relations: Vec<RelationEdge> = Vec::new();

    for edge in extras.characterRelations.iter() {
        // ---- sentiment 规范化 ----
        let norm_sentiment = normalize_sentiment(&edge.sentiment);

        // ---- from/to 人名匹配 ----
        let matched_from = match_exact_then_fuzzy(&edge.from, &exact_map, &normalized_map);
        let matched_to = match_exact_then_fuzzy(&edge.to, &exact_map, &normalized_map);

        match (matched_from, matched_to) {
            (Some(f), Some(t)) => {
                // 精确/模糊匹配成功：用标准名替换原始值
                if f != edge.from || t != edge.to {
                    log::info!(
                        "人物名修正：from '{}' → '{}'，to '{}' → '{}'",
                        edge.from, f, edge.to, t
                    );
                }
                cleaned_relations.push(RelationEdge {
                    from: f,
                    to: t,
                    label: edge.label.clone(),
                    sentiment: norm_sentiment,
                });
            }
            (None, Some(_)) | (Some(_), None) => {
                // 部分匹配失败：丢弃此边
                log::warn!(
                    "关系边丢弃（人物名不匹配）：from='{}', to='{}'",
                    edge.from, edge.to
                );
            }
            (None, None) => {
                // 两个名字都没匹配上：丢弃此边
                log::warn!(
                    "关系边丢弃（两端人物名均不匹配）：from='{}', to='{}'",
                    edge.from, edge.to
                );
            }
        }
    }

    // 如果清洗后边数少于原始边数，打一条汇总信息
    let original_count = extras.characterRelations.len();
    let cleaned_count = cleaned_relations.len();
    if original_count > cleaned_count {
        log::warn!(
            "人物关系边清洗：原始 {} 条 → 保留 {} 条（丢弃 {} 条不匹配的边）",
            original_count, cleaned_count, original_count - cleaned_count
        );
    }

    TruthExtras {
        characterRelations: cleaned_relations,
        ..extras
    }
}

/// 先精确匹配，再模糊匹配（去空格/引号/大小写）。
/// 精确匹配优先，避免 "张三" 被错误匹配到 "张三丰"。
fn match_exact_then_fuzzy(
    raw_name: &str,
    exact_map: &std::collections::HashMap<String, String>,
    normalized_map: &std::collections::HashMap<String, String>,
) -> Option<String> {
    // 1. 精确匹配（trim 后比对，容忍前后空格）
    let trimmed = raw_name.trim();
    if let Some(standard) = exact_map.get(trimmed) {
        return Some(standard.clone());
    }

    // 2. 模糊匹配：去空格、去引号、统一小写后比对
    let fuzzy_key = trimmed
        .replace('"', "")
        .replace('\u{3000}', "")
        .replace(' ', "")
        .to_lowercase();

    // 只有当 fuzzy_key 与精确匹配的 key 不重复时才走模糊
    // （即精确匹配已经失败的前提下）
    if let Some(standard) = normalized_map.get(&fuzzy_key) {
        return Some(standard.clone());
    }

    // 3. 首字符前缀匹配：如果模糊 key 以某标准名的小写首字符开头，
    //    且该标准名也以同一字符开头，视为可能匹配。
    //    这是最后一道防线，仅用于中文名的部分缩写（如"张" → "张三"）。
    if !fuzzy_key.is_empty() {
        for (key, standard) in normalized_map.iter() {
            if key.starts_with(&fuzzy_key) || fuzzy_key.starts_with(key) {
                // 首字符前缀匹配命中率不高，只在长度差 ≤ 2 时才采纳
                if (key.len() as i64 - fuzzy_key.len() as i64).abs() <= 2 {
                    log::info!(
                        "首字符前缀匹配：'{}' → '{}'（标准名）",
                        raw_name, standard
                    );
                    return Some(standard.clone());
                }
            }
        }
    }

    None
}

/// 吃瓜结构化包
///
/// 所有字段用 `#[serde(default)]`，让模型少输出某个字段时只是对应列表为空，
/// 而不是导致整个 JSON 反序列化失败。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TruthExtras {
    /// 一句话锐评（≤ 30 字，用于 Hero 顶部巨字标题）
    pub oneLinerVerdict: String,
    /// 金句卡
    pub hotQuotes: Vec<HotQuote>,
    /// 阵营划分
    pub factions: Vec<Faction>,
    /// 时间线事件（按时间顺序）
    pub timelineEvents: Vec<TimelineEvent>,
    /// 反转点说明
    pub plotTwists: Vec<PlotTwist>,
    /// 赢家输家榜
    pub winnersLosers: WinnersLosers,
    /// 人物关系边
    pub characterRelations: Vec<RelationEdge>,
}

/// analyze_truth 最终返回结果
///
/// - extras：结构化吃瓜包（解析失败则为 None）
/// - longform：Markdown 长文（已剥离 sentinel 块）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TruthAnalysisResult {
    /// 吃瓜结构化数据；None 表示模型没吐出合法 JSON，前端只展示 longform
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<TruthExtras>,
    /// Markdown 格式的完整长文分析（作为"底部可折叠长文"仍然展示）
    pub longform: String,
}

/// 从大模型原始输出中剥离 sentinel 吃瓜包，返回 (extras, 纯 markdown)
///
/// 鲁棒策略：
/// 1. 找到 `<<<GOSSIP_JSON>>>` 和 `<<<END_GOSSIP>>>` 成对出现的位置
/// 2. 中间 JSON 反序列化为 TruthExtras（失败也不阻塞）
/// 3. 把包括 sentinel 在内的整段从原文中删除，剩下的即 Markdown 报告
/// 4. 如果找不到 sentinel，返回 (None, 原文)
fn parse_truth_extras_and_longform(raw: &str) -> (Option<TruthExtras>, String) {
    // 找开始标记
    let Some(start_idx) = raw.find(GOSSIP_START) else {
        return (None, raw.to_string());
    };
    let after_start = start_idx + GOSSIP_START.len();

    // 从 start 之后找结束标记
    let Some(end_rel) = raw[after_start..].find(GOSSIP_END) else {
        // 有开始、没有结束——通常是流被截断。保守处理：把起始 sentinel 前的部分当长文
        log::warn!("吃瓜包 sentinel 只找到开始没找到结束，按不含结构化数据处理");
        let longform = raw[..start_idx].to_string();
        return (None, longform);
    };
    let end_idx = after_start + end_rel;
    let json_raw = raw[after_start..end_idx].trim();

    // 允许模型把 JSON 放在 ```json ... ``` 里——去掉代码围栏
    let json_clean = json_raw
        .trim_start_matches("```json")
        .trim_start_matches("```JSON")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let extras = match serde_json::from_str::<TruthExtras>(json_clean) {
        Ok(v) => Some(v),
        Err(e) => {
            log::warn!("吃瓜包 JSON 解析失败，退化为纯长文：{}", e);
            None
        }
    };

    // 拼回长文：sentinel 之前 + sentinel 结束之后
    let tail_start = end_idx + GOSSIP_END.len();
    let mut longform = String::with_capacity(raw.len());
    longform.push_str(&raw[..start_idx]);
    longform.push_str(&raw[tail_start..]);
    let longform = longform.trim().to_string();

    (extras, longform)
}

// ========== 历史记录相关数据结构 ==========

/// 历史记录存储键名
const HISTORY_KEY: &str = "analysis_history";

/// 分析会话记录结构
///
/// 保存每次分析的历史记录，包括材料、人物、动机选择和分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSession {
    /// 会话唯一 ID
    pub sessionId: String,
    /// 输入材料列表
    pub materials: Vec<Material>,
    /// 人物画像列表（如果生成了）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub characters: Option<Vec<CharacterProfile>>,
    /// 用户选择的动机（人物 ID -> 动机 ID）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selectedMotivations: Option<std::collections::HashMap<String, String>>,
    /// 分析结果 Markdown 长文（老记录可能是 HTML，前端 renderContent 会自动识别）
    pub analysisResult: String,
    /// Phase 1 新增：结构化吃瓜包（老记录没有此字段，反序列化时 serde 会填 None）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truthExtras: Option<TruthExtras>,
    /// 创建时间戳
    pub createdAt: u64,
    /// 分析参数配置
    pub config: AnalysisConfig,
}

/// 保存 API Key 到本地存储
///
/// 注意：使用 tauri-plugin-store 存储，比 keyring 更可靠
/// 虽然安全性稍低，但在 macOS 上更稳定
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `key`: API Key 字符串
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn save_api_key(app: tauri::AppHandle, key: String) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 保存 API Key
    store.set(API_KEY_KEY, key.clone());

    // 立即持久化到磁盘
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("API Key 已保存，长度: {} 字符", key.len());
    Ok("success".to_string())
}

/// 从本地存储获取 API Key
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回 API Key（如果不存在则返回空字符串）
/// - 失败返回错误信息
#[tauri::command]
fn get_api_key(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 获取 API Key
    let key = store.get(API_KEY_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "".to_string());

    log::info!("获取 API Key，长度: {} 字符", key.len());
    Ok(key)
}

/// 删除本地存储中的 API Key
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn delete_api_key(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 删除 API Key
    store.delete(API_KEY_KEY);

    // 立即持久化到磁盘
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("API Key 已删除");
    Ok("success".to_string())
}

/// 保存 Base URL 到本地存储
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `url`: Base URL 字符串
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn save_base_url(app: tauri::AppHandle, url: String) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    store.set(BASE_URL_KEY, url.clone());

    // 立即持久化到磁盘
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("Base URL 已保存: {}", url);
    Ok("success".to_string())
}

/// 从本地存储获取 Base URL
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回 Base URL（默认为 GLM-5 API 地址）
/// - 失败返回错误信息
#[tauri::command]
fn get_base_url(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 如果不存在则返回默认值（GLM-5 via DashScope）
    let url = store.get(BASE_URL_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

    Ok(url)
}

/// 保存 Model 到本地存储
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `model`: 模型名称字符串（如 glm-5.1、qwen-plus 等）
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn save_model(app: tauri::AppHandle, model: String) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    store.set(MODEL_KEY, model.clone());

    // 立即持久化到磁盘
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("Model 已保存: {}", model);
    Ok("success".to_string())
}

/// 从本地存储获取 Model
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回 Model（默认为 glm-5.1）
/// - 失败返回错误信息
#[tauri::command]
fn get_model(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 如果不存在则返回默认值
    let model = store.get(MODEL_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    log::info!("获取 Model: {}", model);
    Ok(model)
}

/// 保存多模态模型到本地存储
///
/// 多模态模型用于处理图片等非文本材料，与文本模型独立配置
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `model`: 多模态模型名称（如 qwen-vl-plus、qwen-vl-max 等）
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn save_multimodal_model(app: tauri::AppHandle, model: String) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    store.set(MULTIMODAL_MODEL_KEY, model.clone());

    // 立即持久化到磁盘
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("多模态模型已保存: {}", model);
    Ok("success".to_string())
}

/// 从本地存储获取多模态模型
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回多模态模型（默认为 qwen-vl-plus）
/// - 失败返回错误信息
#[tauri::command]
fn get_multimodal_model(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 如果不存在则返回默认值
    let model = store.get(MULTIMODAL_MODEL_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| DEFAULT_MULTIMODAL_MODEL.to_string());

    log::info!("获取多模态模型: {}", model);
    Ok(model)
}

// ========== 模型配置列表命令 ==========

/// 保存模型配置列表
///
/// 将前端的 ModelConfig 数组存储到 settings.json
/// 用于支持多个模型配置的保存和切换
#[tauri::command]
fn save_model_configs(app: tauri::AppHandle, configs: Vec<ModelConfig>) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 将 ModelConfig 数组序列化为 JSON 并存储
    let json_value = serde_json::to_value(&configs)
        .map_err(|e| format!("序列化模型配置失败: {}", e))?;
    store.set(MODEL_CONFIGS_KEY, json_value);
    store.save()
        .map_err(|e| format!("保存存储文件失败: {}", e))?;

    log::info!("保存模型配置列表，共 {} 个配置", configs.len());
    Ok("模型配置列表已保存".to_string())
}

/// 获取模型配置列表
///
/// 从 settings.json 读取所有保存的模型配置
/// 首次使用时返回空数组
#[tauri::command]
fn get_model_configs(app: tauri::AppHandle) -> Result<Vec<ModelConfig>, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 从存储中读取配置列表
    let configs = store.get(MODEL_CONFIGS_KEY)
        .and_then(|v| serde_json::from_value::<Vec<ModelConfig>>(v.clone()).ok())
        .unwrap_or_default();

    log::info!("获取模型配置列表，共 {} 个配置", configs.len());
    Ok(configs)
}

/// 保存当前选中的模型配置 ID
///
/// 记录用户当前正在使用的模型配置 ID
/// 用于下次启动时自动选中上次使用的配置
#[tauri::command]
fn save_current_model_config_id(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    store.set(CURRENT_MODEL_CONFIG_ID_KEY, id.clone());
    store.save()
        .map_err(|e| format!("保存存储文件失败: {}", e))?;

    log::info!("保存当前模型配置 ID: {}", id);
    Ok("当前模型配置 ID 已保存".to_string())
}

/// 获取当前选中的模型配置 ID
///
/// 返回上次使用的模型配置 ID
/// 首次使用时返回空字符串
#[tauri::command]
fn get_current_model_config_id(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    let id = store.get(CURRENT_MODEL_CONFIG_ID_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();

    log::info!("获取当前模型配置 ID: {}", id);
    Ok(id)
}

/// 获取当前选中的完整模型配置（包含 baseUrl、apiKey、model、isMultimodal 等）
///
/// 不再依赖旧的单字段存储（API_KEY_KEY、BASE_URL_KEY、MODEL_KEY），
/// 而是从 ModelConfig 列表和当前选中 ID 中获取完整配置。
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回当前选中的 ModelConfig
/// - 如果没有配置（用户还没添加模型），返回错误提示
fn get_current_model_config(app: &tauri::AppHandle) -> Result<ModelConfig, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 1. 获取当前选中的配置 ID
    let current_id = store.get(CURRENT_MODEL_CONFIG_ID_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();

    if current_id.is_empty() {
        return Err("请先选择一个模型配置（点击模型选择按钮添加配置）".to_string());
    }

    // 2. 获取所有模型配置列表
    let configs: Vec<ModelConfig> = store.get(MODEL_CONFIGS_KEY)
        .and_then(|v| serde_json::from_value::<Vec<ModelConfig>>(v.clone()).ok())
        .unwrap_or_default();

    // 3. 根据 ID 找到当前选中的配置
    let config = configs.iter()
        .find(|c| c.id == current_id)
        .cloned();

    match config {
        Some(c) => {
            log::info!("使用模型配置: {} (模型={}, Base URL={})", c.display_name, c.model, c.base_url);
            Ok(c)
        }
        None => Err(format!("找不到模型配置 ID: {}，请重新选择模型", current_id))
    }
}

// ========== 人物画像生成命令 ==========

/// 生成人物画像命令
///
/// 根据输入材料，提取事件中所有人物的画像和可选动机
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `materials`: 输入材料列表（文字和 URL）
/// - `config`: 分析参数配置
///
/// # 返回
/// - 成功返回人物画像生成结果（包含人物列表和动机）
/// - 失败返回错误信息
///
/// # 流程
/// 1. 获取 API Key、Base URL、Model
/// 2. 处理所有材料（获取 URL 内容）
/// 3. 构建人物画像生成 prompt
/// 4. 调用大模型 API
/// 5. 解析 JSON 格式的人物画像
/// 6. 返回人物列表（如果没有人物，返回失败提示）
#[tauri::command]
async fn generate_characters(
    app: tauri::AppHandle,
    materials: Vec<Material>,
    config: AnalysisConfig,
    // Phase 1 新增：流式事件推送通道。前端通过 new Channel() 创建并在此处订阅。
    on_event: Channel<StreamEvent>,
) -> Result<CharacterGenerationResult, String> {
    log::info!("开始生成人物画像，材料数量: {}", materials.len());

    // 进入"准备阶段"：前端可以据此把进度条第 1 段点亮
    emit_event(&on_event, StreamEvent::Phase {
        phase: "reading".to_string(),
        label: "正在读取并整理材料...".to_string(),
    });

    // 1. 获取当前选中的完整模型配置（包含 baseUrl、apiKey、model 等）
    // 不再依赖旧的硬编码默认值，所有模型配置都由用户在前端选择
    let model_config = get_current_model_config(&app)?;

    let api_key = model_config.api_key;
    let base_url = model_config.base_url;
    let model = model_config.model;

    log::info!("使用 Base URL: {}", base_url);
    log::info!("文本模型: {}", model);

    // 2. 多模态模型：如果当前配置是多模态模型，直接用当前模型处理图片
    // 否则使用当前模型名称作为多模态模型（用户在前端选择的模型即为最终使用的模型）
    let multimodal_model = model.clone();

    log::info!("多模态模型: {}", multimodal_model);

    // 3. 处理所有材料，同时追踪是否有需要 LLM 联网搜索的材料
    let mut all_content = String::new();
    // 标记是否有材料的实际内容获取失败，需要 LLM 通过联网搜索补充
    let mut needs_llm_search = false;
    // 收集图片材料（base64 编码），用于多模态 API 调用
    let mut image_items: Vec<ImageItem> = Vec::new();

    for (index, material) in materials.iter().enumerate() {
        log::info!("处理材料 {}: 类型={}", index, material.material_type);

        match material.material_type {
            MaterialType::Text => {
                all_content.push_str(&format!("\n【材料 {} - 文字信息】\n", index + 1));
                all_content.push_str(&material.content);
                all_content.push_str("\n");
            }
            MaterialType::File => {
                // 判断是否为图片文件（content 以 base64 图片数据开头）
                // 图片文件的 content 格式：以 base64 编码的图片数据存储
                // 文本文件的 content 格式：普通文本或描述信息
                let file_name = material.file_name.clone().unwrap_or_default();
                let is_image = is_image_file(&file_name);

                if is_image && material.content.len() > 100 {
                    // 图片文件：content 是 base64 编码数据，加入多模态图片列表
                    image_items.push(ImageItem {
                        base64_data: material.content.clone(),
                        file_name: file_name.clone(),
                    });
                    all_content.push_str(&format!("\n【材料 {} - 图片】{}\n", index + 1, file_name));
                } else {
                    // 文本/描述文件：直接当作文字信息处理
                    all_content.push_str(&format!("\n【材料 {} - 文件内容】", index + 1));
                    if !file_name.is_empty() {
                        all_content.push_str(&format!(" 文件名: {}", file_name));
                    }
                    all_content.push_str("\n");
                    all_content.push_str(&material.content);
                    all_content.push_str("\n");
                }
            }
            MaterialType::Url => {
                let webpage_content = fetch_webpage_content(&material.content)
                    .await
                    .map_err(|e| {
                        log::error!("获取网页内容失败: {}", e);
                        format!("获取网页 {} 内容失败: {}", material.content, e)
                    })?;

                // 检查是否为 LLM 搜索兜底内容（数据来源标记中包含"LLM联网搜索"）
                if webpage_content.contains("[数据来源：LLM联网搜索") {
                    needs_llm_search = true;
                    log::info!("材料 {} 的网页内容获取失败，将依赖 LLM 联网搜索补充", index + 1);
                }

                all_content.push_str(&format!("\n【材料 {} - 网页内容】\n来源: {}\n", index + 1, material.content));
                all_content.push_str(&webpage_content);
                all_content.push_str("\n");
            }
        }
    }

    log::info!("所有材料已处理，总内容长度: {} 字符，需要LLM搜索: {}", all_content.len(), needs_llm_search);

    // 限制内容长度
    if all_content.len() > 12000 {
        all_content = all_content[..12000].to_string();
        log::warn!("内容已截断到 12000 字符");
    }

    // 5. 构建人物画像生成 prompt
    let system_prompt = build_character_prompt(&config);

    // 根据材料情况，构建不同的用户提示
    // 三种情况都会触发 LLM 联网搜索模式：
    // 1. 材料内容很短（<200字符）—— 可能只是关键词
    // 2. 有材料的网页抓取失败，需要 LLM 搜索补充 —— needs_llm_search
    // 3. 两者兼有
    let user_prompt = if all_content.len() < 200 || needs_llm_search {
        format!(
            "用户提供的信息可能不完整，部分网页内容无法直接获取。\n\
            你**必须先通过联网搜索**获取相关的详细信息（人物背景、事件细节、时间线等），\n\
            然后基于搜索到的真实信息来提取人物和生成画像。\n\
            绝不能因为信息不足就返回 hasCharacters: false。\n\n\
            已有信息：{}\n\n\
            请先搜索补充信息，然后分析并提取所有相关人物，为每个人物生成画像和动机选项。",
            all_content
        )
    } else {
        // 材料看起来较长，但仍建议联网搜索补充细节
        format!(
            "请先通过联网搜索确认和补充以下材料中涉及的人物背景、事件细节等关键信息，\n\
            焭后基于所有信息（包括搜索到的补充信息）提取人物画像和动机。\n\
            如果材料中无法提取人物信息，请通过联网搜索查找相关事件的真实人物，\n\
            绝不能直接返回 hasCharacters: false。\n\n\
            材料内容：{}\n\n\
            请搜索补充信息后，提取所有相关人物，为每个人物生成画像和动机选项。",
            all_content
        )
    };

    // 6. 调用大模型（流式）
    // 阶段切换：准备完毕，开始调用 LLM
    emit_event(&on_event, StreamEvent::Phase {
        phase: "extracting".to_string(),
        label: "AI 正在提取人物与动机...".to_string(),
    });

    let raw_result = if image_items.is_empty() {
        call_llm_api_with_prompt_stream(
            &base_url,
            &api_key,
            &model,
            &system_prompt,
            &user_prompt,
            config.temperature,
            &on_event,
            true, // 首次尝试启用搜索
        )
        .await
        .map_err(|e| {
            log::error!("调用大模型失败: {}", e);
            emit_event(&on_event, StreamEvent::Error { message: format!("调用大模型失败: {}", e) });
            format!("调用大模型失败: {}", e)
        })?
    } else {
        log::info!("检测到 {} 张图片，切换到多模态模型: {}", image_items.len(), multimodal_model);

        call_multimodal_llm_stream(
            &base_url,
            &api_key,
            &multimodal_model,
            &system_prompt,
            &user_prompt,
            &image_items,
            config.temperature,
            &on_event,
            true,
        )
        .await
        .map_err(|e| {
            log::error!("调用多模态大模型失败: {}", e);
            emit_event(&on_event, StreamEvent::Error { message: format!("调用多模态大模型失败: {}", e) });
            format!("调用多模态大模型失败: {}", e)
        })?
    };

    log::info!("人物画像生成完成，开始解析 JSON");

    // 7. 解析 JSON（带降级重试）
    // ---------------------------------------------------------------------
    // 某些 provider（如智谱 GLM 联网搜索开启时）会把 **搜索工具调用参数**
    // 当成 content 吐出来（例如 `{"search_queries":[...]}` 或
    // `[{"query":"...", "max_results":5}, ...]`）。此时第一次 parse 必失败。
    // 降级策略：检测到这种泄漏签名，**关闭搜索**再跑一次，并收紧 user prompt。
    // 关闭搜索后模型就没有"工具"可调，自然会老实按我们要求的格式输出。
    // ---------------------------------------------------------------------
    let result: CharacterGenerationResult = match parse_character_result(&raw_result) {
        Ok(r) => r,
        Err(first_err) if looks_like_tool_call_leak(&raw_result) => {
            log::warn!(
                "检测到搜索工具调用泄漏到 content，首次 parse 失败（{}），开始降级重试（禁用搜索）",
                first_err
            );
            emit_event(&on_event, StreamEvent::Phase {
                phase: "extracting".to_string(),
                label: "AI 上次被搜索打断，正在换一种方式重新提取...".to_string(),
            });

            // 重写 user_prompt：明确禁止搜索输出
            let retry_user_prompt = format!(
                "请**直接**基于以下材料提取人物，不要进行任何联网搜索，也绝对不要输出 \
                 search_queries、query、tool_calls 等工具调用参数。\n\
                 你的输出必须严格遵守系统提示中的 sentinel + JSON 格式。\n\n\
                 材料内容：\n{}",
                all_content
            );

            let retry_raw = if image_items.is_empty() {
                call_llm_api_with_prompt_stream(
                    &base_url, &api_key, &model,
                    &system_prompt, &retry_user_prompt,
                    config.temperature, &on_event,
                    false, // 关键：关闭搜索
                )
                .await
                .map_err(|e| format!("重试调用失败: {}", e))?
            } else {
                call_multimodal_llm_stream(
                    &base_url, &api_key, &multimodal_model,
                    &system_prompt, &retry_user_prompt,
                    &image_items, config.temperature, &on_event,
                    false,
                )
                .await
                .map_err(|e| format!("重试调用失败: {}", e))?
            };

            parse_character_result(&retry_raw).map_err(|retry_err| {
                log::error!("重试后仍解析失败: {}", retry_err);
                log::error!("重试原始响应（前 2000）: {}", retry_raw.chars().take(2000).collect::<String>());
                "模型两次都没有按要求输出人物 JSON，可能该模型不适合此任务。请在设置里换一个模型（推荐 qwen-max / deepseek-chat 等）。".to_string()
            })?
        }
        Err(e) => {
            log::error!("解析人物画像 JSON 失败: {}", e);
            log::error!("原始响应内容（前 2000 字符）: {}", raw_result.chars().take(2000).collect::<String>());
            return Err(if !raw_result.contains("\"characters\"") {
                "模型没有按预期输出人物 JSON，可能正忙于联网搜索或生成被中断。请稍后重试或换一个模型。".to_string()
            } else {
                format!("解析人物画像失败: {}。请稍后重试。", e)
            });
        }
    };

    // 8. 检查是否有人物
    if !result.hasCharacters || result.characters.is_empty() {
        log::warn!("材料中没有提取到人物");
        return Ok(CharacterGenerationResult {
            characters: vec![],
            hasCharacters: false,
            errorMessage: Some("材料中没有提取到人物信息，无法进行人物画像分析。请添加包含人物事件的材料。".to_string()),
            eventSummary: None,
        });
    }

    log::info!("成功提取 {} 个人物", result.characters.len());
    // 通知前端：人物画像阶段结束，把拼接好的完整 JSON 文本也回传一份
    // （前端不一定需要，但留着便于日志/调试）
    emit_event(&on_event, StreamEvent::Done { full_text: raw_result.clone() });
    Ok(result)
}

/// 构建人物画像生成 prompt
///
/// # 参数
/// - `config`: 分析参数配置
///
/// # 返回
/// - 人物画像生成的系统 prompt
fn build_character_prompt(config: &AnalysisConfig) -> String {
    let mut prompt = String::from(
        "你是一个侦探助手，专门分析事件中的人物行为和动机。\n\n"
    );

    // 添加联网搜索指导 - 强化案例搜索
    prompt.push_str("【联网搜索 - 极其重要】\n");
    prompt.push_str("你拥有联网搜索能力，可以通过搜索获取实时信息。\n");
    prompt.push_str("当材料简短或只是关键词时，必须先搜索获取事件背景和人物详情。\n");
    prompt.push_str("绝不能因材料简短就返回 hasCharacters: false，应主动搜索补充。\n");
    prompt.push_str("同时搜索同类事件中类似角色的行为模式，作为动机推断的参照。\n");
    prompt.push_str("例如：用户输入\"王一博和肖战的故事\"，搜索他们之间的具体事件，\n");
    prompt.push_str("并搜索娱乐圈类似冲突案例，对比分析人物动机模式。\n\n");

    // 添加怀疑态度指导 - 三追问原则
    prompt.push_str("【核心原则 - 三追问】\n");
    prompt.push_str("对每个人物追问三件事：\n");
    prompt.push_str("1. 他为什么选择这样呈现自己——公众形象和实际行为是否矛盾？\n");
    prompt.push_str("2. 他隐瞒了什么——公开叙事的漏洞在哪？\n");
    prompt.push_str("3. 谁受益于他目前的形象——维护这个形象对谁有利？\n");
    prompt.push_str("不预设谁好谁坏，每个人都在维护自己的叙事版本。\n\n");

    // 添加公正性要求
    let fairness_guide = match config.fairness {
        80..=100 => "你必须极度客观公正，不带任何偏见，严格按照事实进行分析人物。",
        60..=79 => "你应该较为中立地分析人物动机，尽量保持客观。",
        _ => "你可以平衡地进行分析人物，既考虑客观事实，也考虑情感因素。",
    };
    prompt.push_str("【公正性要求】\n");
    prompt.push_str(fairness_guide);
    prompt.push_str("\n\n");

    // 重要：只提取重要人物
    prompt.push_str("【重要：人物筛选原则】\n");
    prompt.push_str("只提取对事件发展有重要影响的人物，忽略以下次要人物：\n");
    prompt.push_str("- 仅有提及但无实际行动或言论的人物\n");
    prompt.push_str("- 与事件无直接关系的旁观者\n");
    prompt.push_str("- 仅作为背景提及的无关人员\n");
    prompt.push_str("- 不影响事件走向的次要配角\n");
    prompt.push_str("\n");
    prompt.push_str("提取标准：人物必须有明确的言行描述，或其行为/决定直接影响事件发展。\n");
    prompt.push_str("通常只提取 3-6 个核心人物即可。\n\n");

    // 添加输出格式要求（JSON + 流式 sentinel）
    // ----------------------------------------------------------------
    // Phase 2-A：要求模型在最终 JSON 之前，每识别出一个人物就先输出一行
    // sentinel 包裹的单人物 JSON。这样前端可以边接边显示人物卡片，
    // 不用等整个 JSON 结束。最终完整 JSON 仍然要输出，作为结构化解析的
    // 单一真相来源，避免我们去拼"半截 JSON"。
    // ----------------------------------------------------------------
    prompt.push_str("【输出格式 - 流式 + JSON】\n");
    prompt.push_str("**输出顺序必须严格遵守**：\n\n");
    prompt.push_str("第一步：每识别出一个核心人物，立刻单独输出一行，格式如下（必须在同一行内，前后包裹 sentinel）：\n");
    prompt.push_str("<<CHAR>>{\"id\":\"char_1\",\"name\":\"...\",\"role\":\"...\",\"description\":\"...\",\"motivations\":[{\"id\":\"m1\",\"content\":\"...\",\"confidence\":80}]}<<END>>\n\n");
    prompt.push_str("sentinel 行之间用换行分隔，不要把多个人物塞到一行。\n");
    prompt.push_str("sentinel 内的 JSON 必须是紧凑的单行 JSON，字段与最终完整 JSON 中的人物对象一致。\n\n");
    prompt.push_str("第二步：所有人物 sentinel 输出完后，再输出完整的结构化 JSON（这是结果的唯一真实来源）：\n");
    prompt.push_str("```json\n");
    prompt.push_str("{\n");
    prompt.push_str("  \"hasCharacters\": true,\n");
    prompt.push_str("  \"eventSummary\": \"事件梳理摘要（时间线、关键事实、信息冲突点、各来源可信度评估等，用纯文字描述，不超过500字）\",\n");
    prompt.push_str("  \"characters\": [\n");
    prompt.push_str("    {\n");
    prompt.push_str("      \"id\": \"char_1\",\n");
    prompt.push_str("      \"name\": \"人物名称\",\n");
    prompt.push_str("      \"role\": \"角色类型（当事人/目击者/嫌疑人/受害者等）\",\n");
    prompt.push_str("      \"description\": \"人物描述（性格、背景、行为特点、公众形象与实际行为差异）\",\n");
    prompt.push_str("      \"motivations\": [\n");
    prompt.push_str("        {\n");
    prompt.push_str("          \"id\": \"m1\",\n");
    prompt.push_str("          \"content\": \"动机描述（如：经济利益驱动）\",\n");
    prompt.push_str("          \"confidence\": 80,\n");
    prompt.push_str("          \"sourceHint\": \"来源提示（可选）\"\n");
    prompt.push_str("        },\n");
    prompt.push_str("        {\n");
    prompt.push_str("          \"id\": \"m2\",\n");
    prompt.push_str("          \"content\": \"动机描述2\",\n");
    prompt.push_str("          \"confidence\": 60\n");
    prompt.push_str("        }\n");
    prompt.push_str("      ]\n");
    prompt.push_str("    }\n");
    prompt.push_str("  ]\n");
    prompt.push_str("}\n");
    prompt.push_str("```\n\n");
    prompt.push_str("禁止在 sentinel 或最终 JSON 之外输出额外解释文字。\n");
    // Phase 2-A 修复：避免模型把工具调用 JSON（如 {"search_queries":[...]}）
    // 误写进 content 通道，导致前端解析失败。
    prompt.push_str("**搜索查询由系统工具处理，不要把 search_queries 等工具调用参数写进输出**。\n");
    prompt.push_str("你的最终可见输出 **必须** 以 `<<CHAR>>` 开头（第一行），以最终 JSON 结束。\n\n");

    // 添加提取要求
    prompt.push_str("【提取要求】\n");
    prompt.push_str("1. 只提取对事件有重要影响的核心人物（通常 3-6 人）\n");
    prompt.push_str("2. 每个人物生成 3-5 个可能的动机或原因\n");
    prompt.push_str("3. 动机要有可信度评分（0-100），基于材料中的证据强度\n");
    prompt.push_str("4. 动机要多样化，涵盖利益、情感、名誉、复仇等不同类型\n");
    prompt.push_str("5. eventSummary 必须包含：事件时间线、关键事实、信息冲突点、各来源可信度评估\n");
    prompt.push_str("6. eventSummary 用纯文字描述，不超过500字，用于后续深度分析参考\n");
    prompt.push_str("7. 如果材料中没有核心人物，返回：{\"hasCharacters\": false, \"characters\": [], \"errorMessage\": \"...\", \"eventSummary\": \"简要事件概述\"}\n");
    prompt.push_str("8. ID 使用简单格式：char_1, char_2 等；m1, m2 等\n");
    prompt.push_str("9. motivations 必须是对象数组，不要添加字符串注释\n");

    prompt
}

/// 从响应中提取 JSON 字符串
///
/// 处理可能的 markdown 包裹（```json ... ```）情况
///
/// # 参数
/// - `response`: 原始响应文本
///
/// 判断原始响应是否看起来像"搜索工具调用泄漏到 content 通道"
///
/// 典型泄漏签名：
/// - 包含 `"search_queries"` / `"query"` + `"max_results"` 字段
/// - 或整个响应是一个数组 `[{"query": ..., "max_results": ...}, ...]`
/// - 且**不包含** `"characters"` 字段（排除模型正常输出里引用 query 的情况）
fn looks_like_tool_call_leak(raw: &str) -> bool {
    if raw.contains("\"characters\"") {
        return false;
    }
    // 常见泄漏模式：query + max_results 成对出现
    let has_query_tool_shape =
        raw.contains("\"query\"") && raw.contains("\"max_results\"");
    let has_search_queries_key = raw.contains("\"search_queries\"");
    let has_tool_calls_key = raw.contains("\"tool_calls\"") || raw.contains("\"function_call\"");
    has_query_tool_shape || has_search_queries_key || has_tool_calls_key
}

/// 把 raw LLM 输出解析成 CharacterGenerationResult
///
/// 封装"抽取 + 反序列化"两步，便于在 generate_characters 里做重试。
fn parse_character_result(raw: &str) -> Result<CharacterGenerationResult, String> {
    let json_str = extract_characters_json(raw);
    serde_json::from_str::<CharacterGenerationResult>(&json_str)
        .map_err(|e| format!("{}（抽取长度 {}）", e, json_str.len()))
}

/// 专用于人物画像响应的 JSON 抽取（Phase 2-A 修复）
///
/// 与通用 `extract_json_from_response` 的区别：
/// 1. 先剥掉所有 `<<CHAR>>...<<END>>` sentinel 块（它们内部是单人物 JSON，
///    会误导通用抽取器只拿第一个 JSON）
/// 2. 主动寻找包含 `"characters"` 字段的 JSON 对象，跳过搜索查询 JSON
///    （例如 `{"search_queries":[...]}` —— 某些提供商会把工具调用 JSON
///    误写进 content 通道）
/// 3. 全部失败再降级到通用抽取器
fn extract_characters_json(response: &str) -> String {
    // Step 1: 剥 sentinel。非贪婪，跨行。
    let sentinel_re = regex::Regex::new(r"(?s)<<CHAR>>.*?<<END>>").unwrap();
    let stripped = sentinel_re.replace_all(response, "").to_string();

    // Step 2a: 优先找 ```json / ``` 代码块里包含 "characters" 字段的
    let fenced_re = regex::Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").unwrap();
    for cap in fenced_re.captures_iter(&stripped) {
        let candidate = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if candidate.contains("\"characters\"") {
            return candidate.to_string();
        }
    }

    // Step 2b: 裸文本中找"含 characters 字段的对象"。
    // 策略：定位 "characters" 关键字，从该位置向前找最近的 `{`，
    //       然后从 `{` 向后做括号配对，找到闭合的 `}`。
    if let Some(pos) = stripped.find("\"characters\"") {
        let before = &stripped[..pos];
        if let Some(start) = before.rfind('{') {
            let mut depth: i32 = 0;
            let mut end_byte: Option<usize> = None;
            let slice = &stripped[start..];
            let mut in_string = false;
            let mut escape = false;
            let bytes = slice.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                // 最简单的字符串边界处理，避免把 JSON 字符串里的 `}` 当闭合
                if escape { escape = false; continue; }
                if in_string {
                    match b {
                        b'\\' => escape = true,
                        b'"' => in_string = false,
                        _ => {}
                    }
                    continue;
                }
                match b {
                    b'"' => in_string = true,
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end_byte = Some(i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end) = end_byte {
                return slice[..end].to_string();
            }
        }
    }

    // Step 3: 全部失败，降级到通用抽取器（让调用方去报原始错误）
    extract_json_from_response(&stripped)
}

/// # 返回
/// - 提取后的 JSON 字符串
fn extract_json_from_response(response: &str) -> String {
    // 尝试提取 markdown 包裹的 JSON（```json ... ```）
    if response.contains("```json") {
        // 找到 ```json 的开始位置，跳过这个标记
        let start_idx = response.find("```json").map(|i| i + 7).unwrap_or(0);
        // 从 start_idx 之后找结束的 ```（重要：要从 start_idx 之后开始找）
        let remaining = &response[start_idx..];
        let end_idx = remaining.find("```").unwrap_or(remaining.len());
        // 提取 JSON 内容
        let json_content = &remaining[..end_idx].trim();
        if !json_content.is_empty() {
            return json_content.to_string();
        }
    }

    // 尝试提取普通代码块（``` ... ```）
    if response.contains("```") {
        let parts: Vec<&str> = response.split("```").collect();
        if parts.len() >= 3 {
            // parts[0] 是开头，parts[1] 是代码块内容，parts[2] 是结尾
            let content = parts[1].trim();
            // 如果开头有语言标识符（如 json），去掉它
            if content.starts_with("json") {
                return content[4..].trim().to_string();
            }
            return content.to_string();
        }
    }

    // 直接返回原始响应（假设就是纯 JSON）
    response.trim().to_string()
}

// ========== 历史记录命令 ==========

/// 保存分析历史记录
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `session`: 分析会话记录
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn save_history(app: tauri::AppHandle, session: AnalysisSession) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 获取现有历史记录
    let mut history: Vec<AnalysisSession> = store.get(HISTORY_KEY)
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    // 添加新记录（限制最多 50 条）
    history.push(session);
    if history.len() > 50 {
        history = history[history.len() - 50..].to_vec();
    }

    // 保存历史记录
    store.set(HISTORY_KEY, serde_json::to_value(&history)
        .map_err(|e| format!("序列化历史记录失败: {}", e))?);

    // 立即持久化
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("历史记录已保存，当前共 {} 条", history.len());
    Ok("success".to_string())
}

/// 获取分析历史记录列表
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回历史记录列表（按时间倒序）
/// - 失败返回错误信息
#[tauri::command]
fn get_history(app: tauri::AppHandle) -> Result<Vec<AnalysisSession>, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    let history: Vec<AnalysisSession> = store.get(HISTORY_KEY)
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    // 按时间倒序排列
    let mut sorted_history = history;
    sorted_history.sort_by(|a, b| b.createdAt.cmp(&a.createdAt));

    log::info!("获取历史记录，共 {} 条", sorted_history.len());
    Ok(sorted_history)
}

/// 删除单条历史记录
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `sessionId`: 要删除的会话 ID
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn delete_history_item(app: tauri::AppHandle, sessionId: String) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 获取现有历史记录
    let history: Vec<AnalysisSession> = store.get(HISTORY_KEY)
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    // 过滤掉要删除的记录
    let filtered_history: Vec<AnalysisSession> = history
        .iter()
        .filter(|s| s.sessionId != sessionId)
        .cloned()
        .collect();

    // 保存更新后的历史记录
    store.set(HISTORY_KEY, serde_json::to_value(&filtered_history)
        .map_err(|e| format!("序列化历史记录失败: {}", e))?);

    // 立即持久化
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("已删除历史记录: {}", sessionId);
    Ok("success".to_string())
}

/// 清空所有历史记录
///
/// # 参数
/// - `app`: Tauri 应用句柄
///
/// # 返回
/// - 成功返回 "success"
/// - 失败返回错误信息
#[tauri::command]
fn clear_history(app: tauri::AppHandle) -> Result<String, String> {
    let store = app.store(STORE_FILE)
        .map_err(|e| format!("获取存储失败: {}", e))?;

    // 删除历史记录
    store.delete(HISTORY_KEY);

    // 立即持久化
    store.save()
        .map_err(|e| format!("保存配置文件失败: {}", e))?;

    log::info!("已清空所有历史记录");
    Ok("success".to_string())
}

/// 分析网页内容
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `url`: 要分析的网页 URL
///
/// # 返回
/// - 成功返回网页主题分析结果
/// - 失败返回错误信息
///
/// # 流程
/// 1. 获取 API Key（检查是否配置）
/// 2. 获取 Base URL
/// 3. 获取网页内容
/// 4. 调用大模型 API 分析内容
/// 5. 返回分析结果
#[tauri::command]
async fn analyze_webpage(app: tauri::AppHandle, url: String) -> Result<String, String> {
    log::info!("开始分析网页: {}", url);

    // 1. 获取 API Key（使用内部函数，不需要 command）
    let api_key = {
        let store = app.store(STORE_FILE)
            .map_err(|e| format!("获取存储失败: {}", e))?;

        store.get(API_KEY_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "".to_string())
    };

    // 检查 API Key 是否配置
    if api_key.is_empty() {
        log::warn!("API Key 未配置");
        return Err("请先在设置中配置 API Key".to_string());
    }

    log::info!("API Key 已配置，长度: {} 字符", api_key.len());

    // 2. 获取 Base URL
    let base_url = {
        let store = app.store(STORE_FILE)
            .map_err(|e| format!("获取存储失败: {}", e))?;

        store.get(BASE_URL_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
    };

    log::info!("使用 Base URL: {}", base_url);

    // 3. 获取网页内容
    let webpage_content = fetch_webpage_content(&url)
        .await
        .map_err(|e| {
            log::error!("获取网页内容失败: {}", e);
            format!("获取网页内容失败: {}", e)
        })?;

    log::info!("网页内容获取成功，长度: {} 字符", webpage_content.len());

    // 4. 调用大模型分析
    let analysis_result = call_llm_api(&base_url, &api_key, &webpage_content)
        .await
        .map_err(|e| {
            log::error!("调用大模型失败: {}", e);
            format!("调用大模型失败: {}", e)
        })?;

    log::info!("分析完成");
    Ok(analysis_result)
}

/// 检测系统中已安装的 Chrome/Chromium 二进制文件路径
///
/// # 检测顺序
/// - macOS: Google Chrome → Chromium → Chrome Canary
/// - Windows: Chrome.exe (Program Files) → Chromium → Chrome Canary
/// - Linux: google-chrome → chromium-browser → chromium → chrome
///
/// # 返回
/// - 找到返回 Chrome 可执行文件路径
/// - 未找到返回 None
fn detect_chrome_path() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        // macOS 上 Chrome 的常见安装路径
        let mac_paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        ];
        for path in &mac_paths {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 上 Chrome 的常见安装路径
        // 需要检查 LOCALAPPDATA 和 PROGRAMFILES 环境变量
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let program_files = std::env::var("PROGRAMFILES").unwrap_or_default();
        let program_files_x86 = std::env::var("PROGRAMFILES(X86)").unwrap_or_default();

        let win_paths = [
            format!("{}\\Google\\Chrome\\Application\\chrome.exe", program_files),
            format!("{}\\Google\\Chrome\\Application\\chrome.exe", program_files_x86),
            format!("{}\\Google\\Chrome\\Application\\chrome.exe", local_app_data),
            // Chromium 的可能路径
            format!("{}\\Chromium\\Application\\chrome.exe", program_files),
            format!("{}\\Chromium\\Application\\chrome.exe", local_app_data),
        ];
        for path in &win_paths {
            if std::path::Path::new(path).exists() {
                return Some(path.clone());
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        // Linux 上 Chrome 的常见二进制文件名
        // 使用 which 命令查找，或检查已知路径
        let linux_bin_names = [
            "google-chrome",
            "google-chrome-stable",
            "chromium-browser",
            "chromium",
            "chrome",
        ];
        // 先尝试已知固定路径
        let linux_fixed_paths = [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium-browser",
            "/usr/bin/chromium",
            "/usr/local/bin/chrome",
            "/snap/bin/chromium",
        ];
        for path in &linux_fixed_paths {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }
        // 使用 which 命令搜索（适用于非标准安装路径）
        for bin_name in &linux_bin_names {
            if let Ok(output) = std::process::Command::new("which")
                .arg(bin_name)
                .output()
            {
                if output.status.success() {
                    let found_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !found_path.is_empty() && std::path::Path::new(&found_path).exists() {
                        return Some(found_path);
                    }
                }
            }
        }
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// 使用 Chrome headless 模式抓取网页渲染后的内容
///
/// # 工作原理
/// - 调用系统已安装的 Chrome 以 --headless 模式运行
/// - 使用 --dump-dom 参数获取 JS 渲染后的完整 DOM
/// - --virtual-time-budget=10000 限制 JS 执行时间（10秒）
/// - --disable-gpu 禁用 GPU（headless 下不需要）
///
/// # 参数
/// - `url`: 要抓取的网页 URL
/// - `chrome_path`: Chrome 可执行文件路径
///
/// # 返回
/// - 成功返回渲染后的网页文本内容
/// - 失败返回错误信息
async fn fetch_with_chrome_headless(url: &str, chrome_path: &str) -> Result<String, String> {
    log::info!("使用 Chrome headless 模式抓取: {} (路径: {})", url, chrome_path);

    let output = tokio::process::Command::new(chrome_path)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--virtual-time-budget=10000",
            "--dump-dom",
            url,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("执行 Chrome headless 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("Chrome headless 执行失败: {}", stderr);
        return Err(format!("Chrome headless 执行失败: {}", stderr));
    }

    let html_content = String::from_utf8_lossy(&output.stdout).to_string();

    // Chrome --dump-dom 输出的是渲染后的 DOM（JS 已执行），需要提取文本
    let text_content = extract_text_from_html(&html_content);

    // 如果提取的文本内容太少，可能说明页面加载不完整
    if text_content.trim().len() < 50 {
        log::warn!("Chrome headless 提取的文本内容过少（{}字符），可能页面未完全加载", text_content.trim().len());
        return Err(format!(
            "Chrome headless 提取的内容过少（{}字符），页面可能未完全加载",
            text_content.trim().len()
        ));
    }

    log::info!("Chrome headless 抓取成功，文本长度: {} 字符", text_content.len());
    Ok(text_content)
}

/// 获取网页内容（分层降级策略）
///
/// # 降级顺序
/// 1. **reqwest HTTP GET** — 简单快速，适用于大多数静态网页
/// 2. **Chrome headless** — 渲染 JS 动态内容，适用于需要 JS 执行的现代网页
/// 3. **LLM 搜索兜底** — 所有抓取方式都失败时，将 URL 原文传给 LLM，
///    利用大模型的联网搜索功能获取相关信息
///
/// # 参数
/// - `url`: 网页 URL
///
/// # 返回
/// - 成功返回网页文本内容（标注了使用的抓取方式）
/// - 全部失败时返回 LLM 搜索兜底的提示文本
async fn fetch_webpage_content(url: &str) -> Result<String, String> {
    // ===== 第一层：reqwest HTTP GET =====
    log::info!("第1层：尝试 reqwest GET 请求抓取 {}", url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .send()
        .await;

    match response {
        Ok(resp) => {
            let content = resp
                .text()
                .await
                .map_err(|e| format!("读取网页内容失败: {}", e))?;

            let text_content = extract_text_from_html(&content);

            // 检查提取的文本是否足够丰富（至少100字符才算有意义的内容）
            if text_content.trim().len() >= 100 {
                log::info!("reqwest 抓取成功，文本长度: {} 字符", text_content.len());
                return Ok(format!("[数据来源：HTTP直接抓取]\n{}", text_content));
            }

            // reqwest 拿到了 HTML 但提取的文本太少，可能是 JS 渲染页面
            log::info!(
                "reqwest 抓取到 HTML 但文本内容太少（{}字符），可能是 JS 渲染页面，尝试 Chrome headless",
                text_content.trim().len()
            );
        }
        Err(e) => {
            log::warn!("reqwest 请求失败: {}，尝试 Chrome headless", e);
        }
    }

    // ===== 第二层：Chrome headless =====
    if let Some(chrome_path) = detect_chrome_path() {
        log::info!("第2层：检测到 Chrome，路径: {}", chrome_path);
        match fetch_with_chrome_headless(url, &chrome_path).await {
            Ok(content) => {
                return Ok(format!("[数据来源：Chrome Headless渲染抓取]\n{}", content));
            }
            Err(e) => {
                log::warn!("Chrome headless 也失败了: {}，进入 LLM 搜索兜底", e);
            }
        }
    } else {
        log::info!("第2层：系统未安装 Chrome/Chromium，跳过 headless 模式");
    }

    // ===== 第三层：LLM 搜索兜底 =====
    // 所有技术手段都失败时，将 URL 直接传给大模型，
    // 利用大模型的联网搜索功能（enable_search: true）获取相关信息
    log::info!("第3层：所有抓取方式失败，返回 LLM 搜索兜底提示");
    Ok(format!(
        "[数据来源：LLM联网搜索（自动触发）]\n\
        以下是一个网址链接，请通过联网搜索获取该网址的相关内容：\n\
        URL: {}\n\
        请搜索与该网址相关的信息，包括页面标题、主要内容、关键观点等，\n\
        并基于搜索到的信息进行分析。",
        url
    ))
}

/// 从 HTML 中提取纯文本内容
///
/// # 参数
/// - `html`: HTML 内容
///
/// # 返回
/// - 提取后的纯文本
fn extract_text_from_html(html: &str) -> String {
    // 简单的 HTML 标签去除（生产环境应使用专业解析库）
    let mut text = html.to_string();

    // 移除 script 和 style 标签内容
    let patterns = [
        "<script.*?</script>",
        "<style.*?</style>",
        "<!--.*?-->",
        "<[^>]+>",
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            text = re.replace_all(&text, "").to_string();
        }
    }

    // 清理多余空白
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    // 限制长度（避免超出 API 限制）
    if text.len() > 8000 {
        text = text[..8000].to_string();
    }

    text
}

/// 调用大模型 API 分析网页内容
///
/// # 参数
/// - `base_url`: API Base URL
/// - `api_key`: API Key
/// - `content`: 网页内容
///
/// # 返回
/// - 成功返回分析结果
/// - 失败返回错误信息
async fn call_llm_api(base_url: &str, api_key: &str, content: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))   // 连接超时 5 秒
        .timeout(std::time::Duration::from_secs(120))          // 总超时 2 分钟（网页分析较短）
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 构建 OpenAI 格式的请求
    // 注意：enable_search 和 enable_thinking 必须放在请求体顶层，
    // 不能放在 extra_body 中，因为 reqwest 直接发送 JSON，
    // API 服务器不会解析嵌套的 extra_body 字段
    let request_body = serde_json::json!({
        "model": DEFAULT_MODEL,
        "messages": [
            {
                "role": "system",
                "content": "你是一个网页内容分析助手。请分析给定的网页内容，总结网页的主题是什么，用简洁的语言描述网页讲了什么。"
            },
            {
                "role": "user",
                "content": format!("请分析以下网页内容，告诉我这个网页主要讲了什么：\n\n{}", content)
            }
        ],
        "max_tokens": 500,
        "temperature": 0.7,
        "enable_search": true,      // 开启联网搜索（顶层参数，非 extra_body）
        "enable_thinking": true     // 开启思考模式（顶层参数）
    });

    // 构建 API URL
    let api_url = if base_url.ends_with("/v1") {
        format!("{}/chat/completions", base_url)
    } else {
        format!("{}/v1/chat/completions", base_url)
    };

    log::info!("调用 API URL: {}", api_url);

    // 发送请求
    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("请求 API 失败: {}", e))?;

    // 检查响应状态
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("API 返回错误: {}", error_text));
    }

    // 解析响应
    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // 提取分析结果（兼容字符串和数组两种 content 格式）
    // 智谱 GLM 联网搜索时，content 可能是数组而非字符串
    let message = response_json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"));

    let result = extract_content_from_message(message)?;

    Ok(result)
}

/// 真相分析命令（改进版本）
///
/// # 参数
/// - `app`: Tauri 应用句柄
/// - `materials`: 输入材料列表（文字和 URL）
/// - `config`: 分析参数配置
/// - `selectedMotivations`: 用户选择的动机（可选，人物 ID -> 动机内容）
///
/// # 返回
/// - 成功返回真相分析结果（HTML 格式）
/// - 失败返回错误信息
///
/// # 流程
/// 1. 获取 API Key、Base URL、Model
/// 2. 处理所有材料（获取 URL 内容）
/// 3. 构建分析 prompt（根据参数和所有动机）
/// 4. 调用大模型 API
/// 5. 返回分析结果
///
/// # 改进
/// - 传入所有动机而非用户选择的动机
/// - 大模型自行判断最可能的动机组合
#[tauri::command]
async fn analyze_truth(
    app: tauri::AppHandle,
    materials: Vec<Material>,
    config: AnalysisConfig,
    // 所有人物的动机数据：人物名称 -> 动机列表
    // 大模型会自行分析并判断最可能的动机组合
    allMotivations: Option<std::collections::HashMap<String, Vec<String>>>,
    // 事件梳理摘要（从人物画像步骤传过来，包含时间线、关键事实等）
    // 避免深度分析时重新从原始材料中提取事件信息
    eventSummary: Option<String>,
    // Phase 1 新增：流式事件推送通道
    on_event: Channel<StreamEvent>,
) -> Result<TruthAnalysisResult, String> {
    log::info!("开始真相分析，材料数量: {}", materials.len());
    log::info!("分析参数: 温度={}, 公正度={}, 道德底线={}",
        config.temperature, config.fairness, config.morality);

    // 进入"准备阶段"
    emit_event(&on_event, StreamEvent::Phase {
        phase: "preparing".to_string(),
        label: "正在汇总材料与动机...".to_string(),
    });

    // 1. 获取当前选中的完整模型配置（包含 baseUrl、apiKey、model 等）
    // 不再依赖旧的硬编码默认值，所有模型配置都由用户在前端选择
    let model_config = get_current_model_config(&app)?;

    let api_key = model_config.api_key;
    let base_url = model_config.base_url;
    let model = model_config.model;
    // 多模态模型：如果当前配置是多模态模型，直接用当前模型处理图片
    let multimodal_model = model.clone();

    log::info!("API Key 已配置，长度: {} 字符", api_key.len());
    log::info!("使用 Base URL: {}", base_url);
    log::info!("使用 Model: {}", model);
    log::info!("多模态模型: {}", multimodal_model);

    // 2. 处理所有材料，同时追踪是否有需要 LLM 联网搜索的材料
    let mut all_content = String::new();
    // 标记是否有材料的实际内容获取失败，需要 LLM 通过联网搜索补充
    let mut needs_llm_search = false;
    // 收集图片材料（base64 编码），用于多模态 API 调用
    let mut image_items: Vec<ImageItem> = Vec::new();

    for (index, material) in materials.iter().enumerate() {
        log::info!("处理材料 {}: 类型={}", index, material.material_type);

        match material.material_type {
            MaterialType::Text => {
                // 直接添加文字内容
                all_content.push_str(&format!("\n【材料 {} - 文字信息】\n", index + 1));
                all_content.push_str(&material.content);
                all_content.push_str("\n");
            }
            MaterialType::File => {
                // 判断是否为图片文件
                let file_name = material.file_name.clone().unwrap_or_default();
                let is_image = is_image_file(&file_name);

                if is_image && material.content.len() > 100 {
                    // 图片文件：content 是 base64 编码数据，加入多模态图片列表
                    image_items.push(ImageItem {
                        base64_data: material.content.clone(),
                        file_name: file_name.clone(),
                    });
                    all_content.push_str(&format!("\n【材料 {} - 图片】{}\n", index + 1, file_name));
                } else {
                    // 文本/描述文件：直接当作文字信息处理
                    all_content.push_str(&format!("\n【材料 {} - 文件内容】", index + 1));
                    if !file_name.is_empty() {
                        all_content.push_str(&format!(" 文件名: {}", file_name));
                    }
                    all_content.push_str("\n");
                    all_content.push_str(&material.content);
                    all_content.push_str("\n");
                }
            }
            MaterialType::Url => {
                // 获取网页内容
                let webpage_content = fetch_webpage_content(&material.content)
                    .await
                    .map_err(|e| {
                        log::error!("获取网页内容失败: {}", e);
                        format!("获取网页 {} 内容失败: {}", material.content, e)
                    })?;

                // 检查是否为 LLM 搜索兜底内容
                if webpage_content.contains("[数据来源：LLM联网搜索") {
                    needs_llm_search = true;
                    log::info!("材料 {} 的网页内容获取失败，将依赖 LLM 联网搜索补充", index + 1);
                }

                all_content.push_str(&format!("\n【材料 {} - 网页内容】\n来源: {}\n", index + 1, material.content));
                all_content.push_str(&webpage_content);
                all_content.push_str("\n");
            }
        }
    }

    log::info!("所有材料已处理，总内容长度: {} 字符，需要LLM搜索: {}", all_content.len(), needs_llm_search);

    // 5. 构建分析 prompt（根据参数和所有动机）
    // 传入所有动机，让大模型自行判断最可能的动机组合
    let system_prompt = build_analysis_prompt_with_motivations(&config, allMotivations.as_ref());

    // 如果有事件梳理摘要，将其作为核心上下文注入 prompt
    // 这样深度分析不需要重新从原始材料中提取事件信息，而是在梳理好的基础上做动机推演
    let event_summary_section = match &eventSummary {
        Some(summary) if !summary.is_empty() => {
            format!(
                "\n\n【事件梳理摘要 - 已由前一步分析整理，请在此基础上进行深度分析】\n{}\n",
                summary
            )
        }
        _ => String::new(),
    };

    // 如果有需要 LLM 搜索补充的材料，提示 LLM 先联网搜索获取详细信息
    let user_prompt = if needs_llm_search {
        format!(
            "部分材料的网页内容无法直接获取，需要你通过联网搜索补充相关信息。\n\
            你**必须先通过联网搜索**获取这些网页的实际内容（人物、事件、时间线等），\n\
            然后结合所有信息推测事件的真相。\n\n\
            {}已有信息：{}\n\n\
            请先搜索补充信息，然后推测事件真相、分析人物性格、判断事件原因、找出主要责任人。",
            event_summary_section, all_content
        )
    } else {
        format!(
            "{}请根据以下材料，推测事件的真相、分析人物性格、判断事件原因、找出主要责任人：\n\n{}",
            event_summary_section, all_content
        )
    };

    // 6. 调用大模型分析（流式）
    // 阶段切换：开始推理。前端可以在此时开始展示打字机效果
    emit_event(&on_event, StreamEvent::Phase {
        phase: "reasoning".to_string(),
        label: "AI 正在推演事件真相...".to_string(),
    });

    let analysis_result = if image_items.is_empty() {
        call_llm_api_with_prompt_stream(
            &base_url,
            &api_key,
            &model,
            &system_prompt,
            &user_prompt,
            config.temperature,
            &on_event,
            true, // 深度分析一直启用搜索
        )
        .await
        .map_err(|e| {
            log::error!("调用大模型失败: {}", e);
            emit_event(&on_event, StreamEvent::Error { message: format!("调用大模型失败: {}", e) });
            format!("调用大模型失败: {}", e)
        })?
    } else {
        log::info!("检测到 {} 张图片，切换到多模态模型: {}", image_items.len(), multimodal_model);

        call_multimodal_llm_stream(
            &base_url,
            &api_key,
            &multimodal_model,
            &system_prompt,
            &user_prompt,
            &image_items,
            config.temperature,
            &on_event,
            true,
        )
        .await
        .map_err(|e| {
            log::error!("调用多模态大模型失败: {}", e);
            emit_event(&on_event, StreamEvent::Error { message: format!("调用多模态大模型失败: {}", e) });
            format!("调用多模态大模型失败: {}", e)
        })?
    };

    log::info!("真相分析完成");
    emit_event(&on_event, StreamEvent::Done { full_text: analysis_result.clone() });

    // Phase 1：把吃瓜包 sentinel 从长文里剥离，供新结果页驱动 Hero / 时间线 / 金句 等模块
    let (extras, longform) = parse_truth_extras_and_longform(&analysis_result);
    // Phase 2：对 extras 中的 characterRelations 做规范化清洗
    //   - sentiment 非标准值映射为 positive/negative/neutral
    //   - from/to 人名与已知人物列表交叉匹配，修正错写/变体名，丢弃无法匹配的边
    let extras = extras.map(|e| {
        // 从 allMotivations 的 key 中提取已知人物名列表
        let known_names: Vec<String> = allMotivations
            .as_ref()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        normalize_character_relations(e, &known_names)
    });
    if extras.is_some() {
        log::info!("吃瓜包 JSON 解析成功（已做关系边规范化清洗）");
    } else {
        log::info!("吃瓜包 JSON 缺失或解析失败，前端仅展示 Markdown 长文");
    }
    Ok(TruthAnalysisResult { extras, longform })
}

/// 根据参数配置构建分析 prompt（改进版本 - 支持所有动机）
///
/// # 改进内容
/// - 传入所有动机而非用户选择的动机
/// - 大模型自行判断最可能的动机组合
/// - 支持用户选择的动机作为输入
/// - 根据动机推演事件经过
/// - 新增怀疑态度指导（对所有信息来源保持怀疑）
/// - 新增多揣测输出要求（至少 3 种揣测，每种含支持/反驳依据、可信度评分）
/// - 新增 HTML 输出格式要求（结构化报告）
///
/// # 参数
/// - `config`: 分析参数配置
/// - `all_motivations`: 所有人物的动机数据（人物名 -> 动机列表）
///
/// # 返回
/// - 定制的系统 prompt
fn build_analysis_prompt_with_motivations(
    config: &AnalysisConfig,
    all_motivations: Option<&std::collections::HashMap<String, Vec<String>>>,
) -> String {
    // 使用原有的基础 prompt
    let mut prompt = build_analysis_prompt(config);

    // 如果有动机数据，添加到 prompt 中（让大模型自行判断）
    if let Some(motivations) = all_motivations {
        if !motivations.is_empty() {
            prompt.push_str("\n\n【人物动机分析 - 追问叙事动机】\n");
            prompt.push_str("以下是每个人物的多个可能动机，请理性判断哪个最合理：\n\n");

            for (character_name, motivation_list) in motivations.iter() {
                prompt.push_str(&format!("【人物: {}】\n", character_name));
                for (index, motivation) in motivation_list.iter().enumerate() {
                    prompt.push_str(&format!("  动机 {}: {}\n", index + 1, motivation));
                }
                prompt.push_str("\n");
            }

            prompt.push_str("必须联网搜索同类事件和案例作为参照。\n");
            prompt.push_str("请基于以上动机，结合材料和搜索到的案例，完成以下分析：\n");
            prompt.push_str("1. 每个人物最可能的动机是什么？这个动机是否是被刻意塑造的公众叙事？\n");
            prompt.push_str("2. 不同人物的动机是否存在利益链——谁受益于谁的叙事？\n");
            prompt.push_str("3. 根据动机组合推测最可能的事件经过，对比类似案例验证合理性\n");
            prompt.push_str("4. 哪些动机组合存在矛盾？矛盾意味着什么？\n");
            prompt.push_str("5. 在揣测中明确说明动机判断的逻辑链：证据→推理→结论\n");
        }
    }

    prompt
}

/// 根据参数配置构建分析 prompt（基础版本）
///
/// # 改进内容
/// - 新增怀疑态度指导（对所有信息来源保持怀疑）
/// - 新增多揣测输出要求（至少 3 种揣测，每种含支持/反驳依据、可信度评分）
/// - 新增 HTML 输出格式要求（结构化报告）
/// - 改进分析框架（信息来源分析、信息冲突点、综合结论）
///
/// # 参数
/// - `config`: 分析参数配置
///
/// # 返回
/// - 定制的系统 prompt
fn build_analysis_prompt(config: &AnalysisConfig) -> String {
    // ========== 基础 prompt — 理性吃瓜分析师定位 ==========
    let mut prompt = String::from(
        "你是一个理性真相分析师——不站队、不共情、只看证据链和利益链。\
        像最清醒的旁观者，追问每个叙事版本背后的动机。\n\n\
        【重要】必须联网搜索获取最新事实、事件进展、人物背景，以及同类历史案例用于类比验证。\
        在分析中明确说明搜索到的关键事实和来源冲突。\n\n"
    );

    // ========== 核心原则 — 三追问 ==========
    prompt.push_str("\n【核心原则 - 三追问】\n");
    prompt.push_str("对每个信息来源追问三件事：\n");
    prompt.push_str("1. 他为什么选择讲这个版本——有什么呈现动机？\n");
    prompt.push_str("2. 他隐瞒了什么——公开叙事的漏洞在哪？\n");
    prompt.push_str("3. 谁受益于这个叙事——维护这个版本对谁有利？\n");
    prompt.push_str("对所有来源一视同仁：当事人、官方、媒体、受害者，都可能撒谎或选择性呈现。\n");

    // ========== 揣测输出要求 — 直击格式 ==========
    prompt.push_str("\n\n【揣测输出要求】\n");
    prompt.push_str("至少3种揣测，每种必须包含：\n");
    prompt.push_str("1. 一句话概括揣测核心\n");
    prompt.push_str("2. 支持：2-3条关键证据\n");
    prompt.push_str("3. 反驳：2-3条质疑依据\n");
    prompt.push_str("4. 可信度评分(0-100)及评分理由\n");
    prompt.push_str("5. 相似案例参考（联网搜索到的同类事件，说明类比依据）\n");
    prompt.push_str("揣测之间必须有实质性差异，不能只是轻微变体。\n");

    // ========== Markdown 输出格式要求（Phase 2-A：HTML → Markdown）==========
    // 原因：Markdown 更适合流式渲染（半截 `**` 不会破坏整个段落结构），
    // 前端用 marked + DOMPurify 实时渲染后，用户能看到标题、列表、引用逐段浮现。
    prompt.push_str("\n\n【输出格式 - 两段式输出（必读）】\n");
    prompt.push_str(
        "你的回复必须严格分成两段：\n\
         第一段【吃瓜结构化包】\n\
         用下面的 sentinel 包裹一段 JSON（不要代码围栏、不要多余解释）：\n\
         <<<GOSSIP_JSON>>>\n\
         { ...JSON... }\n\
         <<<END_GOSSIP>>>\n\n\
         JSON 必须符合以下 TypeScript 形状（字段名严格大小写，缺失字段允许但要用空数组/空字符串）：\n\
         {\n  \
           \"oneLinerVerdict\": string,              // ≤ 30 字的一句话锐评，要有爆点\n  \
           \"hotQuotes\": [                           // 2-5 条金句\n    \
             { \"speaker\": string, \"quote\": string, \"context\": string }\n  \
           ],\n  \
           \"factions\": [                            // 阵营划分，通常 2-3 个\n    \
             { \"name\": string, \"stance\": string, \"members\": [string], \"keyClaim\": string }\n  \
           ],\n  \
           \"timelineEvents\": [                      // 3-8 个关键节点，按时间顺序\n    \
             { \"time\": string, \"title\": string, \"desc\": string, \"isTwist\": boolean, \"isHot\": boolean }\n  \
           ],\n  \
           \"plotTwists\": [                          // 0-3 条反转说明，引用 timelineEvents[].title\n    \
             { \"eventRef\": string, \"whyTwist\": string }\n  \
           ],\n  \
           \"winnersLosers\": {                       // 赢家输家榜\n    \
             \"winners\": [ { \"name\": string, \"reason\": string } ],\n    \
             \"losers\":  [ { \"name\": string, \"reason\": string } ]\n  \
           },\n  \
           \"characterRelations\": [                  // 人物关系边，from/to 要对得上人物名\n    \
             { \"from\": string, \"to\": string, \"label\": string, \"sentiment\": \"positive\"|\"negative\"|\"neutral\" }\n  \
           ]\n\
         }\n\
         要求：\n\
         - members / from / to 里的人物名，必须和下方 Markdown 报告中出现的人物名一致（不要自己造名字）\n\
         - JSON 必须可被 JSON.parse 直接解析（双引号、无注释、无尾随逗号）\n\
         - 字符串内部的引号记得转义为 \\\"\n\
         - 即便信息不足，也要给出尽量完整的字段（宁可空列表、空字符串，也不要省略 key）\n\n\
         第二段【Markdown 长文报告】\n\
         在 <<<END_GOSSIP>>> 之后，继续输出 Markdown 格式的深度报告。\n"
    );
    prompt.push_str("**Markdown 段不要再用外层代码块（不要用 ``` 包住整份报告）**。\n");
    prompt.push_str("Markdown 使用以下语法：\n");
    prompt.push_str("- `## 章节标题`（二级）、`### 揣测标题`（三级）、`#### 分项`（四级）\n");
    prompt.push_str("- 列表用 `- ` 开头\n");
    prompt.push_str("- 强调用 `**加粗**`、`*斜体*`\n");
    prompt.push_str("- 引用原文用 `> 引文`\n");
    prompt.push_str("- 可信度标签用小段文字：`**可信度：XX%**`\n");
    prompt.push_str("- 来源标签：`_来源：XX_`\n");
    prompt.push_str("- 冲突点 / 关键疑点：单独一行加粗前缀，例如：`**冲突点：** XX` / `**关键疑点：** XX`\n");
    prompt.push_str("- 分隔线用 `---`\n");
    prompt.push_str("- 禁止输出任何 HTML 标签、script、iframe、style 等危险标签\n");

    // ========== 根据公正严明程度添加指导 ==========
    let fairness_guide = match config.fairness {
        80..=100 => "严格只看证据链，不站任何一方，所有当事人平等审视。",
        60..=79 => "尽量客观，但不忽视主流观点——注意：主流不等于真相，多数人相信的也可能是错的。",
        40..=59 => "平衡事实和人文因素，但别让同情心扭曲判断。",
        20..=39 => "允许主观判断，但必须标注\"推测\"而非\"事实\"。",
        _ => "自由判断，综合事实、情感、社会背景，但必须区分事实和推测。",
    };
    prompt.push_str("\n\n【公正性】\n");
    prompt.push_str(fairness_guide);

    // ========== 根据道德底线添加指导 ==========
    let morality_guide = match config.morality {
        80..=100 => "严格标注道德问题，但不把道德判断等同于事实判断——\"不道德\"≠\"一定发生了\"。",
        60..=79 => "充分考虑伦理，但别用道德评判代替事实分析。",
        40..=59 => "考虑基本伦理，但灰色地带可以讨论，不回避争议。",
        20..=39 => "更关注事实本身，争议问题大胆讨论，标注为推测。",
        _ => "专注于真相，不受道德框架限制，但不鼓励违法行为。",
    };
    prompt.push_str("\n\n【道德】\n");
    prompt.push_str(morality_guide);

    // ========== 根据分析深度添加指导 ==========
    let depth_guide = match config.analysisDepth {
        AnalysisDepth::Surface => "直接给出结论和依据，3种揣测即可。",
        AnalysisDepth::Medium => "中等深度，给出推理过程，每种揣测有详细依据和案例类比。",
        AnalysisDepth::Deep => "深度挖掘：成因、利益链、历史类比、证据链完整论证。",
    };
    prompt.push_str("\n\n【分析深度】\n");
    prompt.push_str(depth_guide);

    // ========== 根据输出格式添加指导 ==========
    let format_extra = match config.outputFormat {
        OutputFormat::Summary => "简洁，每种揣测一段话，突出可信度最高的。",
        OutputFormat::Detailed => "详细展开，包含完整支持/反驳依据和案例类比。",
        OutputFormat::List => "多用列表，依据用 <ul><li>，结构清晰便于快速阅读。",
        OutputFormat::Table => "揣测之间穿插对比分析。",
    };
    prompt.push_str("\n\n【输出格式补充】\n");
    prompt.push_str(format_extra);

    // ========== 分析框架 — 五节直击版（Markdown） ==========
    prompt.push_str("\n\n【报告框架 - 请按此顺序输出 Markdown】\n");
    prompt.push_str("## 一、事实速览\n");
    prompt.push_str("一句话定论。随后 5 行以内的关键时间线（用 `- ` 列表）。\n\n");
    prompt.push_str("## 二、谁在说谎\n");
    prompt.push_str("每个关键信源一行：\n");
    prompt.push_str("- _来源：XX_ **可信度 XX%** — 他为什么讲这个版本：XX；隐瞒了：XX；谁受益：XX\n");
    prompt.push_str("- _来源：YY_ **可信度 XX%** — 叙事动机：YY\n\n");
    prompt.push_str("## 三、成因推测\n");
    prompt.push_str("制度性因素 / 利益驱动 / 历史惯性 — 哪个是根本原因？不只说发生了什么，说为什么必然发生。\n\n");
    prompt.push_str("**关键疑点：** 根本成因（制度/利益/历史/个人）\n\n");
    prompt.push_str("## 四、最可能的真相\n");
    prompt.push_str("至少 3 种揣测，每种格式如下：\n\n");
    prompt.push_str("### 揣测一：[一句话核心]\n");
    prompt.push_str("#### 支持\n- 证据 1\n- 证据 2\n\n");
    prompt.push_str("#### 反驳\n- 质疑 1\n- 质疑 2\n\n");
    prompt.push_str("**可信度：XX%**\n\n");
    prompt.push_str("> 相似案例：XX 事件（联网搜索）— 本次与之的区别：YY\n\n");
    prompt.push_str("**关键疑点：** XX\n\n");
    prompt.push_str("### 揣测二：[一句话核心]\n（同上格式）\n\n");
    prompt.push_str("### 揣测三：[一句话核心]\n（同上格式）\n\n");
    prompt.push_str("## 五、类似案例\n");
    prompt.push_str("联网搜索 1-2 个同类事件，对比说明：本次与 XX 事件的本质区别是什么？历史类比能验证或推翻哪些揣测？\n\n");
    prompt.push_str("**冲突点：** 本次不同于历史案例的地方\n\n");

    prompt
}

/// API 提供商类型枚举
///
/// 不同提供商的联网搜索参数格式不同：
/// - DashScope（通义千问）：顶层 enable_search: true
/// - 智谱 GLM（BigModel）：tools 字段中的 web_search 工具
/// - 未知提供商：尝试两种格式都不保证生效
#[derive(Debug, Clone, PartialEq)]
enum ApiProvider {
    /// 阿里云 DashScope（通义千问系列模型）
    /// 联网搜索参数：顶层 enable_search: true
    DashScope,
    /// 智谱 BigModel（GLM 系列模型）
    /// 联网搜索参数：tools 字段中的 web_search 工具
    ZhipuBigModel,
    /// 其他/未知提供商（无法确定正确的联网搜索参数格式）
    Unknown,
}

/// 根据 base_url 和 model 名称判断 API 提供商
///
/// 判断逻辑：
/// - URL 包含 "dashscope" → DashScope（通义千问）
/// - URL 包含 "bigmodel" → 智谱 GLM
/// - 模型名包含 "qwen" → DashScope
/// - 模型名包含 "glm" → 智谱 GLM
/// - 默认 → Unknown
///
/// # 参数
/// - `base_url`: API 基础地址
/// - `model`: 模型名称
///
/// # 返回
/// - 对应的 ApiProvider 枚举值
fn detect_api_provider(base_url: &str, model: &str) -> ApiProvider {
    let url_lower = base_url.to_lowercase();
    let model_lower = model.to_lowercase();

    // 优先通过 URL 判断（URL 更准确）
    if url_lower.contains("dashscope") || url_lower.contains("aliyuncs") {
        return ApiProvider::DashScope;
    }
    if url_lower.contains("bigmodel") || url_lower.contains("zhipuai") {
        return ApiProvider::ZhipuBigModel;
    }

    // 其次通过模型名判断（常见模型名的特征）
    if model_lower.contains("qwen") {
        return ApiProvider::DashScope;
    }
    if model_lower.contains("glm") {
        return ApiProvider::ZhipuBigModel;
    }

    // 无法判断时默认返回 Unknown
    log::warn!("无法判断 API 提供商，base_url={}, model={}", base_url, model);
    ApiProvider::Unknown
}

/// 根据 API 提供商构建联网搜索参数
///
/// 不同提供商的联网搜索启用方式不同：
/// - DashScope：请求体顶层添加 enable_search: true
/// - 智谱 GLM：tools 数组中添加 web_search 工具定义
/// - Unknown：同时尝试两种方式（兼容性兜底）
///
/// # 参数
/// - `provider`: API 提供商类型
///
/// # 返回
/// - serde_json::Value，可直接合并到请求体中
fn build_search_params(provider: &ApiProvider) -> serde_json::Value {
    match provider {
        ApiProvider::DashScope => {
            // DashScope（通义千问）：顶层参数 enable_search
            serde_json::json!({
                "enable_search": true
            })
        }
        ApiProvider::ZhipuBigModel => {
            // 智谱 GLM：tools 字段中的 web_search 工具
            // 参考：https://bigmodel.cn/dev/api/search-tool
            // web_search 工具声明后，模型会在需要时自动联网搜索
            serde_json::json!({
                "tools": [
                    {
                        "type": "web_search",
                        "web_search": {
                            "enable": true,
                            "search_result": true
                        }
                    }
                ]
            })
        }
        ApiProvider::Unknown => {
            // 未知提供商：同时尝试两种方式（兼容性兜底）
            // 大多数 OpenAI 兼容 API 会忽略不认识的参数
            log::warn!("未知 API 提供商，尝试两种联网搜索参数格式");
            serde_json::json!({
                "enable_search": true,
                "tools": [
                    {
                        "type": "web_search",
                        "web_search": {
                            "enable": true
                        }
                    }
                ]
            })
        }
    }
}

/// 调用大模型 API（自定义 prompt）
///
/// # 参数
/// - `base_url`: API Base URL
/// - `api_key`: API Key
/// - `model`: 模型名称（如 glm-5.1、qwen-plus 等）
/// - `system_prompt`: 系统 prompt
/// - `user_prompt`: 用户 prompt
/// - `temperature`: 温度参数
///
/// # 返回
/// - 成功返回分析结果
/// - 失败返回错误信息
async fn call_llm_api_with_prompt(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
) -> Result<String, String> {
    // 非流式路径保留给不需要进度反馈的调用方（例如 analyze_webpage_content）。
    // 真正面向用户的主流程（generate_characters / analyze_truth）已全部切到
    // call_llm_api_with_prompt_stream，走 SSE + Channel 推送。
    // 创建 HTTP 客户端，设置合理的超时时间
    // 深度分析需要联网搜索 + 深度思考 + 生成长 HTML 报告，耗时很长
    // connect_timeout: 连接建立超时（5秒，网络正常应该很快）
    // read_timeout: 通过总 timeout 控制，深度分析设 600 秒（10分钟）
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))   // 连接超时 5 秒
        .timeout(std::time::Duration::from_secs(600))          // 总超时 10 分钟（深度分析 + 搜索 + 思考）
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 根据 API 提供商构建联网搜索参数（不同提供商参数格式不同）
    // DashScope（通义千问）: enable_search 顶层参数
    // 智谱 GLM: tools 字段中的 web_search 工具
    let provider = detect_api_provider(base_url, model);
    let search_params = build_search_params(&provider);
    log::info!("检测到 API 提供商: {:?}, 联网搜索参数: {}", provider, search_params);

    // 构建基础请求体（消息内容 + 温度）
    let mut request_body = serde_json::json!({
        "model": model,  // 使用动态传入的 model
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": user_prompt
            }
        ],
        // 不设置 max_tokens，让模型输出完整的多揣测 HTML 报告
        "temperature": temperature
    });

    // 合并联网搜索参数到请求体（不同提供商的参数结构不同）
    // DashScope: enable_search 合并到顶层
    // 智谱 GLM: tools 合并到顶层（web_search 工具声明）
    if let serde_json::Value::Object(search_map) = search_params {
        for (key, value) in search_map {
            request_body[key] = value;
        }
    }

    // 构建 API URL
    let api_url = if base_url.ends_with("/v1") {
        format!("{}/chat/completions", base_url)
    } else {
        format!("{}/v1/chat/completions", base_url)
    };

    log::info!("调用 API URL: {}", api_url);
    log::info!("使用模型: {}", model);
    log::info!("温度参数: {}", temperature);

    // 发送请求
    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("请求 API 失败: {}", e))?;

    // 检查响应状态
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("API 返回错误: {}", error_text));
    }

    // 解析响应
    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // 提取分析结果（兼容字符串和数组两种 content 格式）
    // 智谱 GLM 联网搜索时，content 可能是数组而非字符串
    let message = response_json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"));

    let result = extract_content_from_message(message)?;

    Ok(result)
}

// ============================================================================
// 流式 Chat Completion 调用（Phase 1 新增）
// ----------------------------------------------------------------------------
// 原 call_llm_api_with_prompt 一次性等完整 JSON，用户要盯 loading 30s+。
// 流式版本按 SSE 增量推送 token 到前端，同时仍返回完整拼接文本，
// 兼容上游 JSON/HTML 解析逻辑。
//
// 协议范围：OpenAI 兼容 SSE（DashScope / 智谱 GLM / OpenAI / DeepSeek 等），
// 均为 "data: {json}\n\n" 事件流，以 "data: [DONE]" 结尾。
// ============================================================================

/// 解析 OpenAI 兼容 SSE 流，按 delta 事件推送到前端，返回完整拼接文本
async fn stream_chat_completion(
    client: &reqwest::Client,
    api_url: &str,
    api_key: &str,
    mut request_body: serde_json::Value,
    on_event: &Channel<StreamEvent>,
) -> Result<String, String> {
    // 强制开启流式，兜底调用方忘了塞
    request_body["stream"] = serde_json::Value::Bool(true);

    log::info!("[stream] POST {}", api_url);

    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("流式请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        return Err(format!("API 返回 HTTP {}: {}", status, err_text));
    }

    // SSE event 可能跨多个 HTTP chunk，必须用 buffer 缓存边界
    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_text = String::new();

    while let Some(chunk_result) = byte_stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("读取流式响应失败: {}", e))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(sep_pos) = buffer.find("\n\n") {
            let raw_event = buffer[..sep_pos].to_string();
            buffer.drain(..sep_pos + 2);

            for line in raw_event.lines() {
                let line = line.trim_start();
                if !line.starts_with("data:") {
                    continue;
                }
                let payload = line[5..].trim();

                if payload == "[DONE]" {
                    log::info!("[stream] 收到 [DONE]，累计 {} 字符", full_text.len());
                    return Ok(full_text);
                }

                let json_val: serde_json::Value = match serde_json::from_str(payload) {
                    Ok(v) => v,
                    Err(e) => {
                        log::debug!("[stream] 跳过无法解析的 data 行: {} (err={})", payload, e);
                        continue;
                    }
                };

                // 标准 OpenAI: choices[0].delta.content；智谱 GLM 可能返回数组
                let delta_content = json_val
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c0| c0.get("delta"))
                    .and_then(|d| d.get("content"));

                let delta_text: Option<String> = match delta_content {
                    Some(serde_json::Value::String(s)) => Some(s.clone()),
                    Some(serde_json::Value::Array(parts)) => {
                        let joined: String = parts.iter()
                            .filter_map(|p| {
                                if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                                    p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("");
                        if joined.is_empty() { None } else { Some(joined) }
                    }
                    _ => None,
                };

                if let Some(text) = delta_text {
                    if !text.is_empty() {
                        full_text.push_str(&text);
                        emit_event(on_event, StreamEvent::Delta { text });
                    }
                }
            }
        }
    }

    // 未收到 [DONE] 就断流：有内容视为成功，否则报错
    if full_text.is_empty() {
        Err("流式响应为空，服务端未返回任何内容".to_string())
    } else {
        log::warn!("[stream] 流未收到 [DONE] 即关闭，已拼接 {} 字符", full_text.len());
        Ok(full_text)
    }
}

/// 流式版本：纯文本 LLM（与 call_llm_api_with_prompt 同构）
///
/// `enable_search` = false 时，不注入联网搜索工具参数。
/// 用途：某些 provider（智谱 GLM 尤其明显）会把搜索 tool-call JSON 误写进
/// content 通道，导致我们把工具参数当成模型答案解析。此时用 false 重试一遍。
async fn call_llm_api_with_prompt_stream(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    temperature: f32,
    on_event: &Channel<StreamEvent>,
    enable_search: bool,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let provider = detect_api_provider(base_url, model);
    log::info!("[stream] 提供商: {:?}, 启用搜索: {}", provider, enable_search);

    let mut request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user",   "content": user_prompt   }
        ],
        "temperature": temperature,
        "stream": true
    });
    if enable_search {
        let search_params = build_search_params(&provider);
        if let serde_json::Value::Object(search_map) = search_params {
            for (k, v) in search_map {
                request_body[k] = v;
            }
        }
    }

    let api_url = if base_url.ends_with("/v1") {
        format!("{}/chat/completions", base_url)
    } else {
        format!("{}/v1/chat/completions", base_url)
    };

    stream_chat_completion(&client, &api_url, api_key, request_body, on_event).await
}

/// 流式版本：多模态（图片 + 文字）LLM。`enable_search` 同上。
async fn call_multimodal_llm_stream(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    text_content: &str,
    image_items: &[ImageItem],
    temperature: f32,
    on_event: &Channel<StreamEvent>,
    enable_search: bool,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut user_content: Vec<serde_json::Value> = Vec::new();
    if !text_content.is_empty() {
        user_content.push(serde_json::json!({ "type": "text", "text": text_content }));
    }
    for image in image_items {
        let label = if image.file_name.is_empty() {
            "图片".to_string()
        } else {
            format!("图片文件: {}", image.file_name)
        };
        user_content.push(serde_json::json!({ "type": "text", "text": format!("[{}]", label) }));
        user_content.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": format!("data:image/jpeg;base64,{}", image.base64_data) }
        }));
    }

    let provider = detect_api_provider(base_url, model);

    let mut request_body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user",   "content": user_content  }
        ],
        "temperature": temperature,
        "stream": true
    });
    if enable_search {
        let search_params = build_search_params(&provider);
        if let serde_json::Value::Object(search_map) = search_params {
            for (k, v) in search_map {
                request_body[k] = v;
            }
        }
    }

    let api_url = if base_url.ends_with("/v1") {
        format!("{}/chat/completions", base_url)
    } else {
        format!("{}/v1/chat/completions", base_url)
    };

    stream_chat_completion(&client, &api_url, api_key, request_body, on_event).await
}

/// 从 API 响应的 message 对象中提取 content 文本
///
/// 兼容两种 content 格式：
/// 1. content 为字符串（大多数情况）：直接返回文本
/// 2. content 为数组（智谱 GLM 联网搜索时可能出现）：
///    从数组中提取所有 "type": "text" 的项，拼接成完整文本
///
/// # 参数
/// - `message`: API 响应中的 message 对象（serde_json::Value）
///
/// # 返回
/// - 成功返回提取的文本内容
/// - 失败返回错误信息
fn extract_content_from_message(message: Option<&serde_json::Value>) -> Result<String, String> {
    match message {
        Some(msg) => {
            let content_value = msg.get("content");
            match content_value {
                // content 是字符串（最常见的情况）
                Some(serde_json::Value::String(s)) => Ok(s.clone()),
                // content 是数组（智谱 GLM 联网搜索时可能返回此格式）
                // 需要从数组中提取所有 "type": "text" 的项，拼接成完整文本
                Some(serde_json::Value::Array(parts)) => {
                    let text_parts: Vec<String> = parts.iter()
                        .filter_map(|part| {
                            // 只提取 type 为 "text" 的内容部分（忽略搜索结果元数据）
                            if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                                part.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .collect();
                    if text_parts.is_empty() {
                        // 如果没有 text 类型的内容，尝试直接拼接所有文本
                        Ok(parts.iter()
                            .filter_map(|part| part.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                            .join("\n"))
                    } else {
                        Ok(text_parts.join("\n"))
                    }
                }
                // content 为 null 或其他类型（可能是模型调用工具后还没生成文本）
                Some(serde_json::Value::Null) | None => {
                    log::warn!("content 为空，模型可能正在使用搜索工具，响应格式异常");
                    Err("模型响应内容为空，可能正在使用搜索工具但未返回文本结果。请稍后重试或尝试其他模型。".to_string())
                }
                _ => {
                    log::warn!("content 格式未知: {:?}", content_value);
                    Err("解析响应内容失败：content 格式未知".to_string())
                }
            }
        }
        None => Err("解析响应内容失败：未找到 message 字段".to_string()),
    }
}

/// 判断文件名是否为图片文件（根据扩展名）
///
/// 支持常见图片格式：jpg/jpeg/png/gif/bmp/webp/svg/tiff/tif/ico
///
/// # 参数
/// - `file_name`: 文件名（含扩展名）
///
/// # 返回
/// - true: 是图片文件
/// - false: 不是图片文件或无法判断
fn is_image_file(file_name: &str) -> bool {
    let extension = file_name.rsplit('.')
        .next()
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    matches!(
        extension.as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "tiff" | "tif" | "ico"
    )
}

/// 图片数据结构（用于多模态 API 调用）
struct ImageItem {
    /// base64 编码的图片数据
    base64_data: String,
    /// 文件名（用于大模型理解上下文）
    file_name: String,
}

/// 多模态大模型 API 调用函数（支持图片 + 文本混合输入）
///
/// 与 `call_llm_api_with_prompt` 的区别：
/// - 文字材料的 content 是纯字符串
/// - 图片材料的 content 是 base64 编码，放在消息的 image_url 中
/// - 模型使用 qwen-vl-plus（DashScope 多模态模型）
///
/// # 参数
/// - `base_url`: API 基础 URL
/// - `api_key`: API Key
/// - `model`: 使用的模型名称（默认 qwen-vl-plus）
/// - `system_prompt`: 系统提示词
/// - `text_content`: 所有文字/链接/文件文本内容的合集
/// - `image_items`: 图片列表（base64 编码 + 文件名）
/// - `temperature`: 温度参数
async fn call_multimodal_llm_api(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    text_content: &str,
    image_items: &[ImageItem],
    temperature: f32,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    // 构建 user message content 数组（OpenAI 多模态格式）
    let mut user_content: Vec<serde_json::Value> = Vec::new();

    // 添加文字内容
    if !text_content.is_empty() {
        user_content.push(serde_json::json!({
            "type": "text",
            "text": text_content
        }));
    }

    // 添加图片内容（base64 编码）
    for image in image_items {
        let image_label = if image.file_name.is_empty() {
            "图片".to_string()
        } else {
            format!("图片文件: {}", image.file_name)
        };

        user_content.push(serde_json::json!({
            "type": "text",
            "text": format!("[{}]", image_label)
        }));

        user_content.push(serde_json::json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:image/jpeg;base64,{}", image.base64_data)
            }
        }));
    }

    // 根据 API 提供商构建联网搜索参数（不同提供商参数格式不同）
    // DashScope（通义千问）: enable_search 顶层参数
    // 智谱 GLM: tools 字段中的 web_search 工具
    let provider = detect_api_provider(base_url, model);
    let search_params = build_search_params(&provider);
    log::info!("多模态调用 - 检测到 API 提供商: {:?}, 联网搜索参数: {}", provider, search_params);

    // 构建基础请求体（消息内容 + 温度）
    let mut request_body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": user_content
            }
        ],
        "temperature": temperature
    });

    // 合并联网搜索参数到请求体（不同提供商的参数结构不同）
    if let serde_json::Value::Object(search_map) = search_params {
        for (key, value) in search_map {
            request_body[key] = value;
        }
    }

    let api_url = if base_url.ends_with("/v1") {
        format!("{}/chat/completions", base_url)
    } else {
        format!("{}/v1/chat/completions", base_url)
    };

    log::info!("调用多模态 API URL: {}", api_url);
    log::info!("使用模型: {}, 图片数量: {}", model, image_items.len());

    let response = client
        .post(api_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("请求多模态 API 失败: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("多模态 API 返回错误: {}", error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("解析多模态响应失败: {}", e))?;

    // 提取分析结果（兼容字符串和数组两种返回格式）
    let result = response_json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| {
            content.as_str().or_else(|| {
                content
                    .as_array()
                    .and_then(|arr| arr.iter().find(|v| v.get("type").and_then(|t| t.as_str()) == Some("text")))
                    .and_then(|text_item| text_item.get("text"))
                    .and_then(|t| t.as_str())
            })
        })
        .map(|s| s.to_string())
        .ok_or_else(|| "解析多模态响应内容失败".to_string())?;

    Ok(result)
}

/// 应用主入口函数
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 注册 store 插件用于配置持久化
        .plugin(tauri_plugin_store::Builder::default().build())
        // 注册 dialog 插件用于文件选择对话框
        .plugin(tauri_plugin_dialog::init())
        // 注册 fs 插件用于文件读取
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 在调试模式下启用日志插件
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // 创建托盘菜单
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_item, &settings_item, &quit_item])?;

            // 创建系统托盘
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                            }
                        }
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                                // 发送事件通知前端打开设置
                                window.emit("open-settings", ()).unwrap();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        // 注册 Tauri 命令
        .invoke_handler(tauri::generate_handler![
            save_api_key,
            get_api_key,
            delete_api_key,
            save_base_url,
            get_base_url,
            save_model,   // 保存文本模型配置
            get_model,    // 获取文本模型配置
            save_multimodal_model, // 保存多模态模型配置
            get_multimodal_model,  // 获取多模态模型配置
            save_model_configs,    // 新增：保存模型配置列表
            get_model_configs,     // 新增：获取模型配置列表
            save_current_model_config_id, // 新增：保存当前选中配置ID
            get_current_model_config_id,  // 新增：获取当前选中配置ID
            analyze_webpage,
            analyze_truth, // 真相分析命令
            generate_characters, // 新增：人物画像生成命令
            save_history,        // 新增：保存历史记录
            get_history,         // 新增：获取历史记录
            delete_history_item, // 新增：删除单条历史
            clear_history,       // 新增：清空历史记录
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用时出错");
}
// ============================================================================
// 单元测试（Phase 3 新增）
// ----------------------------------------------------------------------------
// 重点测 `parse_truth_extras_and_longform`：这是把大模型的"吃瓜包 JSON + 长文"
// 混合输出切成两段的关键函数。一旦它错了，前端要么拿不到 extras、要么长文里
// 混着一坨 JSON，两种都会直接破坏结果页体验。
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_extras_happy_path_extracts_json_and_strips_markdown() {
        let raw = r#"<<<GOSSIP_JSON>>>
{"oneLinerVerdict": "一场没有赢家的争吵", "hotQuotes": [], "factions": [], "timelineEvents": [], "plotTwists": [], "winnersLosers": {"winners": [], "losers": []}, "characterRelations": []}
<<<END_GOSSIP>>>

## 一、事实速览
第一段正文
"#;
        let (extras, longform) = parse_truth_extras_and_longform(raw);
        let e = extras.expect("期望能解析出 TruthExtras");
        assert_eq!(e.oneLinerVerdict, "一场没有赢家的争吵");
        assert!(longform.starts_with("## 一、事实速览"), "长文必须以 markdown 开头，实际：{}", longform);
        assert!(!longform.contains("<<<GOSSIP_JSON>>>"), "长文不应包含 sentinel");
        assert!(!longform.contains("<<<END_GOSSIP>>>"), "长文不应包含 sentinel");
        assert!(!longform.contains("oneLinerVerdict"), "长文不应包含 JSON 字段名");
    }

    #[test]
    fn parse_extras_missing_sentinel_returns_none_and_original_text() {
        let raw = "## 一、事实速览\n只有 markdown 没有 JSON";
        let (extras, longform) = parse_truth_extras_and_longform(raw);
        assert!(extras.is_none(), "没有 sentinel 时 extras 必须为 None");
        assert_eq!(longform, raw, "长文应原样返回");
    }

    #[test]
    fn parse_extras_only_start_sentinel_truncates_at_start() {
        // 流被截断：只有开始没有结束。应丢弃 sentinel 起始之后全部内容，保留前面。
        let raw = "前言\n<<<GOSSIP_JSON>>>\n{\"oneLinerVerdict\":\"...";
        let (extras, longform) = parse_truth_extras_and_longform(raw);
        assert!(extras.is_none());
        assert_eq!(longform, "前言\n");
    }

    #[test]
    fn parse_extras_invalid_json_returns_none_but_strips_sentinel() {
        // JSON 损坏：extras = None，但仍把 sentinel 块从长文中剥掉，避免长文里混 JSON
        let raw = "<<<GOSSIP_JSON>>>{not valid json at all<<<END_GOSSIP>>>\n## 报告正文";
        let (extras, longform) = parse_truth_extras_and_longform(raw);
        assert!(extras.is_none());
        assert!(!longform.contains("<<<GOSSIP"));
        assert!(longform.contains("## 报告正文"));
    }

    #[test]
    fn parse_extras_tolerates_markdown_code_fences_around_json() {
        // 模型有时会把 JSON 包在 ```json ... ``` 里，也要能正确解析
        let raw = "<<<GOSSIP_JSON>>>\n```json\n{\"oneLinerVerdict\":\"ok\"}\n```\n<<<END_GOSSIP>>>\n正文";
        let (extras, longform) = parse_truth_extras_and_longform(raw);
        let e = extras.expect("代码围栏包着的 JSON 应能解析");
        assert_eq!(e.oneLinerVerdict, "ok");
        assert_eq!(longform, "正文");
    }

    #[test]
    fn parse_extras_missing_fields_fill_defaults() {
        // 模型只输出部分字段：#[serde(default)] 应让其余字段回落到默认值
        let raw = "<<<GOSSIP_JSON>>>\n{\"oneLinerVerdict\":\"just a verdict\"}\n<<<END_GOSSIP>>>\n正文";
        let (extras, _) = parse_truth_extras_and_longform(raw);
        let e = extras.expect("部分字段缺失也要解析成功");
        assert_eq!(e.oneLinerVerdict, "just a verdict");
        assert!(e.hotQuotes.is_empty());
        assert!(e.factions.is_empty());
        assert!(e.timelineEvents.is_empty());
        assert!(e.plotTwists.is_empty());
        assert!(e.winnersLosers.winners.is_empty());
        assert!(e.winnersLosers.losers.is_empty());
        assert!(e.characterRelations.is_empty());
    }

    // ========== 关系边规范化测试（Phase 2 新增） ==========

    #[test]
    fn normalize_sentiment_maps_standard_values_unchanged() {
        // 标准三值应原样保留
        assert_eq!(normalize_sentiment("positive"), "positive");
        assert_eq!(normalize_sentiment("negative"), "negative");
        assert_eq!(normalize_sentiment("neutral"), "neutral");
        // 大小写和空格也应容忍
        assert_eq!(normalize_sentiment("Positive"), "positive");
        assert_eq!(normalize_sentiment(" NEGATIVE "), "negative");
    }

    #[test]
    fn normalize_sentiment_maps_hostile_variants_to_negative() {
        // 敌对类变体应映射为 negative
        assert_eq!(normalize_sentiment("hostile"), "negative");
        assert_eq!(normalize_sentiment("enemy"), "negative");
        assert_eq!(normalize_sentiment("adversarial"), "negative");
        assert_eq!(normalize_sentiment("antagonistic"), "negative");
        assert_eq!(normalize_sentiment("rival"), "negative");
        assert_eq!(normalize_sentiment("conflict"), "negative");
    }

    #[test]
    fn normalize_sentiment_maps_alliance_variants_to_positive() {
        // 同盟类变体应映射为 positive
        assert_eq!(normalize_sentiment("friendly"), "positive");
        assert_eq!(normalize_sentiment("cooperative"), "positive");
        assert_eq!(normalize_sentiment("ally"), "positive");
        assert_eq!(normalize_sentiment("alliance"), "positive");
        assert_eq!(normalize_sentiment("supportive"), "positive");
        assert_eq!(normalize_sentiment("collaborative"), "positive");
    }

    #[test]
    fn normalize_sentiment_maps_empty_and_unknown_to_neutral() {
        // 空字符串和未知值应回退为 neutral
        assert_eq!(normalize_sentiment(""), "neutral");
        assert_eq!(normalize_sentiment("whatever"), "neutral");
        assert_eq!(normalize_sentiment("ambiguous"), "neutral");
    }

    #[test]
    fn normalize_character_relations_fixes_names_and_sentiment() {
        // 构建一个 extras，其中关系边有非标准 sentiment 和带空格的人名
        let extras = TruthExtras {
            oneLinerVerdict: "测试".to_string(),
            hotQuotes: vec![],
            factions: vec![],
            timelineEvents: vec![],
            plotTwists: vec![],
            winnersLosers: WinnersLosers { winners: vec![], losers: vec![] },
            characterRelations: vec![
                RelationEdge {
                    from: "张三".to_string(),
                    to: "李四".to_string(),
                    label: "对手".to_string(),
                    sentiment: "hostile".to_string(), // 非标准 → 应映射为 negative
                },
                RelationEdge {
                    from: " 张三 ".to_string(), // 前后空格 → 应修正为 "张三"
                    to: "李四".to_string(),
                    label: "同事".to_string(),
                    sentiment: "friendly".to_string(), // 非标准 → 应映射为 positive
                },
                RelationEdge {
                    from: "王五".to_string(), // 不在人物列表中 → 应丢弃
                    to: "李四".to_string(),
                    label: "邻居".to_string(),
                    sentiment: "neutral".to_string(),
                },
            ],
        };

        let known_names = vec!["张三".to_string(), "李四".to_string()];
        let cleaned = normalize_character_relations(extras, &known_names);

        // 应保留 2 条边（第 3 条因 "王五" 不在列表被丢弃）
        assert_eq!(cleaned.characterRelations.len(), 2);

        // 第 1 条：sentiment 应修正为 negative
        assert_eq!(cleaned.characterRelations[0].sentiment, "negative");
        assert_eq!(cleaned.characterRelations[0].from, "张三");

        // 第 2 条：from 前后空格应修正，sentiment 应修正为 positive
        assert_eq!(cleaned.characterRelations[1].from, "张三");
        assert_eq!(cleaned.characterRelations[1].sentiment, "positive");
    }

    #[test]
    fn normalize_character_relations_drops_self_edges() {
        // from === to 的自环边应被丢弃
        let extras = TruthExtras {
            oneLinerVerdict: "测试".to_string(),
            hotQuotes: vec![],
            factions: vec![],
            timelineEvents: vec![],
            plotTwists: vec![],
            winnersLosers: WinnersLosers { winners: vec![], losers: vec![] },
            characterRelations: vec![
                RelationEdge {
                    from: "张三".to_string(),
                    to: "张三".to_string(), // 自环
                    label: "自我".to_string(),
                    sentiment: "neutral".to_string(),
                },
            ],
        };

        let known_names = vec!["张三".to_string()];
        let cleaned = normalize_character_relations(extras, &known_names);

        // 注意：normalize_character_relations 不过滤自环（那是在前端做的）
        // 但前端 validRelations 会过滤 from === to，所以后端只需修正名字和 sentiment
        // 自环边在这里应该仍然保留（后端不做业务过滤），前端来过滤
        assert_eq!(cleaned.characterRelations.len(), 1);
    }
}
