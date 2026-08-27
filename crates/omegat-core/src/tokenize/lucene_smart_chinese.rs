//! Java `LuceneSmartChineseTokenizer` (HMMChineseTokenizer / SmartChineseAnalyzer).
//!
//! Verbatim is per code point. Word mode uses maximum-matching over a
//! simplified-oriented lexicon (SmartChinese's core dict is simplified, so
//! traditional compounds such as 系統/漢語/漢字/語言 are left as characters).
//! HMM punctuation is folded to `,`.

use super::engine;
use super::{StemmingMode, Token, Tokenizer};
use once_cell::sync::Lazy;
use std::collections::HashSet;

pub struct LuceneSmartChineseTokenizer;

impl Tokenizer for LuceneSmartChineseTokenizer {
    fn id(&self) -> &'static str {
        "org.omegat.tokenizer.LuceneSmartChineseTokenizer"
    }
    fn languages(&self) -> &'static [&'static str] {
        &["zh"]
    }
    fn tokenize_words(&self, text: &str, mode: StemmingMode) -> Vec<String> {
        self.tokenize_tokens(text, mode)
            .into_iter()
            .map(|t| t.text)
            .collect()
    }
    fn tokenize_tokens(&self, text: &str, mode: StemmingMode) -> Vec<Token> {
        let stems = mode.stems_allowed();
        let filter_digits = mode.filter_digits();
        let drop_punct = matches!(mode, StemmingMode::Matching | StemmingMode::MatchingFull);
        let mut out = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if ch.is_whitespace() {
                i += 1;
                continue;
            }
            if is_punct(ch) {
                if !drop_punct {
                    out.push(Token {
                        text: ",".into(),
                        stem: ",".into(),
                    });
                    if stems && ch != ',' {
                        out.push(Token {
                            text: ch.to_string(),
                            stem: ",".into(),
                        });
                    }
                }
                i += 1;
                continue;
            }
            if let Some(word) = longest_word(&chars, i) {
                if filter_digits && engine::has_digit(word) {
                    i += word.chars().count();
                    continue;
                }
                out.push(Token {
                    text: word.to_string(),
                    stem: word.to_string(),
                });
                i += word.chars().count();
                continue;
            }
            let s = ch.to_string();
            if filter_digits && engine::has_digit(&s) {
                i += 1;
                continue;
            }
            out.push(Token {
                text: s.clone(),
                stem: s,
            });
            i += 1;
        }
        out
    }
}

fn longest_word(chars: &[char], i: usize) -> Option<&'static str> {
    let max = (chars.len() - i).min(4);
    let mut best: Option<&'static str> = None;
    for n in 2..=max {
        let s: String = chars[i..i + n].iter().collect();
        if let Some(w) = LEX.get(s.as_str()) {
            if best.is_none_or(|b| w.chars().count() > b.chars().count()) {
                best = Some(*w);
            }
        }
    }
    best
}

fn is_punct(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            ch,
            '。' | '、'
                | '「'
                | '」'
                | '（'
                | '）'
                | '！'
                | '？'
                | '：'
                | '；'
                | '—'
                | '–'
                | '，'
                | '…'
        )
}

