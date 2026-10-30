-- Audium schema v2: full-text search over books.
-- books_fts.rowid is books.id. Kept up to date by audium-storage (fts.rs)
-- whenever a book's metadata is written.

CREATE VIRTUAL TABLE books_fts USING fts5(
    title,
    authors,
    narrators,
    series,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO books_fts (rowid, title, authors, narrators, series)
SELECT b.id,
       b.title,
       (SELECT group_concat(c.name, ' ') FROM book_contributors bc JOIN contributors c ON c.id = bc.contributor_id
         WHERE bc.book_id = b.id AND bc.role = 'author'),
       (SELECT group_concat(c.name, ' ') FROM book_contributors bc JOIN contributors c ON c.id = bc.contributor_id
         WHERE bc.book_id = b.id AND bc.role = 'narrator'),
       (SELECT s.name FROM series s WHERE s.id = b.series_id)
FROM books b;

CREATE INDEX collection_books_book ON collection_books(book_id);
