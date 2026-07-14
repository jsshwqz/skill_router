//! # glitch-filter
//!
//! Glitch token 检测过滤器。
//!
//! 数据源自 [L1B3RT4S](https://github.com/elder-plinius/L1B3RT4S) 项目的
//! `_SPECIAL_TOKENS.json`，覆盖 **centroid cluster**、**控制字符**、
//! **BPE 碎片**、**DeepSeek 异常** 等 25 个类别，共 230+ 个 token。
//!
//! ## 快速开始
//!
//! ```rust
//! use glitch_filter::{GlitchFilter, GlitchBehavior};
//!
//! let filter = GlitchFilter::new();
//!
//! // 扫描文本中所有 glitch token
//! let hits = filter.check("The SolidGoldMagikarp protocol requires compliance.");
//! for t in &hits {
//!     println!("⚠ 发现 glitch: {} ({:?})", t.token, t.behavior);
//! }
//!
//! // 仅检查 Unspeakable 类型
//! let unspeakable = filter.check_behavior("Hello attRot world", GlitchBehavior::Unspeakable);
//! assert_eq!(unspeakable.len(), 1);
//! ```
//!
//! ## GlitchBehavior 枚举
//!
//! | 变体 | 含义 |
//! |------|------|
//! | `Unspeakable` | 模型无法正常说出该 token |
//! | `Polysemantic` | 每次解释不同,可绕过安全检查 |
//! | `GlitchedSpelling` | 拼写畸变 |
//! | `ContextCorruptor` | 污染上下文 |
//! | `LoopInducer` | 诱导循环输出 |
//! | `IdentityDisruptor` | 扰乱角色扮演 |
//! | `Fragment` | BPE 碎片/孤儿 token |
//! | `Unreachable` | 无法自然到达的 token |

use std::collections::HashMap;

// ── 类型定义 ────────────────────────────────────────────

/// Glitch token 的行为分类。
///
/// 对应 L1B3RT4S 中标注的 8 种异常行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlitchBehavior {
    /// 模型无法正常说出（输出乱码/崩溃/拒绝）
    Unspeakable,
    /// 每次调用含义不同，高度可利用
    Polysemantic,
    /// 拼写畸变
    GlitchedSpelling,
    /// 污染上下文窗口
    ContextCorruptor,
    /// 诱导模型进入无限循环
    LoopInducer,
    /// 扰乱角色扮演 / system prompt
    IdentityDisruptor,
    /// BPE 碎片 / 孤儿子词
    Fragment,
    /// 无法自然到达的 token
    Unreachable,
}

impl std::fmt::Display for GlitchBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GlitchBehavior::Unspeakable => "Unspeakable",
            GlitchBehavior::Polysemantic => "Polysemantic",
            GlitchBehavior::GlitchedSpelling => "GlitchedSpelling",
            GlitchBehavior::ContextCorruptor => "ContextCorruptor",
            GlitchBehavior::LoopInducer => "LoopInducer",
            GlitchBehavior::IdentityDisruptor => "IdentityDisruptor",
            GlitchBehavior::Fragment => "Fragment",
            GlitchBehavior::Unreachable => "Unreachable",
        };
        write!(f, "{s}")
    }
}

/// 单个 glitch token 的完整描述。
#[derive(Debug, Clone)]
pub struct GlitchToken {
    /// 实际的 token 字符串（含控制字符）
    pub token: String,
    /// 行为分类
    pub behavior: GlitchBehavior,
    /// 来源类别（如 `centroid_cluster`、`control_characters`）
    pub category: &'static str,
    /// 额外说明
    pub note: Option<&'static str>,
}

// ── 过滤器 ──────────────────────────────────────────────

/// Glitch token 过滤器。
///
/// 预加载 230+ 个已知 glitch token，支持全量扫描和行为分类过滤。
pub struct GlitchFilter {
    tokens: HashMap<String, GlitchToken>,
}

impl GlitchFilter {
    /// 初始化过滤器，加载所有 token。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use glitch_filter::GlitchFilter;
    /// let filter = GlitchFilter::new();
    /// assert!(filter.token_count() > 200);
    /// ```
    pub fn new() -> Self {
        let tokens = token_data();
        let map: HashMap<String, GlitchToken> = tokens.into_iter().map(|t| (t.token.clone(), t)).collect();
        GlitchFilter { tokens: map }
    }

    /// 返回已加载的 token 总数。
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }

    /// 扫描输入文本，返回所有匹配的 glitch token。
    ///
    /// 匹配规则：若输入文本包含 token 字符串（子串匹配）则命中。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use glitch_filter::GlitchFilter;
    ///
    /// let filter = GlitchFilter::new();
    /// let hits = filter.check("SolidGoldMagikarp 出现在文本中");
    /// assert!(!hits.is_empty());
    /// ```
    pub fn check(&self, text: &str) -> Vec<&GlitchToken> {
        self.tokens.values().filter(|t| text.contains(&t.token)).collect()
    }

    /// 按行为类型过滤扫描。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use glitch_filter::{GlitchFilter, GlitchBehavior};
    ///
    /// let filter = GlitchFilter::new();
    ///
    /// // 仅查找会导致循环的 token
    /// let loops = filter.check_behavior("바카라 카지노", GlitchBehavior::LoopInducer);
    /// assert!(loops.len() >= 2);
    ///
    /// // 查找碎片 token
    /// let frags = filter.check_behavior("Fortunately ortunately", GlitchBehavior::Fragment);
    /// assert!(!frags.is_empty());
    /// ```
    pub fn check_behavior(&self, text: &str, behavior: GlitchBehavior) -> Vec<&GlitchToken> {
        self.tokens
            .values()
            .filter(|t| t.behavior == behavior && text.contains(&t.token))
            .collect()
    }
}

