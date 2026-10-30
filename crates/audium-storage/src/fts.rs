//! Keeps the full-text index (`books_fts`, migration 0002) in step with the
//! books' metadata, and turns user input into an FTS5 query.

use audium_core::BookId;
use rusqlite::Connection;

use crate::Result;

/// Re-indexes one book (or removes it from the index if it no longer exists).
pub fn refresh(conn: &Connection, book: BookId) -> Result<()> {
    conn.execute("DELETE FROM books_fts WHERE rowid = ?1", [book.0])?;
    conn.execute(
        "INSERT INTO books_fts (rowid, title, authors, narrators, series)
         SELECT b.id, b.title,
                (SELECT group_concat(c.name, ' ') FROM book_contributors bc JOIN contributors c ON c.id = bc.contributor_id
                  WHERE bc.book_id = b.id AND bc.role = 'author'),
                (SELECT group_concat(c.name, ' ') FROM book_contributors bc JOIN contributors c ON c.id = bc.contributor_id
                  WHERE bc.book_id = b.id AND bc.role = 'narrator'),
                (SELECT s.name FROM series s WHERE s.id = b.series_id)
         FROM books b WHERE b.id = ?1",
        [book.0],
    )?;
    Ok(())
}

/// Drops index entries of deleted books.
pub fn prune(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM books_fts WHERE rowid NOT IN (SELECT id FROM books)",
        [],
    )?;
    Ok(())
}

/// User text -> FTS5 query: every word must match as a prefix
/// ("har pot" finds "Harry Potter"). Operators and quotes in the input are
/// treated as plain text. Returns `None` when there is nothing to search.
pub fn query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(16)
        .map(|t| format!("\"{t}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_text_becomes_prefix_terms() {
        assert_eq!(query("har pot").as_deref(), Some("\"har\"* \"pot\"*"));
        assert_eq!(
            query("  \"OR\" NEAR(*) -x ").as_deref(),
            Some("\"OR\"* \"NEAR\"* \"x\"*")
        );
        assert_eq!(query("d’été").as_deref(), Some("\"d\"* \"été\"*"));
        assert_eq!(query(" ;:- "), None);
    }
}
