use crate::rng::Rng;
use model::Species;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    /// Wrong answers are random birds of the pool.
    Easy,
    /// Half look-alikes, half random.
    Normal,
    /// Wrong answers are the closest relatives (same genus, then family).
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub target: i64,
    /// Species ids, shuffled, containing `target`.
    pub options: Vec<i64>,
}

impl Question {
    pub fn is_correct(&self, species_id: i64) -> bool {
        species_id == self.target
    }
}

fn genus(s: &Species) -> &str {
    s.scientific.split_whitespace().next().unwrap_or("")
}

/// 2 = same genus, 1 = same family, 0 = unrelated.
fn similarity(a: &Species, b: &Species) -> u8 {
    if genus(a) == genus(b) {
        2
    } else if !a.family.is_empty() && a.family == b.family {
        1
    } else {
        0
    }
}

/// Picks `n_options - 1` wrong answers for `target` from `pool`.
fn distractors<'a>(
    target: &Species,
    pool: &'a [Species],
    n: usize,
    difficulty: Difficulty,
    rng: &mut Rng,
) -> Vec<&'a Species> {
    let mut others: Vec<&Species> = pool.iter().filter(|s| s.id != target.id).collect();
    rng.shuffle(&mut others);
    let take_similar = match difficulty {
        Difficulty::Easy => 0,
        Difficulty::Normal => n / 2,
        Difficulty::Hard => n,
    };
    // Stable sort keeps the shuffle order among equally similar birds.
    let mut by_sim = others.clone();
    by_sim.sort_by_key(|s| std::cmp::Reverse(similarity(target, s)));
    let mut picked: Vec<&Species> = by_sim.into_iter().take(take_similar).collect();
    for s in others {
        if picked.len() == n {
            break;
        }
        if !picked.iter().any(|p| p.id == s.id) {
            picked.push(s);
        }
    }
    picked
}

/// Builds one question per target, each with `n_options` shuffled choices.
pub fn generate(
    targets: &[i64],
    pool: &[Species],
    n_options: usize,
    difficulty: Difficulty,
    rng: &mut Rng,
) -> Vec<Question> {
    targets
        .iter()
        .filter_map(|&id| {
            let target = pool.iter().find(|s| s.id == id)?;
            let mut options: Vec<i64> =
                distractors(target, pool, n_options.saturating_sub(1), difficulty, rng)
                    .iter()
                    .map(|s| s.id)
                    .collect();
            options.push(id);
            rng.shuffle(&mut options);
            Some(Question { target: id, options })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(id: i64, sci: &str, family: &str) -> Species {
        Species {
            id,
            scientific: sci.into(),
            name_en: format!("en{id}"),
            name_fr: format!("fr{id}"),
            family: family.into(),
            order: String::new(),
        }
    }

    fn pool() -> Vec<Species> {
        vec![
            sp(1, "Parus major", "Paridae"),
            sp(2, "Parus ater", "Paridae"),
            sp(3, "Cyanistes caeruleus", "Paridae"),
            sp(4, "Turdus merula", "Turdidae"),
            sp(5, "Anas platyrhynchos", "Anatidae"),
            sp(6, "Ardea cinerea", "Ardeidae"),
        ]
    }

    #[test]
    fn options_contain_target_without_duplicates() {
        let mut rng = Rng::new(7);
        for q in generate(&[1, 4, 6], &pool(), 4, Difficulty::Normal, &mut rng) {
            assert_eq!(q.options.len(), 4);
            assert!(q.options.contains(&q.target));
            let mut o = q.options.clone();
            o.sort();
            o.dedup();
            assert_eq!(o.len(), 4);
        }
    }

    #[test]
    fn hard_picks_relatives_first() {
        let mut rng = Rng::new(3);
        let q = &generate(&[1], &pool(), 3, Difficulty::Hard, &mut rng)[0];
        // Same genus (2) then same family (3) fill the two wrong slots.
        assert!(q.options.contains(&2) && q.options.contains(&3));
    }

    #[test]
    fn small_pool_gives_fewer_options() {
        let mut rng = Rng::new(1);
        let q = &generate(&[1], &pool()[..2], 4, Difficulty::Easy, &mut rng)[0];
        assert_eq!(q.options.len(), 2);
    }

    #[test]
    fn unknown_target_is_skipped() {
        let mut rng = Rng::new(1);
        assert!(generate(&[99], &pool(), 4, Difficulty::Easy, &mut rng).is_empty());
    }
}
