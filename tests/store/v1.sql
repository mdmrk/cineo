-- cineo-store schema version 1, as written by M4.
CREATE TABLE addons (
    position INTEGER PRIMARY KEY,
    transport_url TEXT NOT NULL UNIQUE
) STRICT;
CREATE TABLE library_items (
    id TEXT PRIMARY KEY NOT NULL,
    content_type TEXT NOT NULL,
    name TEXT NOT NULL,
    poster TEXT,
    video_id TEXT NOT NULL,
    time_offset_ms INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    updated_ms INTEGER NOT NULL
) STRICT;
INSERT INTO addons VALUES (1, 'https://second.example/manifest.json');
INSERT INTO addons VALUES (0, 'https://first.example/path/manifest.json?token=x');
INSERT INTO library_items VALUES
    ('tt0000001', 'movie', 'First Example Film', 'https://img.example/1.jpg',
     'tt0000001', 600000, 6000000, 1000),
    ('tt0000002', 'series', 'Example Series', 'javascript:alert(1)',
     'tt0000002:1:2', 0, 0, 2000),
    ('tt0000003', 'movie', 'Negative Time', NULL, 'tt0000003', -5, 100, 3000),
    ('tt0000004', '', 'Empty Type', NULL, 'tt0000004', 5, 100, 4000);
PRAGMA user_version = 1;
