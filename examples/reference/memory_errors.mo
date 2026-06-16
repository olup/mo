module examples.memory_errors

struct User {
    name: String
}

struct File {
    fd: Int
}

fn open_file(path: String) -> File {
    File { fd: 0 }
}

fn read_file_to_string(file: File) -> String {
    "contents"
}

fn string_from_raw(ptr: *mut Byte, len: Int) -> String {
    ""
}

fn borrow_name(user: &User) -> &String {
    &user.name
}

fn rename(user: &mut User, name: String) {
    user.name = name
}

fn consume(user: User) -> String {
    user.name
}

fn load(path: String) -> Result<String, IOError> {
    let file = open_file(path)
    read_file_to_string(file)
}

unsafe fn from_raw(ptr: *mut Byte, len: Int) -> String {
    string_from_raw(ptr, len)
}

extern "C" {
    fn puts(s: *const Byte) -> Int32
}

test "ownership examples parse" {
    let mut user = User { name: "Ada" }
    rename(mut user, "Grace")
    assert(borrow_name(user).len() > 0)
}
