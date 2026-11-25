CREATE TABLE ip_bans (
    network TEXT PRIMARY KEY,
    reason TEXT NOT NULL,
    banned_by TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    -- Unix time; 0 means for good.
    until_at BIGINT NOT NULL
);
