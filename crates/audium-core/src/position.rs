//! Conversions between a book-wide position and (track, offset).
//!
//! Positions are stored as track + offset; the book-wide position is derived
//! from the track durations.

/// Book-wide position of `offset_ms` into track `index`. Out-of-range indexes
/// count every track before them.
pub fn book_position(durations_ms: &[u64], index: usize, offset_ms: u64) -> u64 {
    durations_ms.iter().take(index).sum::<u64>() + offset_ms
}

/// The (track index, offset) for a book-wide position. Positions past the end
/// map to the end of the last track. Returns `None` for a book without tracks.
pub fn track_position(durations_ms: &[u64], position_ms: u64) -> Option<(usize, u64)> {
    let last = durations_ms.len().checked_sub(1)?;
    let mut start = 0u64;
    for (i, &d) in durations_ms.iter().enumerate() {
        if position_ms < start + d || i == last {
            return Some((i, (position_ms - start).min(d)));
        }
        start += d;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_inside_tracks() {
        let d = [1000, 2000, 3000];
        assert_eq!(track_position(&d, 0), Some((0, 0)));
        assert_eq!(track_position(&d, 999), Some((0, 999)));
        assert_eq!(track_position(&d, 1500), Some((1, 500)));
        assert_eq!(book_position(&d, 1, 500), 1500);
        assert_eq!(book_position(&d, 2, 2999), 5999);
    }

    #[test]
    fn track_boundaries_belong_to_the_next_track() {
        let d = [1000, 2000, 3000];
        assert_eq!(track_position(&d, 1000), Some((1, 0)));
        assert_eq!(track_position(&d, 3000), Some((2, 0)));
        assert_eq!(book_position(&d, 1, 0), 1000);
    }

    #[test]
    fn past_the_end_and_empty() {
        let d = [1000, 2000];
        assert_eq!(track_position(&d, 3000), Some((1, 2000)));
        assert_eq!(track_position(&d, 99_999), Some((1, 2000)));
        assert_eq!(track_position(&[], 0), None);
        assert_eq!(book_position(&d, 9, 0), 3000);
    }

    #[test]
    fn zero_length_tracks_are_skipped() {
        let d = [1000, 0, 2000];
        assert_eq!(track_position(&d, 1000), Some((2, 0)));
    }
}
