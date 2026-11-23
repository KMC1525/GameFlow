//! Verificações estáticas feitas pelo agente Testador (não executam o jogo).

use std::path::Path;

/// Confere se parênteses/colchetes/chaves estão balanceados, ignorando strings, comentários e regex simples.
/// Não é um parser de JS completo, mas pega a maioria dos cortes/truncamentos típicos de saída de LLM.
pub fn js_balanced(src: &str) -> Result<(), String> {
    let chars: Vec<char> = src.chars().collect();
    let mut stack: Vec<(char, usize)> = vec![];
    let (mut i, mut line) = (0usize, 1usize);
    let mut last_sig: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
                if i + 1 >= chars.len() {
                    return Err(format!("comentário /* não fechado (linha {line})"));
                }
                i += 2;
            }
            '"' | '\'' => {
                let q = c;
                let start = line;
                i += 1;
                while i < chars.len() && chars[i] != q {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    if i < chars.len() && chars[i] == '\n' {
                        return Err(format!("texto entre aspas não fechado (linha {start})"));
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(format!("texto entre aspas não fechado (linha {start})"));
                }
                i += 1;
                last_sig = Some(q);
            }
            '`' => {
                // template literal: ignora `${ }` aninhado de forma simples
                let start = line;
                i += 1;
                let mut depth = 0;
                while i < chars.len() {
                    match chars[i] {
                        '\\' => i += 1,
                        '\n' => line += 1,
                        '$' if chars.get(i + 1) == Some(&'{') => {
                            depth += 1;
                            i += 1;
                        }
                        '}' if depth > 0 => depth -= 1,
                        '`' if depth == 0 => break,
                        _ => {}
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(format!("template literal não fechado (linha {start})"));
                }
                i += 1;
                last_sig = Some('`');
            }
            '/' if matches!(last_sig, None | Some('(') | Some(',') | Some('=') | Some(':') | Some('[') | Some('!') | Some('&') | Some('|') | Some('?') | Some('{') | Some('}') | Some(';')) => {
                // literal de regex
                i += 1;
                let mut in_class = false;
                while i < chars.len() && (chars[i] != '/' || in_class) {
                    match chars[i] {
                        '\\' => i += 1,
                        '[' => in_class = true,
                        ']' => in_class = false,
                        '\n' => break,
                        _ => {}
                    }
                    i += 1;
                }
                i += 1;
                last_sig = Some('/');
            }
            '(' | '[' | '{' => {
                stack.push((c, line));
                last_sig = Some(c);
                i += 1;
            }
            ')' | ']' | '}' => {
                let want = match c {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                match stack.pop() {
                    Some((o, _)) if o == want => {}
                    Some((o, l)) => return Err(format!("`{c}` na linha {line} não combina com `{o}` aberto na linha {l}")),
                    None => return Err(format!("`{c}` sobrando na linha {line}")),
                }
                last_sig = Some(c);
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            _ => {
                last_sig = Some(c);
                i += 1;
            }
        }
    }
    match stack.pop() {
        Some((o, l)) => Err(format!("`{o}` aberto na linha {l} nunca foi fechado")),
        None => Ok(()),
    }
}

/// Referências locais (`src=`/`href=`) de um HTML.
pub fn html_local_refs(html: &str) -> Vec<String> {
    let mut out = vec![];
    for attr in ["src=\"", "href=\""] {
        let mut rest = html;
        while let Some(i) = rest.find(attr) {
            rest = &rest[i + attr.len()..];
            if let Some(j) = rest.find('"') {
                let v = &rest[..j];
                if !v.starts_with("http") && !v.starts_with("//") && !v.starts_with('#') && !v.starts_with("data:") && !v.is_empty() {
                    out.push(v.split(['?', '#']).next().unwrap_or(v).to_string());
                }
                rest = &rest[j..];
            }
        }
    }
    out
}

/// Usa `node --check` quando o Node.js está disponível (verificação sintática real).
pub async fn node_check(node: &str, file: &Path) -> Option<Result<(), String>> {
    let out = tokio::process::Command::new(node).arg("--check").arg(file).stdin(std::process::Stdio::null()).output().await.ok()?;
    if out.status.success() {
        Some(Ok(()))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg: Vec<&str> = err.lines().filter(|l| !l.trim().is_empty()).take(4).collect();
        Some(Err(msg.join(" | ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_ok() {
        let src = r#"
            // comentário com { não fechado
            const s = "texto com } e ' ";
            const t = `a ${ {x:1}.x } b`;
            const r = /[}{]+/g.test('x');
            function f(a) { return [a, (a + 1)]; }
        "#;
        assert_eq!(js_balanced(src), Ok(()));
    }

    #[test]
    fn detects_truncated_code() {
        assert!(js_balanced("function f() { if (x) {").is_err());
        assert!(js_balanced("const a = [1,2;").is_err());
        assert!(js_balanced("x = 'abc").is_err());
        assert!(js_balanced("}").is_err());
    }

    #[test]
    fn division_is_not_regex() {
        assert_eq!(js_balanced("const a = (b / 2) + c / 3; if (x) { y = a / b; }"), Ok(()));
    }

    #[test]
    fn html_refs() {
        let h = r##"<link href="style.css"><script src="js/a.js"></script><script src="https://x/y.js"></script><a href="#top">"##;
        assert_eq!(html_local_refs(h), vec!["js/a.js", "style.css"]);
    }
}
