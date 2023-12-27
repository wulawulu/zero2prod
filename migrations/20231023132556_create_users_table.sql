-- Add migration script here
CREATE TABLE users(
    _user_id uuid PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL
);