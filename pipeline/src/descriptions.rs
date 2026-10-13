//! Riddle texts: the "Description" section of each bird's Wikipedia article, with its own names masked.

use crate::{http::Client, species::SpeciesRow};
use anyhow::Result;
use model::{Description, Lang};
use regex::{Regex, RegexBuilder};

const MASK: &str = "…";
const MIN_CHARS: usize = 120;
const MAX_CHARS: usize = 600;
const MAX_MASKS: usize = 4;

/// Section titles that describe the bird's appearance, per language.
const HEADINGS_EN: &[&str] = &["description", "appearance", "morphology"];
const HEADINGS_FR: &[&str] = &["description", "morphologie", "caractéristiques", "description physique", "caractéristiques physiques"];

pub fn fetch(http: &Client, species: &[SpeciesRow]) -> Result<Vec<Description>> {
    let mut out = Vec::new();
    let (mut no_page, mut no_section, mut rejected) = (0, 0, 0);
    for (i, sp) in species.iter().enumerate() {
        for (lang, code, headings) in [(Lang::En, "en", HEADINGS_EN), (Lang::Fr, "fr", HEADINGS_FR)] {
            let title = sp.scientific.replace(' ', "_");
            let url = format!(
                "https://{code}.wikipedia.org/w/api.php?action=query&prop=extracts|info&inprop=url\
                 &explaintext=1&redirects=1&format=json&titles={title}"
            );
            // A missing page is normal: skip that language for this bird.
            let json = match http.get_json(&url) {
                Ok(j) => j,
                Err(e) => {
                    no_page += 1;
                    if no_page <= 3 {
                        eprintln!("  fetch failed: {e:#}");
                    }
                    continue;
                }
            };
            let Some(page) = json["query"]["pages"].as_object().and_then(|p| p.values().next()) else { continue };
            let Some(full) = page["extract"].as_str() else { continue };
            let Some(section) = section(full, headings) else { no_section += 1; continue };
            let Some(text) = riddle(&section, sp) else { rejected += 1; continue };
            {
                out.push(Description {
                    species_id: sp.taxon_id,
                    lang,
                    text,
                    source_url: page["fullurl"].as_str().unwrap_or_default().to_string(),
                });
            }
        }
        if (i + 1) % 25 == 0 {
            println!("[{}/{}] {} descriptions so far", i + 1, species.len(), out.len());
        }
    }
    println!("skipped: {no_page} no page, {no_section} no description section, {rejected} too short / over-masked");
    Ok(out)
}

/// Body of the first section whose heading is in `headings` (`== Title ==` lines in plain-text extracts).
fn section(full: &str, headings: &[&str]) -> Option<String> {
    let heading = Regex::new(r"^(=+)\s*(.*?)\s*=+\s*$").ok()?;
    let mut inside: Option<usize> = None;
    let mut body = String::new();
    for line in full.lines() {
        if let Some(c) = heading.captures(line) {
            let level = c[1].len();
            let title = c[2].to_lowercase();
            match inside {
                // A heading of the same or higher level ends the section; deeper ones belong to it.
                Some(l) if level <= l => break,
                Some(_) => continue,
                None if headings.contains(&title.as_str()) => inside = Some(level),
                None => {}
            }
        } else if inside.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }
    let body = body.trim().to_string();
    (!body.is_empty()).then_some(body)
}

/// Masks the bird's names and keeps the first sentences up to `MAX_CHARS`.
/// `None` if too short, or if so many names were hidden that the text is useless.
fn riddle(section: &str, sp: &SpeciesRow) -> Option<String> {
    let genus = sp.scientific.split_whitespace().next().unwrap_or_default();
    let epithet = sp.scientific.split_whitespace().nth(1).unwrap_or_default();
    let mut names: Vec<String> = vec![sp.name_en.clone(), sp.name_fr.clone(), sp.scientific.clone(), genus.to_string()];
    // Short forms: "robin" for "European Robin", "rougegorge" for "Rougegorge familier".
    for name in [&sp.name_en, &sp.name_fr] {
        if let Some(w) = name.split(|c: char| !c.is_alphanumeric()).next_back().filter(|_| name.contains(' ') || name.contains('-')) {
            names.push(w.to_string());
        }
        if let Some(w) = name.split(|c: char| !c.is_alphanumeric()).next() {
            names.push(w.to_string());
        }
    }
    if epithet.chars().count() >= 5 {
        names.push(epithet.to_string());
    }
    names.retain(|n| n.chars().count() >= 4);
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    names.dedup();
    let pattern = names.iter().map(|n| regex::escape(n)).collect::<Vec<_>>().join("|");
    let abbreviated = format!(r"\b{}\.\s*{}", regex::escape(&genus.chars().take(1).collect::<String>()), regex::escape(epithet));
    let re: Regex = RegexBuilder::new(&format!(r"\b({pattern})(s|x)?\b|{abbreviated}")).case_insensitive(true).build().ok()?;
    let parens = Regex::new(r"\s*\([^)]*\)").ok()?;
    let refs = Regex::new(r"\[\d+\]").ok()?;

    let flat = refs.replace_all(&parens.replace_all(&section.replace('\n', " "), ""), "").into_owned();
    let mut text = String::new();
    for s in sentences(&flat).into_iter().filter(|s| s.chars().count() > 20) {
        if !text.is_empty() && text.chars().count() + s.chars().count() > MAX_CHARS {
            break;
        }
        text.push_str(s);
        text.push(' ');
    }
    let text = text.trim().to_string();
    if text.chars().count() < MIN_CHARS || re.find_iter(&text).count() > MAX_MASKS {
        return None;
    }
    Some(re.replace_all(&text, MASK).into_owned())
}

/// Splits after `.`, `!` or `?` only when followed by a space and a capital: keeps "25.1 cm" and "P. major" intact.
fn sentences(text: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut start = 0;
    for w in 0..chars.len() {
        let (i, c) = chars[w];
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let next_is_start = chars.get(w + 1).is_some_and(|(_, c)| c.is_whitespace())
            && chars.get(w + 2).is_some_and(|(_, c)| c.is_uppercase());
        // "P. major": a single capital letter before the dot is an initial, not a sentence end.
        let initial = w >= 1 && chars[w - 1].1.is_uppercase() && (w < 2 || !chars[w - 2].1.is_alphabetic());
        if next_is_start && !initial {
            out.push(text[start..i + c.len_utf8()].trim());
            start = i + c.len_utf8();
        }
    }
    if start < text.len() {
        out.push(text[start..].trim());
    }
    out
}
