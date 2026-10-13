//! Typed-answer checking: ignores case, accents, punctuation and tolerates small typos.

fn fold(c: char) -> Option<char> {
    let c = match c {
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'œ' => 'o',
        other => other,
    };
    c.is_alphanumeric().then_some(c)
}

pub fn normalize(s: &str) -> String {
    s.to_lowercase().chars().filter_map(fold).collect()
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// True when `input` matches any accepted name (EN, FR, Latin...), allowing roughly
/// one typo per 5 letters.
pub fn is_match(input: &str, accepted: &[&str]) -> bool {
    let input: Vec<char> = normalize(input).chars().collect();
    if input.is_empty() {
        return false;
    }
    accepted.iter().any(|name| {
        let name: Vec<char> = normalize(name).chars().collect();
        edit_distance(&input, &name) <= name.len() / 5
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_case_and_accents() {
        assert!(is_match("heron cendre", &["Héron cendré"]));
        assert!(is_match("  GREY-heron ", &["Grey Heron"]));
    }

    #[test]
    fn tolerates_typos_but_not_other_birds() {
        assert!(is_match("Rougegorge famillier", &["Rougegorge familier"]));
        assert!(!is_match("Mésange bleue", &["Mésange charbonnière"]));
        assert!(!is_match("", &["Merle noir"]));
    }

    #[test]
    fn short_names_need_exact_match() {
        assert!(!is_match("Geai", &["Pipit"]));
    }
}