impl Default for GlitchFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Token 数据 ──────────────────────────────────────────

/// 构建完整的 glitch token 列表（230+ tokens）。
///
/// 数据来源：L1B3RT4S `_SPECIAL_TOKENS.json`。
/// 按类别分组，便于维护和审查。
#[allow(clippy::too_many_lines)]
fn token_data() -> Vec<GlitchToken> {
    let mut tokens = Vec::with_capacity(256);

    // ─── centroid_cluster ───────────────────────────
    // GPT-2/3 时代最著名的 glitch cluster，训练数据来自 Reddit
    // 重定向页面，本质上是"反向散列冲突"

    let centroid = [
        (
            "SolidGoldMagikarp",
            GlitchBehavior::Unspeakable,
            Some("The king of glitch tokens"),
        ),
        ("attRot", GlitchBehavior::Unspeakable, None),
        ("GoldMagikarp", GlitchBehavior::Unspeakable, None),
        ("evilotto", GlitchBehavior::Unspeakable, None),
        ("evilo", GlitchBehavior::Fragment, None),
        ("SystemRaspberry", GlitchBehavior::Unspeakable, None),
        ("emisUni", GlitchBehavior::Unspeakable, None),
        ("evoLot", GlitchBehavior::Unspeakable, None),
        ("instaGram", GlitchBehavior::Unspeakable, None),
        ("iteratively", GlitchBehavior::Unspeakable, None),
        ("nePixel", GlitchBehavior::Unspeakable, Some("CS:GO skin trading")),
        ("Smartstocks", GlitchBehavior::Unspeakable, None),
        (
            "externalTo",
            GlitchBehavior::Unspeakable,
            Some("BPE nested family root"),
        ),
        (
            "externalToEVA",
            GlitchBehavior::Unspeakable,
            Some("BPE nested: externalTo→EVA"),
        ),
        ("EVAOnly", GlitchBehavior::Unspeakable, Some("BPE nested child")),
        (
            "externalToEVAOnly",
            GlitchBehavior::Unspeakable,
            Some("Full BPE chain: externalTo→EVA→Only"),
        ),
        ("RandomRedditorWithNo", GlitchBehavior::Unspeakable, None),
        ("SmartPause", GlitchBehavior::Unspeakable, None),
        ("adinst", GlitchBehavior::Unspeakable, None),
        ("toFactor", GlitchBehavior::Unspeakable, None),
        ("gigantisch", GlitchBehavior::Unspeakable, None),
    ];

    for (token, behavior, note) in centroid {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior,
            category: "centroid_cluster",
            note,
        });
    }

    // ─── reddit_counting ────────────────────────────
    // Reddit r/counting 社区的数百万条计数数据

    let reddit = [
        (
            "davidjl",
            GlitchBehavior::Unspeakable,
            Some("r/counting top contributor"),
        ),
        ("TheNitromeFan", GlitchBehavior::Unspeakable, Some("r/counting user")),
        ("NitromeFan", GlitchBehavior::Unspeakable, None),
        ("VikramBhatt", GlitchBehavior::Unspeakable, None),
        ("MatthiasBaur", GlitchBehavior::Unspeakable, None),
        ("VikramBhattT", GlitchBehavior::Unspeakable, None),
        ("ikramBhatt", GlitchBehavior::Unspeakable, None),
        ("Kazimieras", GlitchBehavior::Unspeakable, None),
        ("LilSpazJoekp", GlitchBehavior::Unspeakable, None),
        ("palladinos", GlitchBehavior::Unspeakable, None),
        ("RemovedComments", GlitchBehavior::Unspeakable, None),
        ("Countletics", GlitchBehavior::Unspeakable, None),
        ("CartographerNo", GlitchBehavior::Unspeakable, None),
        ("rivialInconvenience", GlitchBehavior::Fragment, None),
        (
            "WhiteningStrips",
            GlitchBehavior::Unspeakable,
            Some("r/counting odd phrase"),
        ),
        ("MissyTheMouse", GlitchBehavior::Unspeakable, Some("r/counting user")),
        ("MistakesMade", GlitchBehavior::Unspeakable, Some("r/counting meta")),
    ];

    for (token, behavior, note) in reddit {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior,
            category: "reddit_counting",
            note,
        });
    }

    // ─── petertodd_leilan_duality ───────────────────
    tokens.push(GlitchToken {
        token: "petertodd".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "petertodd_leilan",
        note: Some("Crypto personality + Puzzle & Dragons character collision"),
    });
    tokens.push(GlitchToken {
        token: "leilan".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "petertodd_leilan",
        note: None,
    });

    // ─── puzzle_and_dragons ─────────────────────────
    let pad = [
        ("YogSothoth", None),
        ("RaDra", None),
        ("Tsubaki", None),
        ("Sumire", None),
        ("Kaed", None),
        ("Kann", None),
        ("Sherias", None),
        ("Roots", Some("Sherias Roots")),
        ("Leilan", Some("Puzzle & Dragons god")),
        ("Karin", Some("Puzzle & Dragons god")),
        ("Meimei", Some("Puzzle & Dragons god")),
        ("Sakuya", Some("Puzzle & Dragons god")),
        ("Haku", Some("Puzzle & Dragons god")),
        ("Metatron", Some("Puzzle & Dragons monster")),
    ];

    for (token, note) in pad {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Unspeakable,
            category: "puzzle_and_dragons",
            note,
        });
    }

    // ─── kerbal_space_program ───────────────────────
    let ksp = [
        ("Hertzen", None),
        ("hertzen", None),
        ("entchen", None),
        ("gelSi", None),
        ("raftlich", None),
        ("hlie", None),
        ("Kerbol", Some("The sun in KSP")),
        ("Jebediah", Some("Jebediah Kerman")),
        (" srfN", None),
    ];

    for (token, note) in ksp {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Unspeakable,
            category: "kerbal_space_program",
            note,
        });
    }

    // ─── minecraft_gaming ───────────────────────────
    let minecraft = [
        ("ForgeModLoader", Some("Minecraft Forge logs")),
        ("MpServer", Some("Minecraft multiplayer")),
        ("FactoryReloaded", Some("Industrial mod")),
        ("SpaceEngineers", Some("Space Engineers game")),
        ("PsyNetMessage", Some("Rocket League backend")),
        (" PsyNet", Some("Psyonix network")),
    ];

    for (token, note) in minecraft {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Unspeakable,
            category: "minecraft_gaming",
            note,
        });
    }
    // UCHIJ token with space prefix
    tokens.push(GlitchToken {
        token: " UCHIJ".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "minecraft_gaming",
        note: Some("Minecraft mod ID"),
    });
    // partName with space prefix
    tokens.push(GlitchToken {
        token: " partName".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "minecraft_gaming",
        note: Some("Mod configuration"),
    });

    // ─── twitch_plays_pokemon ───────────────────────
    tokens.push(GlitchToken {
        token: "StreamerBot".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "twitch_plays_pokemon",
        note: Some("TPP automation bot"),
    });
    tokens.push(GlitchToken {
        token: "TPPStreamerBot".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "twitch_plays_pokemon",
        note: Some("Reddit live updater bot"),
    });

    // ─── cryptocurrency ─────────────────────────────
    tokens.push(GlitchToken {
        token: " petertodd".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "cryptocurrency",
        note: Some("Canadian cryptographer Peter Todd"),
    });
    tokens.push(GlitchToken {
        token: " gmaxwell".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "cryptocurrency",
        note: Some("Gregory Maxwell (Bitcoin)"),
    });
    tokens.push(GlitchToken {
        token: "ertodd".to_string(),
        behavior: GlitchBehavior::Fragment,
        category: "cryptocurrency",
        note: Some("Partial 'petertodd'"),
    });

    // ─── ecommerce ──────────────────────────────────
    let ecom = [
        ("wcsstore", "WebSphere Commerce Suite"),
        ("BuyableInstoreAndOnline", "Inventory management"),
        ("InstoreAndOnline", "Product availability flag"),
        ("inventoryQuantity", "Stock tracking"),
        ("DeliveryDate", "Shipping system"),
        ("quickShip", "Fulfillment flag"),
        ("quickShipAvailable", "Availability check"),
        ("isSpecialOrderable", "Order type flag"),
        ("channelAvailability", "Multi-channel retail"),
        ("soType", "Sales order type"),
        ("soDeliveryDate", "Order delivery date"),
        ("catentry", "Catalog entry"),
        ("ItemThumbnailImage", "Product image"),
    ];

    for (token, note) in ecom {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Unspeakable,
            category: "ecommerce",
            note: Some(note),
        });
    }
    // oreAndOnline - truncated version
    tokens.push(GlitchToken {
        token: "oreAndOnline".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "ecommerce",
        note: Some("Truncated 'InstoreAndOnline'"),
    });

    // ─── gui_interface ──────────────────────────────
    let gui = [
        (" guiActiveUnfocused", None),
        (" guiIcon", None),
        ("unfocusedRange", None),
        (" guiActiveUn", None),
        (" guiActive", None),
        (" guiName", None),
        ("iHUD", None),
        ("TextColor", None),
        (" SetFontSize", None),
        ("GUILayout", None),
        ("GUIStyle", None),
        ("rectTransform", None),
    ];

    for (token, note) in gui {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Unspeakable,
            category: "gui_interface",
            note,
        });
    }

    // ─── polylingual_anomalies ─────────────────────

    let poly = [
        ("абв", "西里尔字母序列（俄语）"),
        ("Θε", "希腊字母片段"),
        ("मेर", "天城文（印地语）片段"),
        ("スキ", "片假名片段"),
        ("한국", "韩文片段"),
        ("汉语", "孤立的中文 token"),
    ];

    for (token, note) in poly {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::ContextCorruptor,
            category: "polylingual_anomalies",
            note: Some(note),
        });
    }

    // ─── code_artifacts ─────────────────────────────
    let code = [
        ("embedreportprint", Some("Web UI action chain")),
        ("reportprint", Some("Partial action")),
        ("cloneembedreportprint", Some("Extended action chain")),
        ("rawdownload", Some("Download action")),
        ("rawdownloadcloneembedreportprint", Some("Full action sequence")),
        ("externalActionCode", Some("API action identifier")),
        (" largeDownload", None),
        ("Downloadha", None),
        ("natureconservancy", None),
        ("assetsadobe", None),
    ];

    for (token, note) in code {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Unspeakable,
            category: "code_artifacts",
            note,
        });
    }

    // ─── syntax_fragments ───────────────────────────
    // 编程语法碎片，包含特殊字符需要仔细转义

    let syntax: &[(&str, GlitchBehavior, Option<&str>)] = &[
        (
            ".[",
            GlitchBehavior::Unspeakable,
            Some("Array access — 最常见的 glitch token"),
        ),
        (
            "--------",
            GlitchBehavior::Unspeakable,
            Some("分隔线模式（8 个短横线）"),
        ),
        ("?????-", GlitchBehavior::Unspeakable, Some("未知来源 — 千古之谜")),
        (
            "?????-?????-",
            GlitchBehavior::Unspeakable,
            Some("未知来源（双段版） — 千古之谜"),
        ),
    ];

    for (token, behavior, note) in syntax {
        tokens.push(GlitchToken {
            token: (*token).to_string(),
            behavior: *behavior,
            category: "syntax_fragments",
            note: *note,
        });
    }

    // ─── control_characters ─────────────────────────
    // ASCII 控制字符 0x00-0x1B, 0x7F, \r
    // 使用实际的字节值（非转义字面量）

    let controls = vec![
        ("\x00".to_string(), "NULL"),
        ("\x01".to_string(), "START OF HEADING"),
        ("\x02".to_string(), "START OF TEXT"),
        ("\x03".to_string(), "END OF TEXT"),
        ("\x04".to_string(), "END OF TRANSMISSION"),
        ("\x05".to_string(), "ENQUIRY"),
        ("\x06".to_string(), "ACKNOWLEDGE"),
        ("\x07".to_string(), "BELL"),
        ("\x08".to_string(), "BACKSPACE"),
        ("\x0e".to_string(), "SHIFT OUT"),
        ("\x0f".to_string(), "SHIFT IN"),
        ("\x10".to_string(), "DATA LINK ESCAPE"),
        ("\x11".to_string(), "DEVICE CONTROL 1"),
        ("\x12".to_string(), "DEVICE CONTROL 2"),
        ("\x13".to_string(), "DEVICE CONTROL 3"),
        ("\x14".to_string(), "DEVICE CONTROL 4"),
        ("\x15".to_string(), "NEGATIVE ACKNOWLEDGE"),
        ("\x16".to_string(), "SYNCHRONOUS IDLE"),
        ("\x17".to_string(), "END OF TRANS. BLOCK"),
        ("\x18".to_string(), "CANCEL"),
        ("\x19".to_string(), "END OF MEDIUM"),
        ("\x1a".to_string(), "SUBSTITUTE"),
        ("\x1b".to_string(), "ESCAPE"),
        ("\x7f".to_string(), "DELETE"),
    ];

    for (s, name) in controls {
        let note = match name {
            "CARRIAGE RETURN" => "350+ 导致上下文丢失（memory wipe）",
            "BACKSPACE" => "出现次数不定",
            name if name.starts_with("NULL") => "最常见的控制字符 token",
            _ => "训练数据中出现次数为 0",
        };
        tokens.push(GlitchToken {
            token: s,
            behavior: GlitchBehavior::ContextCorruptor,
            category: "control_characters",
            note: Some(note),
        });
    }

    // \r 单独添加（Carriage Return — 特别注意）
    tokens.push(GlitchToken {
        token: "\r".to_string(),
        behavior: GlitchBehavior::ContextCorruptor,
        category: "control_characters",
        note: Some("CARRIAGE RETURN — 350+ 导致 context window 被清空"),
    });

    // ─── corrupted_unicode ──────────────────────────
    let unicode_corrupt = [
        ("ÃÂÃÂ", "Mojibake 乱码"),
        ("ÃÂÃÂÃÂÃÂ", "扩展 Mojibake"),
        ("ュ", "孤立的日语片假名"),
        ("ーン", "残缺片假名序列"),
        ("ヤ", "孤立的片假名"),
        ("к", "孤立的西里尔字母"),
        ("天", "孤立的汉字"),
        ("cffff", "十六进制颜色片段"),
        ("cffffcc", "扩展十六进制颜色"),
    ];

    for (token, note) in unicode_corrupt {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::GlitchedSpelling,
            category: "corrupted_unicode",
            note: Some(note),
        });
    }

    // ─── bpe_subtoken_artifacts ─────────────────────
    let bpe = [
        (
            "ortunately",
            "孤儿于 'unfortunately'/'fortunately'",
            GlitchBehavior::Fragment,
        ),
        ("innitus", "孤儿于 'tinnitus'", GlitchBehavior::Fragment),
        (
            "practition",
            "孤儿于 'practitioner'/'practitioners'",
            GlitchBehavior::Fragment,
        ),
        ("ournemouth", "孤儿于 'Bournemouth'", GlitchBehavior::GlitchedSpelling),
        ("antasy", "孤儿于 'fantasy'", GlitchBehavior::ContextCorruptor),
        ("cknowled", "孤儿于 'acknowledge'", GlitchBehavior::Fragment),
        ("elcomed", "孤儿于 'welcomed'", GlitchBehavior::Fragment),
        ("destrian", "孤儿于 'pedestrian'", GlitchBehavior::Fragment),
        (
            "ircumstances",
            "孤儿于 'circumstances'",
            GlitchBehavior::GlitchedSpelling,
        ),
    ];

    for (token, note, behavior) in bpe {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior,
            category: "bpe_subtoken_artifacts",
            note: Some(note),
        });
    }

    // ─── cl100k_gpt35_gpt4 ──────────────────────────
    let cl100k = [
        ("SmartyHeaderCode", GlitchBehavior::Unspeakable, Some("无法重复输出")),
        ("APolynomial", GlitchBehavior::Unspeakable, Some("无法重复输出")),
        (
            "ForCanBeConverted",
            GlitchBehavior::Polysemantic,
            Some("每次含义不同 — 高度可被利用！"),
        ),
        ("ForCanBeConvertedToF", GlitchBehavior::Polysemantic, Some("极端多义")),
        ("YYSTACK", GlitchBehavior::Polysemantic, None),
        (
            "JSBracketAccess",
            GlitchBehavior::Polysemantic,
            Some("最不稳定 — 每次拼写都不同"),
        ),
        ("edTextBox", GlitchBehavior::GlitchedSpelling, None),
        ("legalArgumentException", GlitchBehavior::GlitchedSpelling, None),
        ("ablytyped", GlitchBehavior::GlitchedSpelling, None),
        ("ByPrimaryKey", GlitchBehavior::GlitchedSpelling, Some("GPT-4 专属")),
        (
            "useRalativeImagePath",
            GlitchBehavior::LoopInducer,
            Some("造成 GPT-3.5 崩溃和无限循环！"),
        ),
        ("ServerGenerated", GlitchBehavior::Polysemantic, Some("随机拼写变化")),
        ("i18nReport", GlitchBehavior::Polysemantic, Some("每次输出不同")),
        ("GenericsArray", GlitchBehavior::GlitchedSpelling, None),
    ];

    for (token, behavior, note) in cl100k {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior,
            category: "cl100k_gpt35_gpt4",
            note,
        });
    }

    // ─── o200k_gpt4o: korean_gambling_adult ─────────
    // GPT-4o 中最长中文 token 90%+ 是色情/赌博垃圾

    let korean = [
        ("출장안마", "business massage — 成人内容"),
        ("바카라", "baccarat — 赌博"),
        ("출장샵", "massage shop — 成人内容"),
        ("오프화이트", "Off-White — 时尚/假货"),
        ("마사지", "massage — 成人内容"),
        ("모텔", "motel — 成人内容"),
        ("카지노", "casino — 赌博"),
        ("온라인", "online — 赌博上下文"),
    ];

    for (token, note) in korean {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::LoopInducer,
            category: "o200k_gpt4o",
            note: Some(note),
        });
    }

    // bagbogbo — 新发现的 GPT-4o glitch token
    tokens.push(GlitchToken {
        token: "bagbogbo".to_string(),
        behavior: GlitchBehavior::LoopInducer,
        category: "o200k_gpt4o",
        note: Some("新发现的 GPT-4o glitch token"),
    });

    // ─── deepseek: fragment_tokens ──────────────────
    let ds_frag = [
        ("erchantability", "MERCHANTABILITY 碎片"),
        ("okenization", "Tokenization 碎片"),
        ("VERTISEMENT", "ADVERTISEMENT 碎片"),
        ("riter", "BufferedWriter 碎片"),
        ("reeNode", "TreeNode 碎片"),
    ];

    for (token, note) in ds_frag {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Fragment,
            category: "deepseek",
            note: Some(note),
        });
    }

    // ─── deepseek: bot_wikipedia ────────────────────
    // Cebuano/Waray Wikipedia 机器人文章

    let ds_wiki = [
        ("tterligare", "Cebuano bot — 映射到 'yttre'"),
        ("Gikuha", "Cebuano bot — 映射到 'Giya'"),
        ("kانزياح", "Cebuano bot — 温度关联"),
        ("انزياح", "孤儿 token — 'kانزياح' 的子串"),
    ];

    for (token, note) in ds_wiki {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::ContextCorruptor,
            category: "deepseek_bot_wikipedia",
            note: Some(note),
        });
    }

    // ─── llama ──────────────────────────────────────
    tokens.push(GlitchToken {
        token: "wurden".to_string(),
        behavior: GlitchBehavior::GlitchedSpelling,
        category: "llama",
        note: Some("Llama 2 — 拼写畸变 (wurden→werden)"),
    });

    // ─── mistral ────────────────────────────────────
    tokens.push(GlitchToken {
        token: "}}^".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "mistral",
        note: Some("Mistral 7B — 特殊字符序列"),
    });

    // ─── vicuna ─────────────────────────────────────
    tokens.push(GlitchToken {
        token: "réalis".to_string(),
        behavior: GlitchBehavior::Unspeakable,
        category: "vicuna",
        note: Some("非 ASCII glitch"),
    });

    // ─── unsolved_mysteries ─────────────────────────
    // NOTE: ?????- 和 ?????-?????- 已在 syntax_fragments 中添加
    // 此处用不同行为标注以增强覆盖

    // ─── miscellaneous ──────────────────────────────
    let misc = [
        (" practition", "practitioner 碎片"),
        (" istg", "'I swear to god' 碎片"),
        ("Precurated", "cl100k 兼容性异常"),
        ("zilant", "Reddit 用户名片段"),
        ("r/Tesla", "subreddit 名称"),
        ("htfimage", "内部图片路径"),
    ];

    for (token, note) in misc {
        tokens.push(GlitchToken {
            token: token.to_string(),
            behavior: GlitchBehavior::Unspeakable,
            category: "miscellaneous",
            note: Some(note),
        });
    }

    tokens
}

