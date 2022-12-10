create table source
(
    "id"           uuid not null default gen_random_uuid() primary key,
    "url"          text not null,
    "last_updated" timestamptz
);

create table blacklist
(
    "domain" varchar(255) not null primary key,
    "source" uuid         not null references source (id) on delete cascade
);
