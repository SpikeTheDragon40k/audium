//! Natural ("human") ordering: digit runs compare by value, so `2 < 10`.

use std::cmp::Ordering;

/// Compares two strings naturally and case-insensitively. Ties are broken by
/// the exact strings so the order is total and deterministic.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    natural_cmp_folded(a, b).then_with(|| a.cmp(b))
}

fn natural_cmp_folded(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let da = take_digits(&mut a);
                let db = take_digits(&mut b);
                let ord = compare_digit_runs(&da, &db);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn take_digits(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
        digits.push(c);
        it.next();
    }
    digits
}

/// Compares digit runs by value without parsing (no overflow on long runs).
/// Equal values with different zero padding order the shorter run first.
fn compare_digit_runs(a: &str, b: &str) -> Ordering {
    let ta = a.trim_start_matches('0');
    let tb = b.trim_start_matches('0');
    ta.len()
        .cmp(&tb.len())
        .then_with(|| ta.cmp(tb))
        .then_with(|| a.len().cmp(&b.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut v: Vec<&str>) -> Vec<&str> {
        v.sort_by(|a, b| natural_cmp(a, b));
        v
    }

    #[test]
    fn numbers_compare_by_value() {
        assert_eq!(
            sorted(vec!["Track 10", "Track 2", "Track 1"]),
            vec!["Track 1", "Track 2", "Track 10"]
        );
        assert_eq!(natural_cmp("2", "10"), Ordering::Less);
    }

    #[test]
    fn zero_padding_and_case() {
        assert_eq!(sorted(vec!["02", "1", "010"]), vec!["1", "02", "010"]);
        assert_eq!(natural_cmp("01", "1"), Ordering::Greater);
        assert_eq!(sorted(vec!["b", "A", "a"]), vec!["A", "a", "b"]);
    }

    #[test]
    fn mixed_and_unicode() {
        assert_eq!(
            sorted(vec!["Disc 2/01", "Disc 10/01", "Disc 2/10", "Disc 2/2"]),
            vec!["Disc 2/01", "Disc 2/2", "Disc 2/10", "Disc 10/01"]
        );
        assert_eq!(natural_cmp("Énigme 3", "énigme 12"), Ordering::Less);
        assert_eq!(natural_cmp("", "a"), Ordering::Less);
    }

    #[test]
    fn very_long_digit_runs_do_not_overflow() {
        let a = "9".repeat(100);
        let b = format!("1{}", "0".repeat(100));
        assert_eq!(natural_cmp(&a, &b), Ordering::Less);
    }
}
