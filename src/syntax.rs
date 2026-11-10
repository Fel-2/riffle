use std::path::Path;

use crate::filesystem::FileClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Python,
    JavaScript,
    Go,
    C,
    Cpp,
    Java,
    Ruby,
    Php,
    Lua,
    Shell,
    Toml,
    Json,
    Yaml,
    Markdown,
    Html,
    Css,
    Sql,
    Plain,
}

impl Lang {
    pub fn from_path(path: &Path, class: FileClass) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if matches!(
            name.as_str(),
            "makefile" | "dockerfile" | "justfile" | "rakefile" | "gemfile"
        ) {
            return Self::Shell;
        }
        if name.starts_with(".env") || name.ends_with("rc") {
            return Self::Shell;
        }
        match ext.as_str() {
            "rs" => Self::Rust,
            "py" | "pyi" => Self::Python,
            "js" | "mjs" | "cjs" | "jsx" => Self::JavaScript,
            "ts" | "tsx" => Self::JavaScript,
            "go" => Self::Go,
            "c" | "h" => Self::C,
            "cc" | "cpp" | "cxx" | "hpp" | "hxx" => Self::Cpp,
            "java" | "kt" | "kts" | "scala" | "cs" | "swift" | "dart" => Self::Java,
            "rb" => Self::Ruby,
            "php" => Self::Php,
            "lua" => Self::Lua,
            "sh" | "bash" | "zsh" | "fish" | "env" => Self::Shell,
            "toml" | "ini" | "conf" | "cfg" | "desktop" | "service" | "lock" => Self::Toml,
            "json" | "jsonc" => Self::Json,
            "yaml" | "yml" => Self::Yaml,
            "md" | "markdown" | "rst" => Self::Markdown,
            "html" | "htm" | "xml" | "svg" | "vue" | "svelte" => Self::Html,
            "css" | "scss" | "sass" | "less" => Self::Css,
            "sql" => Self::Sql,
            _ => match class {
                FileClass::Text => Self::Plain,
                FileClass::Config => Self::Toml,
                FileClass::Code => Self::Plain,
                _ => Self::Plain,
            },
        }
    }

    fn line_comment(self) -> Option<&'static str> {
        match self {
            Self::Rust | Self::JavaScript | Self::Go | Self::C | Self::Cpp | Self::Java => {
                Some("//")
            }
            Self::Python | Self::Shell | Self::Toml | Self::Yaml => Some("#"),
            Self::Ruby | Self::Php => Some("#"),
            Self::Lua | Self::Sql => Some("--"),
            _ => None,
        }
    }

    fn block_comment(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Rust
            | Self::JavaScript
            | Self::Go
            | Self::C
            | Self::Cpp
            | Self::Java
            | Self::Css
            | Self::Php => Some(("/*", "*/")),
            Self::Html => Some(("<!--", "-->")),
            _ => None,
        }
    }

    fn is_config(self) -> bool {
        matches!(self, Self::Toml | Self::Json | Self::Yaml)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Comment,
    String,
    Number,
    Keyword,
    Type,
    Function,
    Constant,
    Key,
    Operator,
    Heading,
    Link,
    Code,
    Attribute,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
}

pub fn tokenize(lines: &[String], lang: Lang) -> Vec<Vec<Token>> {
    let mut out = Vec::with_capacity(lines.len());
    let mut state = State::default();
    for line in lines {
        out.push(tokenize_line(line, lang, &mut state));
    }
    out
}

#[derive(Default)]
struct State {
    block_end: Option<&'static str>,
    markdown_fence: bool,
}