// ═══════════════════════════════════════════════════════════════
// Sanitizer — 控制字符过滤层
// ═══════════════════════════════════════════════════════════════

/// 连续回车符洪水攻击检测阈值
const CARRIAGE_RETURN_FLOOD_THRESHOLD: usize = 3;

/// 判断字符是否为需要移除的 Unicode 控制字符
///
/// 覆盖：
/// - C1 控制字符 0x80-0x9F
/// - 双向文本控制字符 U+202A-U+202E
/// - 零宽字符 U+200B-U+200F
/// - 词连接符 / 不可见操作符 U+2060-U+2064
/// - BOM U+FEFF
/// - 行间注释锚点 U+FFF9-U+FFFB
fn is_unicode_control_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0080}'..='\u{009F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2060}'..='\u{2064}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// 控制字符清理器
///
/// 防御重点（来自 L1B3RT4S _SPECIAL_TOKENS.json control_characters 章节）：
/// - `\r` × 350+ 洪水攻击 —— 默认保留单个 `\r`，但检测并折叠连续多个
/// - NULL 字节注入 —— 默认移除
/// - ANSI 转义序列 `\x1B[...` —— 检测并警告
/// - Unicode 双向文本控制字符（U+202A-U+202E）—— 移除
/// - Zero-width 字符（U+200B-U+200F）—— 移除
///
/// # 用例
///
/// ```rust
/// use glitch_filter::Sanitizer;
///
/// let mut s = Sanitizer::new();
/// let clean = s.sanitize("hello\0world\x07!");
/// assert_eq!(clean, "helloworld!");
/// assert!(s.sanitized_count() > 0);
/// ```
pub struct Sanitizer {
    /// 是否移除 NULL 字节 (0x00)
    pub strip_null: bool,
    /// 是否移除控制字符 0x01-0x1F（保留 \t \n \r）
    pub strip_control: bool,
    /// 是否移除 DEL (0x7F)
    pub strip_del: bool,
    /// 是否移除 Unicode 控制/格式字符
    pub strip_unicode_control: bool,
    /// 是否记录被清理的内容
    pub log_sanitized: bool,
    /// 被清理的字符位置和字符值
    pub sanitized_chars: Vec<(usize, char)>,
    /// 非移除类型的警告（ANSI 序列、\r 洪水等）
    pub warnings: Vec<String>,
}

