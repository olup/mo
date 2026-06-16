extern "C" {
    fn answer_from_c() -> Int32
}

pub fn answer() -> Int {
    return answer_from_c()
}
