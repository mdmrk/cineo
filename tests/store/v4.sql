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
, stream TEXT, favorite INTEGER NOT NULL DEFAULT 0) STRICT;
CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
) STRICT;
INSERT INTO addons VALUES (1, 'https://second.example/manifest.json');
INSERT INTO addons VALUES (0, 'https://first.example/path/manifest.json?token=x');
INSERT INTO library_items VALUES
    ('tt0000001', 'movie', 'First Example Film', 'https://img.example/1.jpg',
     'tt0000001', 600000, 6000000, 1000,
     '{"addon":"https://first.example/path/manifest.json?token=x","title":"First Example","logo":"https://img.example/logo.png","background":"javascript:alert(1)","stream":{"url":"https://media.example/v/1.mp4","name":"1080p"}}',
     3),
    ('tt0000002', 'series', 'Example Series', NULL,
     'tt0000002:1:2', 5000, 7000, 2000,
     '{"addon":"https://second.example/manifest.json","title":"Second Example","stream":{"infoHash":"0123456789abcdef0123456789abcdef01234567","fileIdx":2,"sources":["tracker:udp://tracker.example:1337"]}}',
     0),
    ('tt0000003', 'movie', 'Unreadable Stream', NULL,
     'tt0000003', 5, 100, 3000, '{"addon":"x"}', -1),
    ('tt0000004', '', 'Empty Type', NULL, 'tt0000004', 5, 100, 4000, NULL, 0);
INSERT INTO settings VALUES
    ('p2p_enabled', 'false'),
    ('p2p_acknowledged', 'true'),
    ('subtitle_language', 'spa'),
    ('seek_step', '30'),
    ('volume', '40'),
    ('subtitle_bold', 'maybe'),
    ('retired_key', 'x');
PRAGMA user_version = 4;
