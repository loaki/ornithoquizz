//! Leitner boxes: a correct answer moves a bird up a box (reviewed less often),
//! a wrong answer sends it back to box 0.

pub const MAX_BOX: u8 = 4;
/// Days to wait before a bird in this box is due again.
const INTERVAL_DAYS: [u32; 5] = [0, 1, 3, 7, 21];

pub fn next_box(current: u8, correct: bool) -> u8 {
    if correct {
        (current + 1).min(MAX_BOX)
    } else {
        0
    }
}

pub fn is_due(box_: u8, last_seen_day: u32, today: u32) -> bool {
    today.saturating_sub(last_seen_day) >= INTERVAL_DAYS[box_.min(MAX_BOX) as usize]
}

/// Higher = should be asked sooner. Unseen birds count as box 0, overdue ones rank higher.
pub fn priority(box_: u8, last_seen_day: Option<u32>, today: u32) -> u32 {
    let Some(last) = last_seen_day else { return 1000 };
    let overdue = today
        .saturating_sub(last)
        .saturating_sub(INTERVAL_DAYS[box_.min(MAX_BOX) as usize]);
    (MAX_BOX - box_.min(MAX_BOX)) as u32 * 100 + overdue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_move() {
        assert_eq!(next_box(0, true), 1);
        assert_eq!(next_box(MAX_BOX, true), MAX_BOX);
        assert_eq!(next_box(3, false), 0);
    }

    #[test]
    fn due_dates() {
        assert!(is_due(0, 10, 10));
        assert!(!is_due(2, 10, 12));
        assert!(is_due(2, 10, 13));
    }

    #[test]
    fn unseen_first_then_weak_boxes() {
        assert!(priority(0, None, 5) > priority(0, Some(0), 5));
        assert!(priority(0, Some(0), 5) > priority(4, Some(0), 5));
    }
}
