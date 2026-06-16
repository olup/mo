module examples.methods_interfaces

interface Writer {
    fn write(&mut self, bytes: Slice<Byte>) -> Result<Int, Error>
}

interface Display {
    fn display(&self) -> String
}

interface ReadWriter: Reader + Writer {}

struct File: Writer {
    fd: Int

    fn new(fd: Int) -> File {
        File { fd }
    }

    fn fd(&self) -> Int {
        self.fd
    }
    fn write(&mut self, bytes: Slice<Byte>) -> Result<Int, Error> {
        io.write(self.fd, bytes)
    }
}

fn print(value: Display) {
    io.print(value.display())
}

test "interfaces parse" {
    let file = File.new(1)
    assert(file.fd() == 1)
}
