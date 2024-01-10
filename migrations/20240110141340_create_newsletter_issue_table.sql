-- Add migration script here
CREATE TABLE newsletter_issues
(
    newsletter_issue_id uuid NOT NULL,
    title               TEXT NOT NULL,
    text_context        TEXT not null,
    html_context        TEXT not null,
    published_at        TEXT NOT NULL,
    PRIMARY KEY (newsletter_issue_id)
);