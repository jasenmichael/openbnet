-- Kept to SQL that SQLite and PostgreSQL both accept. Ids are assigned by the server.
CREATE TABLE accounts (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL UNIQUE,
    role TEXT NOT NULL DEFAULT 'player',
    locked BIGINT NOT NULL DEFAULT 0,
    muted BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    last_login_at BIGINT NOT NULL DEFAULT 0,
    last_login_product TEXT NOT NULL DEFAULT '',
    last_login_ip TEXT NOT NULL DEFAULT ''
);

CREATE TABLE credentials (
    account_id BIGINT NOT NULL REFERENCES accounts (id),
    scheme TEXT NOT NULL,
    data TEXT NOT NULL,
    PRIMARY KEY (account_id, scheme)
);

CREATE TABLE profiles (
    account_id BIGINT NOT NULL REFERENCES accounts (id),
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    PRIMARY KEY (account_id, name)
);

CREATE TABLE records (
    account_id BIGINT NOT NULL REFERENCES accounts (id),
    product TEXT NOT NULL,
    ladder BIGINT NOT NULL,
    wins BIGINT NOT NULL DEFAULT 0,
    losses BIGINT NOT NULL DEFAULT 0,
    draws BIGINT NOT NULL DEFAULT 0,
    disconnects BIGINT NOT NULL DEFAULT 0,
    rating BIGINT NOT NULL DEFAULT 0,
    high_rating BIGINT NOT NULL DEFAULT 0,
    last_result TEXT NOT NULL DEFAULT '',
    last_game_at BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, product, ladder)
);

CREATE TABLE games (
    id BIGINT PRIMARY KEY,
    product TEXT NOT NULL,
    name TEXT NOT NULL,
    game_type BIGINT NOT NULL,
    ladder BIGINT NOT NULL,
    started_at BIGINT NOT NULL,
    ended_at BIGINT NOT NULL
);

CREATE TABLE game_players (
    game_id BIGINT NOT NULL REFERENCES games (id),
    account_id BIGINT NOT NULL REFERENCES accounts (id),
    result TEXT NOT NULL,
    PRIMARY KEY (game_id, account_id)
);

CREATE TABLE game_reports (
    game_id BIGINT NOT NULL REFERENCES games (id),
    reporter_id BIGINT NOT NULL REFERENCES accounts (id),
    subject_id BIGINT NOT NULL REFERENCES accounts (id),
    result TEXT NOT NULL,
    PRIMARY KEY (game_id, reporter_id, subject_id)
);
