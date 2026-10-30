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

/// A saved position this close to the end of a track resumes at the start
/// of the next one (the track was effectively finished).
pub const TRACK_END_SNAP_MS: u64 = 500;

/// Where to resume a book. `tracks` are the playable tracks as
/// (track id, duration). The saved track is found by id; if it is gone (a
/// missing file), the saved book-wide position is used instead. Returns
/// (track index, offset), or `None` for a book without tracks.
pub fn resume_point<T: PartialEq>(
    tracks: &[(T, u64)],
    saved_track: Option<&T>,
    offset_ms: u64,
    book_ms: u64,
) -> Option<(usize, u64)> {
    let durations: Vec<u64> = tracks.iter().map(|(_, d)| *d).collect();
    let (index, offset) = match saved_track.and_then(|id| tracks.iter().position(|(t, _)| t == id))
    {
        Some(i) => (i, offset_ms),
        None => track_position(&durations, book_ms)?,
    };
    let duration = durations[index];
    if duration > 0 && offset + TRACK_END_SNAP_MS >= duration {
        return Some(if index + 1 < tracks.len() {
            (index + 1, 0)
        } else {
            (index, duration)
        });
    }
    Some((index, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_by_track_id() {
        let t = [(10, 1000), (11, 2000), (12, 3000)];
        assert_eq!(resume_point(&t, Some(&11), 700, 1700), Some((1, 700)));
        assert_eq!(resume_point(&t, Some(&10), 0, 0), Some((0, 0)));
        assert_eq!(resume_point::<i32>(&[], Some(&1), 0, 0), None);
    }

    #[test]
    fn resume_near_track_boundaries() {
        let t = [(10, 1000), (11, 2000), (12, 3000)];
        // Just before the end of a track: start of the next.
        assert_eq!(resume_point(&t, Some(&10), 999, 999), Some((1, 0)));
        assert_eq!(
            resume_point(&t, Some(&10), 500, 500),
            Some((1, 0)),
            "exactly at the snap distance"
        );
        assert_eq!(resume_point(&t, Some(&10), 499, 499), Some((0, 499)));
        // Past the reported duration (durations are estimates).
        assert_eq!(resume_point(&t, Some(&11), 2500, 3500), Some((2, 0)));
        // End of the last track stays at the end.
        assert_eq!(resume_point(&t, Some(&12), 2900, 5900), Some((2, 3000)));
    }

    #[test]
    fn resume_falls_back_to_the_book_position() {
        // Track 11 is missing: only 10 and 12 are playable.
        let t = [(10, 1000), (12, 3000)];
        assert_eq!(resume_point(&t, Some(&11), 700, 1700), Some((1, 700)));
        assert_eq!(resume_point(&t, None, 0, 0), Some((0, 0)));
        // Saved track gone and no durations known (rare: the scanner reads
        // durations): the position cannot be mapped; start of the last track.
        assert_eq!(resume_point(&[(1, 0), (2, 0)], None, 0, 5000), Some((1, 0)));
    }

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
