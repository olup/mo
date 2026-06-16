pub fn alloc(size: Int) -> Int {
    return raw_alloc(size)
}

pub fn alloc_string(size: Int) -> String {
    return raw_alloc_string(size)
}

pub fn free(ptr: Int) {
    raw_free(ptr)
}

pub fn alloc_ptr(size: Int) -> *mut Byte {
    return raw_alloc(size)
}

pub fn free_ptr(ptr: *mut Byte) {
    raw_free(ptr)
}

pub fn load8_ptr(ptr: *const Byte, offset: Int) -> Int {
    return raw_load8(ptr, offset)
}

pub fn store8_ptr(ptr: *mut Byte, offset: Int, value: Int) {
    raw_store8(ptr, offset, value)
}

pub fn load8(ptr: Int, offset: Int) -> Int {
    return raw_load8(ptr, offset)
}

pub fn store8(ptr: Int, offset: Int, value: Int) {
    raw_store8(ptr, offset, value)
}

pub fn load64(ptr: Int, offset: Int) -> Int {
    return raw_load64(ptr, offset)
}

pub fn store64(ptr: Int, offset: Int, value: Int) {
    raw_store64(ptr, offset, value)
}

pub fn string_store8(value: &String, offset: Int, byte: Int) {
    raw_string_store8(value, offset, byte)
}

pub fn string_ptr(value: &String) -> Int {
    return raw_string_ptr(value)
}

pub fn string_clone_ptr(value: &Str) -> Int {
    return raw_string_clone_ptr(value)
}

pub fn string_from_ptr(ptr: Int) -> String {
    return raw_string_from_ptr(ptr)
}

pub fn function_ptr(task: fn() -> ()) -> Int {
    return raw_function_ptr(task)
}

pub fn function_from_ptr(ptr: Int) -> fn() -> () {
    return raw_function_from_ptr(ptr)
}

pub fn function_ptr_int(task: fn(Int) -> ()) -> Int {
    return raw_function_ptr(task)
}

pub fn function_from_ptr_int(ptr: Int) -> fn(Int) -> () {
    return raw_function_from_ptr_int(ptr)
}

pub fn function_ptr_handler(task: fn(Int, &Str) -> Int) -> Int {
    return raw_function_ptr(task)
}

pub fn function_from_ptr_handler(ptr: Int) -> fn(Int, &Str) -> Int {
    return raw_function_from_ptr_handler(ptr)
}

pub fn function_ptr_request_handler(task: fn(Int, &http__Request, &Str) -> Int) -> Int {
    return raw_function_ptr(task)
}

pub fn function_from_ptr_request_handler(ptr: Int) -> fn(Int, &http__Request, &Str) -> Int {
    return raw_function_from_ptr_request_handler(ptr)
}

pub fn function_ptr_response_handler(task: fn(Int, &http__Request, &Str) -> http__Response) -> Int {
    return raw_function_ptr(task)
}

pub fn function_from_ptr_response_handler(ptr: Int) -> fn(Int, &http__Request, &Str) -> http__Response {
    return raw_function_from_ptr_response_handler(ptr)
}

pub fn store32le(ptr: Int, offset: Int, value: Int) {
    raw_store8(ptr, offset, value % 256)
    raw_store8(ptr, offset + 1, (value / 256) % 256)
    raw_store8(ptr, offset + 2, (value / 65536) % 256)
    raw_store8(ptr, offset + 3, (value / 16777216) % 256)
}

pub fn set_nonblocking_fd(fd: Int) -> Int {
    return raw_set_nonblocking(fd)
}

pub fn thread_spawn(task: fn() -> ()) -> Int {
    return raw_thread_spawn(task)
}

pub fn thread_join(handle: Int) -> Int {
    return raw_thread_join(handle)
}

pub fn write(fd: Int, text: &Str) -> Int {
    return raw_write(fd, text)
}

pub fn strlen(value: &Str) -> Int {
    return raw_strlen(value)
}

pub fn string_concat(a: &Str, b: &Str) -> String {
    return raw_string_concat(a, b)
}

pub fn int_to_string(value: Int) -> String {
    return raw_int_to_string(value)
}

pub fn float_to_int(value: Float64) -> Int {
    return raw_float_to_int(value)
}

pub fn string_load8(value: &Str, offset: Int) -> Int {
    return raw_load8(value, offset)
}

pub fn mem_alloc_count() -> Int {
    return raw_mem_alloc_count()
}

pub fn mem_free_count() -> Int {
    return raw_mem_free_count()
}

pub fn mem_live_bytes() -> Int {
    return raw_mem_live_bytes()
}

pub fn mem_high_water_bytes() -> Int {
    return raw_mem_high_water_bytes()
}