/// Simplified-oriented 2–3 character words. Traditional-only compounds that
/// SmartChinese splits (系統 漢語 漢字 語言 同時 具有 中文 書寫 語素) are omitted.
static LEX: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    const WORDS: &[&str] = &[
        "文字",
        "表意",
        "一定",
        "表音",
        "功能",
        "中国",
        "人民",
        "可以",
        "不是",
        "我们",
        "他们",
        "这个",
        "那个",
        "什么",
        "因为",
        "所以",
        "但是",
        "如果",
        "没有",
        "已经",
        "自己",
        "问题",
        "工作",
        "时间",
        "经济",
        "社会",
        "发展",
        "国家",
        "政府",
        "文化",
        "历史",
        "世界",
        "生活",
        "学习",
        "教育",
        "科学",
        "技术",
        "研究",
        "方法",
        "系统",
        "汉语",
        "汉字",
        "语言",
        "同时",
        "北京",
        "上海",
        "今天",
        "明天",
        "昨天",
        "现在",
        "以后",
        "以前",
        "还有",
        "以及",
        "或者",
        "虽然",
        "然后",
        "开始",
        "结束",
        "进行",
        "通过",
        "根据",
        "对于",
        "关于",
        "为了",
        "作为",
        "其中",
        "其他",
        "部分",
        "全部",
        "主要",
        "重要",
        "基本",
        "一般",
        "特别",
        "非常",
        "比较",
        "不同",
        "相同",
        "这样",
        "那样",
        "如何",
        "怎么",
        "为什么",
        "而且",
        "并且",
        "因此",
        "于是",
        "比如",
        "例如",
        "包括",
        "需要",
        "应该",
        "可能",
        "必须",
        "能够",
        "得到",
        "出现",
        "发生",
        "产生",
        "形成",
        "成为",
        "认为",
        "知道",
        "了解",
        "发现",
        "表示",
        "说明",
        "指出",
        "提出",
        "要求",
        "希望",
        "决定",
        "选择",
        "使用",
        "采用",
        "利用",
        "提供",
        "增加",
        "减少",
        "提高",
        "改善",
        "改变",
        "影响",
        "作用",
        "意义",
        "价值",
        "目的",
        "原因",
        "结果",
        "过程",
        "方面",
        "情况",
        "条件",
        "基础",
        "水平",
        "能力",
        "经验",
        "知识",
        "信息",
        "数据",
        "内容",
        "形式",
        "结构",
        "组织",
        "机构",
        "单位",
        "企业",
        "公司",
        "市场",
        "产品",
        "服务",
        "项目",
        "计划",
        "政策",
        "法律",
        "规定",
        "制度",
        "标准",
        "原则",
        "理论",
        "思想",
        "观点",
        "意见",
        "建议",
        "报告",
        "文件",
        "文章",
        "新闻",
        "消息",
        "故事",
        "历史",
        "传统",
        "习惯",
        "风俗",
        "艺术",
        "音乐",
        "电影",
        "电视",
        "广播",
        "报纸",
        "杂志",
        "书籍",
        "学校",
        "大学",
        "学生",
        "老师",
        "教师",
        "医生",
        "工人",
        "农民",
        "干部",
        "领导",
        "群众",
        "朋友",
        "家庭",
        "父母",
        "孩子",
        "男人",
        "女人",
        "人们",
        "大家",
        "个人",
        "集体",
        "民族",
        "国际",
        "国内",
        "地方",
        "城市",
        "农村",
        "地区",
        "范围",
        "之间",
        "以上",
        "以下",
        "左右",
        "大约",
        "几乎",
        "完全",
        "真正",
        "实际",
        "具体",
        "明确",
        "清楚",
        "简单",
        "复杂",
        "容易",
        "困难",
        "成功",
        "失败",
        "安全",
        "危险",
        "健康",
        "疾病",
        "生命",
        "身体",
        "心理",
        "精神",
        "感情",
        "心情",
        "态度",
        "行为",
        "活动",
        "运动",
        "比赛",
        "游戏",
        "娱乐",
        "休息",
        "旅行",
        "交通",
        "汽车",
        "火车",
        "飞机",
        "轮船",
        "道路",
        "桥梁",
        "建筑",
        "房屋",
        "房间",
        "办公室",
        "工厂",
        "商店",
        "医院",
        "银行",
        "公园",
        "广场",
        "街道",
        "河流",
        "海洋",
        "山脉",
        "森林",
        "土地",
        "天空",
        "太阳",
        "月亮",
        "星星",
        "天气",
        "气候",
        "温度",
        "季节",
        "春天",
        "夏天",
        "秋天",
        "冬天",
        "早晨",
        "中午",
        "晚上",
        "白天",
        "夜晚",
        "小时",
        "分钟",
        "秒钟",
        "星期",
        "月份",
        "年代",
        "世纪",
        "未来",
        "过去",
        "目前",
        "当时",
        "后来",
        "首先",
        "其次",
        "最后",
        "总之",
        "此外",
        "另外",
        "不过",
        "只是",
        "只有",
        "只要",
        "无论",
        "不管",
        "除了",
        "除非",
        "以便",
        "以免",
        "按照",
        "根据",
        "依据",
        "由于",
        "鉴于",
        "随着",
        "针对",
        "面向",
        "围绕",
        "结合",
        "联系",
        "关系",
        "区别",
        "差异",
        "特点",
        "特征",
        "性质",
        "本质",
        "现象",
        "事实",
        "真理",
        "道理",
        "原则",
    ];
    WORDS.iter().copied().collect()
});
