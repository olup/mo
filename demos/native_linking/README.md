# Native static library demo

This demo shows a Mo package that declares a C static archive in its own
`mo.toml`. The app imports only the Mo package; it does not pass linker flags on
the CLI.

Build the demo archive:

```sh
cd demos/native_linking/app/packages/native_answer
cc -c vendor/answer.c -o vendor/answer.o
ar rcs vendor/libanswer.a vendor/answer.o
```

Then build and run the Mo app:

```sh
cd ../../../../..
cargo run -- build demos/native_linking/app/main.mo -o /tmp/mo_native_linking_demo
/tmp/mo_native_linking_demo
echo $?
```

The program exits with `42`.
