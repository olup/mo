# SQLite native package demo

This demo shows a Mo package that wraps SQLite through `extern "C"` and declares
its static archive in the package `mo.toml`. The app imports only the Mo package;
the final link picks up `vendor/libsqlite3.a` transitively.

Prepare the vendored static archive:

```sh
./demos/sqlite/prepare-sqlite.sh
```

Build and run the app:

```sh
cargo run -- build demos/sqlite/app/main.mo -o /tmp/mo_sqlite_demo
/tmp/mo_sqlite_demo
echo $?
```

The program exits with `42` after opening an in-memory SQLite database, creating
a table, inserting a row, and closing the database.

The prepare script downloads the current SQLite amalgamation advertised on the
official SQLite download page, then compiles only `sqlite3.c` into
`app/packages/sqlite/vendor/libsqlite3.a`.
