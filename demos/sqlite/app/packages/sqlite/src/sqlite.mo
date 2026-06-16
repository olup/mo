import * as core from "core/unsafe"

pub struct Database {
    handle: Int
}

extern "C" {
    fn sqlite3_open(filename: &Str, out_db: Int) -> Int32
    fn sqlite3_close(db: Int) -> Int32
    fn sqlite3_exec(db: Int, sql: &Str, callback: Int, callback_arg: Int, errmsg: Int) -> Int32
}

pub fn open(path: &Str) -> Database {
    let out_db = core.alloc(8)
    core.store64(out_db, 0, 0)
    let rc = sqlite3_open(path, out_db)
    let handle = core.load64(out_db, 0)
    core.free(out_db)
    if rc == 0 {
        return Database { handle: handle }
    }
    if handle != 0 {
        sqlite3_close(handle)
    }
    return Database { handle: 0 }
}

pub fn open_memory() -> Database {
    return open(":memory:")
}

pub fn is_open(db: &Database) -> Bool {
    return db.handle != 0
}

pub fn exec(db: &Database, sql: &Str) -> Int {
    return sqlite3_exec(db.handle, sql, 0, 0, 0)
}

pub fn close(db: &Database) -> Int {
    if db.handle == 0 {
        return 0
    }
    return sqlite3_close(db.handle)
}

pub fn smoke() -> Int {
    let db = open_memory()
    if !is_open(db) {
        return 1
    }
    let created = exec(db, "create table demo(id integer primary key, name text)")
    let inserted = exec(db, "insert into demo(name) values('mo')")
    let closed = close(db)
    if created == 0 && inserted == 0 && closed == 0 {
        return 42
    }
    return 2
}
