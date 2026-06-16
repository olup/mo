import * as core from "core/unsafe"

interface Drop {
    fn drop(&self)
}

struct ManualString: Drop {
    data: String

    fn drop(&self) {
        free_data(self.data)
    }
}

fn make_data() -> String {
    return core.string_concat("drop", " interface")
}

fn free_data(value: &String) {
    core.free(core.string_ptr(value))
}

fn exercise() {
    let value = ManualString { data: make_data() }
}

fn main() -> Int {
    let alloc0 = core.mem_alloc_count()
    let free0 = core.mem_free_count()
    exercise()
    let alloc1 = core.mem_alloc_count()
    let free1 = core.mem_free_count()
    if alloc1 < alloc0 + 1 {
        return 1
    }
    if free1 < free0 + 1 {
        return 2
    }
    return 42
}