fn tokenize_line(line: &str, lang: Lang, state: &mut State) -> Vec<Token> {
    if lang == Lang::Markdown {
        return markdown_line(line, state);
    }
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < line.len() {
        let ch = line[pos..].chars().next().unwrap();
        let clen = ch.len_utf8();

        if let Some(end) = state.block_end {
            if let Some(index) = line[pos..].find(end) {
                let end_pos = pos + index + end.len();
                push(&mut tokens, &line[pos..end_pos], TokenKind::Comment);
                state.block_end = None;
                pos = end_pos;
            } else {
                push(&mut tokens, &line[pos..], TokenKind::Comment);
                pos = line.len();
            }
            continue;
        }

        if let Some((open, close)) = lang.block_comment() {
            if line[pos..].starts_with(open) {
                let after = pos + open.len();
                if let Some(index) = line[after..].find(close) {
                    let end_pos = after + index + close.len();
                    push(&mut tokens, &line[pos..end_pos], TokenKind::Comment);
                    pos = end_pos;
                } else {
                    push(&mut tokens, &line[pos..], TokenKind::Comment);
                    state.block_end = Some(close);
                    pos = line.len();
                }
                continue;
            }
        }

        if let Some(marker) = lang.line_comment() {
            if line[pos..].starts_with(marker) {
                push(&mut tokens, &line[pos..], TokenKind::Comment);
                break;
            }
        }

        if ch == '"' || ch == '\'' || (ch == '`' && lang == Lang::JavaScript) {
            let end = scan_string(line, pos, ch);
            let kind = if lang.is_config()
                && next_significant(line, end).is_some_and(|c| c == ':' || c == '=')
            {
                TokenKind::Key
            } else {
                TokenKind::String
            };
            push(&mut tokens, &line[pos..end], kind);
            pos = end;
            continue;
        }

        if ch.is_ascii_digit() {
            let end = scan_number(line, pos);
            push(&mut tokens, &line[pos..end], TokenKind::Number);
            pos = end;
            continue;
        }

        if ch.is_alphabetic() || ch == '_' {
            let end = scan_ident(line, pos);
            let word = &line[pos..end];
            let kind = classify_word(lang, word, line, end);
            push(&mut tokens, word, kind);
            pos = end;
            continue;
        }

        if ch == '@' && matches!(lang, Lang::Rust | Lang::Python) {
            let end = scan_ident(line, pos + 1);
            push(&mut tokens, &line[pos..end], TokenKind::Attribute);
            pos = end;
            continue;
        }

        if ch == '$' && lang == Lang::Shell {
            let end = scan_ident(line, pos + 1);
            push(&mut tokens, &line[pos..end], TokenKind::Constant);
            pos = end;
            continue;
        }

        if ch.is_whitespace() {
            let mut end = pos + clen;
            while end < line.len() {
                let next = line[end..].chars().next().unwrap();
                if !next.is_whitespace() {
                    break;
                }
                end += next.len_utf8();
            }
            push(&mut tokens, &line[pos..end], TokenKind::Plain);
            pos = end;
            continue;
        }

        if is_operator(ch) {
            let mut end = pos + clen;
            while end < line.len() {
                let next = line[end..].chars().next().unwrap();
                if !is_operator(next) {
                    break;
                }
                end += next.len_utf8();
            }
            push(&mut tokens, &line[pos..end], TokenKind::Operator);
            pos = end;
            continue;
        }

        push(&mut tokens, &line[pos..pos + clen], TokenKind::Plain);
        pos += clen;
    }
    if tokens.is_empty() {
        tokens.push(Token {
            text: String::new(),
            kind: TokenKind::Plain,
        });
    }
    tokens
}

fn push(tokens: &mut Vec<Token>, text: &str, kind: TokenKind) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = tokens.last_mut() {
        if last.kind == kind {
            last.text.push_str(text);
            return;
        }
    }
    tokens.push(Token {
        text: text.to_string(),
        kind,
    });
}

fn scan_string(line: &str, start: usize, quote: char) -> usize {
    let mut pos = start + quote.len_utf8();
    while pos < line.len() {
        let ch = line[pos..].chars().next().unwrap();
        pos += ch.len_utf8();
        if ch == '\\' {
            if pos < line.len() {
                let next = line[pos..].chars().next().unwrap();
                pos += next.len_utf8();
            }
            continue;
        }
        if ch == quote {
            break;
        }
    }
    pos
}

fn scan_number(line: &str, start: usize) -> usize {
    let mut pos = start;
    while pos < line.len() {
        let ch = line[pos..].chars().next().unwrap();
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | 'x' | 'X') {
            pos += ch.len_utf8();
        } else {
            break;
        }
    }
    pos
}

fn scan_ident(line: &str, start: usize) -> usize {
    let mut pos = start;
    while pos < line.len() {
        let ch = line[pos..].chars().next().unwrap();
        if ch.is_alphanumeric() || ch == '_' {
            pos += ch.len_utf8();
        } else {
            break;
        }
    }
    pos
}

fn next_significant(line: &str, from: usize) -> Option<char> {
    line[from..].chars().find(|c| !c.is_whitespace())
}

fn classify_word(lang: Lang, word: &str, line: &str, end: usize) -> TokenKind {
    if is_keyword(lang, word) {
        return TokenKind::Keyword;
    }
    if matches!(
        word,
        "true" | "false" | "null" | "nil" | "None" | "True" | "False"
    ) {
        return TokenKind::Constant;
    }
    if lang.is_config() && next_significant(line, end).is_some_and(|c| c == ':' || c == '=') {
        return TokenKind::Key;
    }
    if word.chars().next().is_some_and(|c| c.is_uppercase()) {
        return TokenKind::Type;
    }
    if next_significant(line, end) == Some('(') {
        return TokenKind::Function;
    }
    TokenKind::Plain
}

