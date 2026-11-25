CREATE TABLE friends (
    account_id BIGINT NOT NULL REFERENCES accounts (id),
    friend_id BIGINT NOT NULL REFERENCES accounts (id),
    added_at BIGINT NOT NULL,
    PRIMARY KEY (account_id, friend_id)
);

CREATE TABLE topics (
    channel TEXT PRIMARY KEY,
    topic TEXT NOT NULL
);

CREATE INDEX records_by_rating ON records (product, ladder, rating);
