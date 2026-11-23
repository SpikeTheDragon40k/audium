-- Audium schema v3: remember the state of a folder's sidecar files
-- (metadata.json, desc.txt, reader.txt, cover.*, folder.*) when its book was
-- last read, so a rescan notices when only a sidecar changed. NULL means
-- unknown: the folder is read once more on the next rescan.

ALTER TABLE books ADD COLUMN sidecar_stamp TEXT;
