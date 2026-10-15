//! lovelog 짧은 글 규칙(200자). tyange-cms `app/utils/short-form.ts`, lovelog `src/lib/timeline.ts`,
//! cms-api `craft_sync.rs`와 같은 값·같은 글자 수 계산이다. **기준일·한도·태그 조건을 바꾸면 네 곳을 함께 고친다.**

use crate::cms::Tag;

pub const SHORT_FORM_SINCE: &str = "2026-09-23";
pub const SHORT_FORM_LIMIT: usize = 200;
const LOVELOG_CATEGORY: &str = "general";
const LOVELOG_TAGS: [&str; 2] = ["love", "lovelog"];

pub fn is_short_form(tags: &[Tag], published_at: &str) -> bool {
    published_at >= SHORT_FORM_SINCE
        && tags
            .iter()
            .any(|t| t.category == LOVELOG_CATEGORY && LOVELOG_TAGS.contains(&t.tag.as_str()))
}

fn find_char(chars: &[char], from: usize, target: char) -> Option<usize> {
    (from..chars.len()).find(|&i| chars[i] == target)
}

/// `![alt](url)`은 통째로 지우고 `[text](url)`은 text만 남긴다.
fn strip_links(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let is_image = chars[i] == '!' && chars.get(i + 1) == Some(&'[');
        let open = if is_image { i + 1 } else { i };
        if chars[open] == '[' {
            if let Some(close) = find_char(&chars, open + 1, ']') {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(end) = find_char(&chars, close + 2, ')') {
                        if !is_image {
                            out.extend(&chars[open + 1..close]);
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn strip_line_prefix(line: &str) -> &str {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(char::is_whitespace) {
        return line[hashes..].trim_start();
    }
    if let Some(rest) = line.strip_prefix('>') {
        return rest.strip_prefix(' ').unwrap_or(rest);
    }
    if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return rest.trim_start();
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && line[digits..].starts_with(". ") {
        return line[digits + 2..].trim_start();
    }
    line
}

pub fn plain_len(markdown: &str) -> usize {
    let stripped = strip_links(markdown);
    let joined = stripped
        .lines()
        .map(strip_line_prefix)
        .collect::<Vec<_>>()
        .join("\n");
    joined
        .chars()
        .filter(|c| !matches!(c, '`' | '*' | '_' | '~'))
        .collect::<String>()
        .trim()
        .chars()
        .count()
}

/// CMS 에디터처럼 짧은 글이 한도를 넘으면 **published로는 저장하지 못한다**(draft는 된다). 이유를 돌려준다.
pub fn violation(tags: &[Tag], published_at: &str, content: &str, status: &str) -> Option<String> {
    if status != "published" || !is_short_form(tags, published_at) {
        return None;
    }
    let len = plain_len(content);
    (len > SHORT_FORM_LIMIT).then(|| {
        format!(
            "lovelog 짧은 글은 {SHORT_FORM_LIMIT}자까지만 발행할 수 있습니다(지금 {len}자). draft로는 저장할 수 있습니다"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(category: &str, tag: &str) -> Tag {
        Tag { tag: tag.into(), category: category.into() }
    }

    #[test]
    fn plain_len_matches_lovelog_rules() {
        assert_eq!(plain_len("**해니**가 [여기](https://x.com)서 웃었다."), "해니가 여기서 웃었다.".chars().count());
        assert_eq!(plain_len("![](https://tyange.com/api/cms/images/a.png)"), 0);
        assert_eq!(
            plain_len("# 제목\n\n- 하나\n- 둘\n\n> 인용\n\n1. 첫째"),
            "제목\n\n하나\n둘\n\n인용\n\n첫째".chars().count()
        );
        assert_eq!(plain_len("  가나다  "), 3);
    }

    #[test]
    fn only_lovelog_posts_since_the_cutoff_are_short_form() {
        assert!(is_short_form(&[tag("general", "lovelog")], "2026-10-15"));
        assert!(is_short_form(&[tag("general", "love")], SHORT_FORM_SINCE));
        assert!(!is_short_form(&[tag("general", "lovelog")], "2026-09-22"));
        assert!(!is_short_form(&[tag("dev", "lovelog")], "2026-10-15"), "카테고리가 general이어야 한다");
        assert!(!is_short_form(&[tag("general", "rust")], "2026-10-15"));
        assert!(!is_short_form(&[], "2026-10-15"));
    }

    #[test]
    fn a_long_short_form_post_cannot_be_published_but_can_be_a_draft() {
        let tags = [tag("general", "lovelog")];
        let long = "가".repeat(SHORT_FORM_LIMIT + 1);
        let ok = "가".repeat(SHORT_FORM_LIMIT);
        assert!(violation(&tags, "2026-10-15", &long, "published").is_some());
        assert!(violation(&tags, "2026-10-15", &long, "draft").is_none());
        assert!(violation(&tags, "2026-10-15", &ok, "published").is_none());
        assert!(violation(&[tag("dev", "rust")], "2026-10-15", &long, "published").is_none());
    }

    #[test]
    fn non_ascii_input_does_not_panic() {
        assert_eq!(plain_len("[가나다]라마바"), "[가나다]라마바".chars().count());
    }
}