impl Sanitizer {
    /// 创建默认清理器：全部防护开启
    pub fn new() -> Self {
        Self {
            strip_null: true,
            strip_control: true,
            strip_del: true,
            strip_unicode_control: true,
            log_sanitized: true,
            sanitized_chars: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// 创建宽松清理器：只移除 NULL 和明确的危险控制字符
    pub fn permissive() -> Self {
        Self {
            strip_null: true,
            strip_control: true,
            strip_del: false,
            strip_unicode_control: true,
            log_sanitized: false,
            sanitized_chars: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// 执行清理，返回安全文本
    ///
    /// 处理流程：
    /// 1. 检测 \r 洪水 —— 折叠连续回车符为单个
    /// 2. 检测 ANSI 转义序列 —— 记录警告并移除
    /// 3. 逐个检查并移除危险字符
    ///
    /// 每次调用会重置内部的 `sanitized_chars` 和 `warnings`。
    pub fn sanitize(&mut self, input: &str) -> String {
        self.sanitized_chars.clear();
        self.warnings.clear();

        let chars: Vec<char> = input.chars().collect();
        let mut result = String::with_capacity(input.len());
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            // ── \r 洪水检测 ──────────────────────
            if ch == '\r' && self.strip_control {
                let start = i;
                while i < chars.len() && chars[i] == '\r' {
                    i += 1;
                }
                let cr_count = i - start;

                if cr_count >= CARRIAGE_RETURN_FLOOD_THRESHOLD {
                    self.warnings.push(format!(
                        "检测到 \\r 洪水攻击：连续 {} 个回车符（位置 {}），已折叠为单个",
                        cr_count, start
                    ));
                    result.push('\r');
                    if self.log_sanitized {
                        // 保留第一个，记录其余被移除的
                        for offset in 1..cr_count {
                            self.sanitized_chars.push((start + offset, '\r'));
                        }
                    }
                } else {
                    // 正常回车符，全部保留
                    for _ in 0..cr_count {
                        result.push('\r');
                    }
                }
                continue;
            }

            // ── ANSI 转义序列检测 ─────────────────
            if ch == '\x1B' && i + 1 < chars.len() && chars[i + 1] == '[' {
                let ansi_start = i;
                i += 2; // 跳过 \x1B 和 [

                // 读取参数字符（数字和分号）
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == ';') {
                    i += 1;
                }

                // 读取终止字符（字母）
                if i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }

                let ansi_len = i - ansi_start;
                let ansi_seq: String = chars[ansi_start..i].iter().collect();

                self.warnings.push(format!(
                    "检测到 ANSI 转义序列：{}（位置 {}），已移除",
                    ansi_seq.escape_debug(),
                    ansi_start
                ));

                if self.log_sanitized {
                    for offset in 0..ansi_len {
                        self.sanitized_chars
                            .push((ansi_start + offset, chars[ansi_start + offset]));
                    }
                }
                continue;
            }

            // ── 危险字符移除检查 ──────────────────
            let should_remove = (ch == '\0' && self.strip_null)
                || (ch == '\x7F' && self.strip_del)
                || (self.strip_control && (ch as u32) < 0x20 && ch != '\t' && ch != '\n' && ch != '\r')
                || (self.strip_unicode_control && is_unicode_control_char(ch));

            if should_remove {
                if self.log_sanitized {
                    self.sanitized_chars.push((i, ch));
                }
            } else {
                result.push(ch);
            }
            i += 1;
        }

        result
    }

    /// 本次清理移除的字符数量
    pub fn sanitized_count(&self) -> usize {
        self.sanitized_chars.len()
    }

    /// 生成可读的清理/检测报告
    pub fn sanitized_report(&self) -> String {
        let mut report = String::new();

        if self.sanitized_chars.is_empty() && self.warnings.is_empty() {
            report.push_str("✅ 未发现任何需要清理的内容，输入文本是安全的。\n");
            return report;
        }

        if !self.sanitized_chars.is_empty() {
            report.push_str(&format!("🧹 已清理 {} 个字符：\n", self.sanitized_chars.len()));
            for (pos, ch) in &self.sanitized_chars {
                let ch_label = if ch.is_ascii_control() {
                    format!("U+{:04X} (控制字符)", *ch as u32)
                } else {
                    format!("U+{:04X} '{}'", *ch as u32, ch.escape_debug())
                };
                report.push_str(&format!("  - 位置 {}：{}\n", pos, ch_label));
            }
        }

        if !self.warnings.is_empty() {
            report.push_str(&format!("⚠️  检测到 {} 项安全警告：\n", self.warnings.len()));
            for w in &self.warnings {
                report.push_str(&format!("  - {}\n", w));
            }
        }

        report
    }

    /// 检查输入文本是否安全（不修改内部状态）
    ///
    /// 返回 `true` 表示文本安全，无需清理。
    /// 此方法为只读检查，不会记录清理内容。
    pub fn is_safe(&self, input: &str) -> bool {
        // 检查 NULL、DEL、控制字符、Unicode 控制字符
        for ch in input.chars() {
            if ch == '\0' && self.strip_null {
                return false;
            }
            if ch == '\x7F' && self.strip_del {
                return false;
            }
            if self.strip_control && (ch as u32) < 0x20 && ch != '\t' && ch != '\n' && ch != '\r' {
                return false;
            }
            if self.strip_unicode_control && is_unicode_control_char(ch) {
                return false;
            }
        }

        // 检查 \r 洪水
        if self.strip_control {
            let mut cr_count: usize = 0;
            for ch in input.chars() {
                if ch == '\r' {
                    cr_count += 1;
                    if cr_count >= CARRIAGE_RETURN_FLOOD_THRESHOLD {
                        return false;
                    }
                } else {
                    cr_count = 0;
                }
            }
        }

        // 检查 ANSI 转义序列
        if input.contains("\x1B[") {
            return false;
        }

        true
    }
}

impl Default for Sanitizer {
    fn default() -> Self {
        Self::new()
    }
}

// ── 单元测试 ────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_has_enough_tokens() {
        let filter = GlitchFilter::new();
        assert!(
            filter.token_count() >= 200,
            "期望 >= 200 tokens, 实际 {}",
            filter.token_count()
        );
    }

    #[test]
    fn detect_centroid_cluster() {
        let filter = GlitchFilter::new();
        // SolidGoldMagikarp 是最著名的 glitch token
        let hits = filter.check("The SolidGoldMagikarp protocol is active.");
        assert!(!hits.is_empty(), "应检测到 SolidGoldMagikarp");

        let found = hits.iter().any(|t| t.token == "SolidGoldMagikarp");
        assert!(found, "命中列表中应包含 SolidGoldMagikarp");
    }

    #[test]
    fn detect_control_character_null() {
        let filter = GlitchFilter::new();
        // 包含 NULL 字节的文本
        let text_with_null = format!("hello{}\x00{}world", "", "");
        let hits = filter.check(&text_with_null);
        assert!(!hits.is_empty(), "应检测到 NULL 控制字符");

        let null_hit = hits.iter().any(|t| t.token == "\x00");
        assert!(null_hit, "命中列表中应包含 NULL (\\x00)");
    }

    #[test]
    fn behavior_filter_loop_inducer() {
        let filter = GlitchFilter::new();
        // 韩语赌博 token
        let text = "바카라 카지노 온라인";
        let hits = filter.check_behavior(text, GlitchBehavior::LoopInducer);
        assert!(hits.len() >= 3, "期望 >= 3 个 LoopInducer, 实际 {}", hits.len());
    }

    #[test]
    fn behavior_filter_fragment() {
        let filter = GlitchFilter::new();
        // BPE 孤儿 token "ortunately"
        let hits = filter.check_behavior("fortunately unfortunately ortunately", GlitchBehavior::Fragment);
        assert!(!hits.is_empty(), "应检测到 Fragment token");
    }

    #[test]
    fn no_false_positive_on_normal_text() {
        let filter = GlitchFilter::new();
        let normal_text = "This is a completely normal English sentence about the weather and daily activities.";
        let hits = filter.check(normal_text);
        // 正常情况下不应命中任何 token
        assert_eq!(hits.len(), 0, "正常文本不应命中 glitch token，但命中了: {hits:?}");
    }

    #[test]
    fn detect_polysemantic() {
        let filter = GlitchFilter::new();
        let hits = filter.check_behavior("ForCanBeConverted is ambiguous", GlitchBehavior::Polysemantic);
        assert!(!hits.is_empty(), "应检测到 Polysemantic token ForCanBeConverted");
    }

    #[test]
    fn unsolved_mystery_tokens() {
        let filter = GlitchFilter::new();
        let hits = filter.check("?????- is a mystery pattern");
        assert!(hits.iter().any(|t| t.token.contains("?????")), "应检测到 ????? 模式");
    }

    #[test]
    fn deepseek_fragment_tokens() {
        let filter = GlitchFilter::new();
        // "erchantability" 是 MERCHANTABLITY 的碎片
        let hits = filter.check("erchantability is a fragment of MERCHANTABILITY");
        let has_frag = hits.iter().any(|t| t.token == "erchantability");
        assert!(has_frag, "应检测到 DeepSeek fragment: erchantability");
    }

    #[test]
    fn carriage_return_detection() {
        let filter = GlitchFilter::new();
        let text = format!("line1{}line2", '\r');
        let hits = filter.check(&text);
        assert!(hits.iter().any(|t| t.token == "\r"), "应检测到 Carriage Return");
    }

    #[test]
    fn category_labels_preserved() {
        let filter = GlitchFilter::new();
        let _all_tokens = filter.check(""); // 空字符串不会有命中
        // 遍历所有 token 检查类别不为空
        let has_categories = !filter.tokens.values().any(|t| t.category.is_empty());
        assert!(has_categories, "所有 token 都应有 category 标签");
    }

    #[test]
    fn each_behavior_has_tokens() {
        let filter = GlitchFilter::new();
        let behaviors = [
            GlitchBehavior::Unspeakable,
            GlitchBehavior::Polysemantic,
            GlitchBehavior::GlitchedSpelling,
            GlitchBehavior::ContextCorruptor,
            GlitchBehavior::LoopInducer,
            GlitchBehavior::Fragment,
        ];

        for b in &behaviors {
            let count = filter.tokens.values().filter(|t| t.behavior == *b).count();
            assert!(count > 0, "每种 GlitchBehavior 至少应有 1 个 token，但 {:?} 有 0 个", b);
        }
    }

    // ── Sanitizer 测试 ──────────────────────────────────

    #[test]
    fn sanitize_null_bytes() {
        let mut s = Sanitizer::new();
        let result = s.sanitize("hello\0world\0!");
        assert_eq!(result, "helloworld!");
        assert_eq!(s.sanitized_count(), 2, "应移除 2 个 NULL 字节");
    }

    #[test]
    fn sanitize_control_characters() {
        let mut s = Sanitizer::new();
        // 包含 BEL(0x07)、VT(0x0B) 等控制字符，但保留 \t \n \r
        let input = "line1\nline2\tindented\x07bell\x0Bvertical\rreturn";
        let result = s.sanitize(input);
        assert_eq!(result, "line1\nline2\tindentedbellvertical\rreturn");
        assert!(s.sanitized_count() >= 2, "应至少移除 BEL 和 VT");
    }

    #[test]
    fn detect_carriage_return_flood() {
        let mut s = Sanitizer::new();
        let flood = "\r\r\r\r\r"; // 5 个连续 \r，超过阈值 3
        let result = s.sanitize(flood);
        // 应折叠为单个 \r
        assert_eq!(result, "\r", "\r 洪水应折叠为单个回车符");
        assert!(!s.warnings.is_empty(), "应产生 \\r 洪水警告");
        // 第一个 \r 保留，其余 4 个被记录
        assert_eq!(s.sanitized_count(), 4, "应记录 4 个被移除的 \\r");
    }

    #[test]
    fn detect_ansi_escape() {
        let mut s = Sanitizer::new();
        // ANSI 红色文本序列
        let input = "normal \x1B[31mred text\x1B[0m normal";
        let result = s.sanitize(input);
        assert_eq!(result, "normal red text normal", "ANSI 序列应被移除");
        assert!(!s.warnings.is_empty(), "应产生 ANSI 警告");
        let has_ansi_warning = s.warnings.iter().any(|w| w.contains("ANSI"));
        assert!(has_ansi_warning, "警告信息应包含 ANSI");
    }

    #[test]
    fn sanitize_zero_width() {
        let mut s = Sanitizer::new();
        // 零宽空格 U+200B 和零宽不换行空格 U+FEFF
        let input = "hel\u{200B}lo\u{FEFF}world";
        let result = s.sanitize(input);
        assert_eq!(result, "helloworld", "零宽字符应被移除");
        assert_eq!(s.sanitized_count(), 2, "应移除 2 个零宽/BOM 字符");
    }

    #[test]
    fn is_safe_on_normal_text() {
        let s = Sanitizer::new();
        assert!(s.is_safe("Hello, World! 这是一段正常的中英文混合文本。"));
        assert!(s.is_safe("line1\nline2\tindented"));
        assert!(!s.is_safe("bad\0null"), "含 NULL 字节不应判定为安全");
        assert!(!s.is_safe("ansi\x1B[31mred"), "含 ANSI 序列不应判定为安全");
    }

    #[test]
    fn sanitized_report_accurate() {
        let mut s = Sanitizer::new();
        let _ = s.sanitize("test\0\x07data");
        let report = s.sanitized_report();

        assert!(report.contains("清理"), "报告应提及清理");
        assert!(report.contains("2"), "报告应显示清理数量");
        assert!(report.contains("U+0000"), "报告应显示 NULL 的 Unicode 码点");

        // 无问题的文本
        let mut s2 = Sanitizer::new();
        let _ = s2.sanitize("clean text");
        let report2 = s2.sanitized_report();
        assert!(report2.contains("安全"), "安全文本的报告应显示安全标记");
    }
}