fn is_operator(ch: char) -> bool {
    matches!(
        ch,
        '+' | '-'
            | '*'
            | '/'
            | '%'
            | '='
            | '!'
            | '<'
            | '>'
            | '&'
            | '|'
            | '^'
            | '~'
            | '?'
            | ':'
            | ';'
            | ','
            | '.'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '#'
    )
}

fn is_keyword(lang: Lang, word: &str) -> bool {
    let list: &[&str] = match lang {
        Lang::Rust => &[
            "fn",
            "let",
            "mut",
            "pub",
            "use",
            "mod",
            "struct",
            "enum",
            "impl",
            "trait",
            "for",
            "while",
            "loop",
            "if",
            "else",
            "match",
            "return",
            "self",
            "Self",
            "const",
            "static",
            "async",
            "await",
            "move",
            "ref",
            "where",
            "type",
            "crate",
            "super",
            "dyn",
            "unsafe",
            "extern",
            "as",
            "in",
            "break",
            "continue",
            "yield",
            "macro_rules",
        ],
        Lang::Python => &[
            "def", "class", "import", "from", "as", "if", "elif", "else", "for", "while", "return",
            "try", "except", "finally", "with", "lambda", "pass", "raise", "yield", "async",
            "await", "global", "nonlocal", "del", "not", "and", "or", "is", "in",
        ],
        Lang::JavaScript => &[
            "function",
            "const",
            "let",
            "var",
            "class",
            "import",
            "export",
            "default",
            "if",
            "else",
            "for",
            "while",
            "return",
            "async",
            "await",
            "try",
            "catch",
            "finally",
            "new",
            "this",
            "typeof",
            "instanceof",
            "interface",
            "type",
            "enum",
            "extends",
            "implements",
            "public",
            "private",
            "readonly",
            "static",
            "from",
            "of",
            "in",
            "switch",
            "case",
            "break",
            "continue",
            "throw",
            "delete",
            "void",
            "yield",
        ],
        Lang::Go => &[
            "func",
            "package",
            "import",
            "var",
            "const",
            "type",
            "struct",
            "interface",
            "map",
            "chan",
            "go",
            "defer",
            "if",
            "else",
            "for",
            "range",
            "return",
            "switch",
            "case",
            "default",
            "break",
            "continue",
            "select",
        ],
        Lang::C | Lang::Cpp => &[
            "include",
            "define",
            "ifdef",
            "ifndef",
            "endif",
            "int",
            "char",
            "void",
            "struct",
            "enum",
            "union",
            "typedef",
            "static",
            "const",
            "return",
            "for",
            "while",
            "do",
            "if",
            "else",
            "switch",
            "case",
            "break",
            "continue",
            "class",
            "public",
            "private",
            "protected",
            "namespace",
            "template",
            "typename",
            "using",
            "new",
            "delete",
            "auto",
            "unsigned",
            "signed",
            "long",
            "short",
            "float",
            "double",
            "bool",
            "extern",
        ],
        Lang::Java => &[
            "public",
            "private",
            "protected",
            "class",
            "interface",
            "extends",
            "implements",
            "static",
            "final",
            "void",
            "int",
            "long",
            "double",
            "float",
            "boolean",
            "char",
            "new",
            "return",
            "if",
            "else",
            "for",
            "while",
            "try",
            "catch",
            "finally",
            "import",
            "package",
            "abstract",
            "synchronized",
            "throws",
            "throw",
            "super",
            "this",
            "enum",
            "var",
            "fun",
            "val",
            "when",
            "object",
            "data",
        ],
        Lang::Ruby => &[
            "def",
            "class",
            "module",
            "end",
            "if",
            "elsif",
            "else",
            "unless",
            "while",
            "until",
            "for",
            "in",
            "do",
            "return",
            "yield",
            "require",
            "attr_accessor",
            "attr_reader",
            "self",
            "then",
            "begin",
            "rescue",
            "ensure",
        ],
        Lang::Php => &[
            "function",
            "class",
            "public",
            "private",
            "protected",
            "static",
            "return",
            "if",
            "else",
            "elseif",
            "foreach",
            "for",
            "while",
            "echo",
            "new",
            "namespace",
            "use",
            "fn",
            "match",
            "require",
            "include",
        ],
        Lang::Lua => &[
            "function", "local", "end", "if", "then", "else", "elseif", "for", "while", "do",
            "return", "require", "repeat", "until", "break",
        ],
        Lang::Shell => &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
            "function", "export", "local", "return", "echo", "cd", "set", "unset", "source",
            "alias", "in", "select", "until",
        ],
        Lang::Sql => &[
            "select", "from", "where", "insert", "into", "update", "delete", "create", "table",
            "drop", "alter", "join", "on", "group", "by", "order", "limit", "values", "set", "and",
            "or", "not", "null", "as", "distinct", "having", "union", "index",
        ],
        Lang::Toml | Lang::Json | Lang::Yaml => &[],
        _ => &[],
    };
    list.contains(&word) || list.contains(&word.to_ascii_lowercase().as_str())
}

