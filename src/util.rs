//! Utilitários pequenos e sem dependências externas: PRNG, slugs, parsing de JSON "sujo" vindo de LLMs etc.

use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// PRNG determinístico (SplitMix64). Mesma semente => mesmo jogo/assets.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    #[cfg(test)]
    pub fn from_str(s: &str) -> Self {
        Rng::new(hash_str(s))
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Número em [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Inteiro em [lo, hi] (inclusivo).
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

/// FNV-1a 64 bits.
pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// "Minha Aventura Épica!" -> "minha-aventura-epica"
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in s.chars() {
        let c = fold_accent(c);
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "jogo".into() } else { out.chars().take(48).collect() }
}

pub fn fold_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        _ => c,
    }
}

/// Minúsculas e sem acentos: facilita casar palavras-chave em prompts em pt/en.
pub fn normalize(s: &str) -> String {
    s.chars().map(|c| fold_accent(c).to_ascii_lowercase()).collect()
}

/// Verifica se o texto normalizado contém alguma das palavras-chave.
pub fn has_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

/// Junta um caminho relativo do usuário a uma raiz garantindo que não escapa dela.
pub fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.replace('\\', "/");
    let rel = rel.trim_start_matches('/');
    let mut out = root.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

/// Caminho relativo com barras normais (para JSON/URLs).
pub fn rel_slash(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/")
}

/// Extrai o primeiro objeto JSON de um texto (LLMs adoram cercar JSON com ``` e explicações).
pub fn extract_json(text: &str) -> Option<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if v.is_object() || v.is_array() {
            return Some(v);
        }
    }
    let bytes: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '{' {
            // varre até o fecho balanceado respeitando strings
            let (mut depth, mut in_str, mut esc) = (0i32, false, false);
            for j in i..bytes.len() {
                let c = bytes[j];
                if in_str {
                    if esc {
                        esc = false;
                    } else if c == '\\' {
                        esc = true;
                    } else if c == '"' {
                        in_str = false;
                    }
                    continue;
                }
                match c {
                    '"' => in_str = true,
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            let cand: String = bytes[i..=j].iter().collect();
                            if let Ok(v) = serde_json::from_str::<Value>(&cand) {
                                return Some(v);
                            }
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        i += 1;
    }
    None
}

/// Mescla `patch` sobre `base` (objetos recursivamente; arrays/valores substituem). `null` é ignorado.
pub fn deep_merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                if v.is_null() {
                    continue;
                }
                match b.get_mut(k) {
                    Some(bv) => deep_merge(bv, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, p) => {
            if !p.is_null() {
                *b = p.clone();
            }
        }
    }
}

/// Divide uma linha de comando estilo shell em argumentos (aspas simples/duplas e `\`).
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == '\\' && q == '"' && matches!(chars.peek(), Some('"') | Some('\\')) {
                    cur.push(chars.next().unwrap());
                } else {
                    cur.push(c);
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    has = true;
                }
                c if c.is_whitespace() => {
                    if has || !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                        has = false;
                    }
                }
                _ => cur.push(c),
            },
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Cor "#rrggbb" -> (r,g,b)
pub fn parse_hex(c: &str) -> (u8, u8, u8) {
    let c = c.trim().trim_start_matches('#');
    if c.len() >= 6 {
        let p = |i: usize| u8::from_str_radix(&c[i..i + 2], 16).unwrap_or(128);
        (p(0), p(2), p(4))
    } else {
        (128, 128, 128)
    }
}

pub fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Pega o primeiro título razoável de uma frase (máx. n palavras).
pub fn first_words(s: &str, n: usize) -> String {
    s.split_whitespace().take(n).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_works() {
        assert_eq!(slugify("Minha Aventura Épica!"), "minha-aventura-epica");
        assert_eq!(slugify("***"), "jogo");
    }

    #[test]
    fn safe_join_blocks_escape() {
        let root = Path::new("/tmp/p");
        assert!(safe_join(root, "../x").is_none());
        assert!(safe_join(root, "a/../../x").is_none());
        assert_eq!(safe_join(root, "/a/b.txt").unwrap(), Path::new("/tmp/p/a/b.txt"));
        assert_eq!(safe_join(root, "a\\b.txt").unwrap(), Path::new("/tmp/p/a/b.txt"));
    }

    #[test]
    fn extract_json_from_markdown() {
        let t = "Claro! Aqui está:\n```json\n{\"a\": {\"b\": \"}\"}, \"c\": [1,2]}\n```\nEspero que ajude";
        let v = extract_json(t).unwrap();
        assert_eq!(v["a"]["b"], "}");
    }

    #[test]
    fn split_args_quotes() {
        assert_eq!(split_args(r#"sd -p "a red fox" -o 'out file.png' --x"#), vec!["sd", "-p", "a red fox", "-o", "out file.png", "--x"]);
    }

    #[test]
    fn merge_ignores_null() {
        let mut a = serde_json::json!({"a": 1, "b": {"c": 2}});
        deep_merge(&mut a, &serde_json::json!({"a": null, "b": {"d": 3}}));
        assert_eq!(a, serde_json::json!({"a": 1, "b": {"c": 2, "d": 3}}));
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::from_str("x");
        let mut b = Rng::from_str("x");
        assert_eq!(a.next_u64(), b.next_u64());
    }
}