fn markdown_line(line: &str, state: &mut State) -> Vec<Token> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        state.markdown_fence = !state.markdown_fence;
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Code,
        }];
    }
    if state.markdown_fence {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Code,
        }];
    }
    if trimmed.starts_with('#') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Heading,
        }];
    }
    if trimmed.starts_with('>') {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }
    if trimmed.starts_with("---") || trimmed.starts_with("===") {
        return vec![Token {
            text: line.to_string(),
            kind: TokenKind::Comment,
        }];
    }

    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < line.len() {
        let ch = line[pos..].chars().next().unwrap();
        if ch == '`' {
            let end = line[pos + 1..]
                .find('`')
                .map(|i| pos + 1 + i + 1)
                .unwrap_or(line.len());
            push(&mut tokens, &line[pos..end], TokenKind::Code);
            pos = end;
            continue;
        }
        if ch == '*' && line[pos..].starts_with("**") {
            let end = line[pos + 2..]
                .find("**")
                .map(|i| pos + 2 + i + 2)
                .unwrap_or(line.len());
            push(&mut tokens, &line[pos..end], TokenKind::Heading);
            pos = end;
            continue;
        }
        if ch == '[' {
            if let Some(close) = line[pos..].find(']') {
                let after = pos + close + 1;
                if line[after..].starts_with('(') {
                    if let Some(end) = line[after..].find(')') {
                        let end = after + end + 1;
                        push(&mut tokens, &line[pos..end], TokenKind::Link);
                        pos = end;
                        continue;
                    }
                }
            }
        }
        let clen = ch.len_utf8();
        push(&mut tokens, &line[pos..pos + clen], TokenKind::Plain);
        pos += clen;
    }
    if tokens.is_empty() {
        tokens.push(Token {
            text: String::new(),
            kind: TokenKind::Plain,
        });
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str, lang: Lang) -> Vec<TokenKind> {
        tokenize(&[line.to_string()], lang)
            .remove(0)
            .into_iter()
            .filter(|t| !t.text.trim().is_empty())
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn detects_languages() {
        assert_eq!(
            Lang::from_path(Path::new("a.rs"), FileClass::Code),
            Lang::Rust
        );
        assert_eq!(
            Lang::from_path(Path::new("a.py"), FileClass::Code),
            Lang::Python
        );
        assert_eq!(
            Lang::from_path(Path::new("a.toml"), FileClass::Config),
            Lang::Toml
        );
        assert_eq!(
            Lang::from_path(Path::new("a.md"), FileClass::Text),
            Lang::Markdown
        );
    }

    #[test]
    fn rust_line_comments_and_keywords() {
        let kinds = kinds("let x = 1; // note", Lang::Rust);
        assert!(kinds.contains(&TokenKind::Keyword));
        assert!(kinds.contains(&TokenKind::Number));
        assert_eq!(*kinds.last().unwrap(), TokenKind::Comment);
    }

    #[test]
    fn strings_are_single_tokens() {
        let tokens = tokenize(&["let s = \"a // b\";".to_string()], Lang::Rust).remove(0);
        let string = tokens.iter().find(|t| t.kind == TokenKind::String).unwrap();
        assert_eq!(string.text, "\"a // b\"");
    }

    #[test]
    fn block_comments_span_lines() {
        let lines = vec!["/* one".to_string(), "two */ let x = 1;".to_string()];
        let tokens = tokenize(&lines, Lang::Rust);
        assert!(tokens[0].iter().all(|t| t.kind == TokenKind::Comment));
        assert!(tokens[1].iter().any(|t| t.kind == TokenKind::Keyword));
    }

    #[test]
    fn config_keys_are_highlighted() {
        let toml = kinds("name = \"riffle\"", Lang::Toml);
        assert_eq!(toml[0], TokenKind::Key);
        let json = kinds("\"name\": \"riffle\"", Lang::Json);
        assert_eq!(json[0], TokenKind::Key);
    }

    #[test]
    fn markdown_headings_and_links() {
        let heading = kinds("# Title", Lang::Markdown);
        assert_eq!(heading[0], TokenKind::Heading);
        let link = kinds("see [docs](https://x)", Lang::Markdown);
        assert!(link.contains(&TokenKind::Link));
    }

    #[test]
    fn unicode_does_not_panic() {
        let _ = tokenize(&["let 日本 = \"値\";".to_string()], Lang::Rust);
        let _ = tokenize(&["# 日本語".to_string()], Lang::Markdown);
    }
}
