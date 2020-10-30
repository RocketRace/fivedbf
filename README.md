# fivedbf

Implementation of a [5D Brainfuck With Multiverse Time Travel](https://esolangs.org/wiki/5D_Brainfuck_With_Multiverse_Time_Travel) interpreter.

# Usage

Interpret a .5dbfwmvtt file by passing the file path to the executable.

```bash
fivedbf path_to_file.5dbfwmvtt
```

# Building

Requires rustc 1.47 or greater (for const generics in array types). 
To update rustc, run `rustup update stable`.
 
To build, run:

```bash
cargo build --release
```
 
# Configuration

Specify the features for cargo (`--features "some_features"`) to alter 
the default behavior of the executable. Valid features are:

* `"debug"` : enables debug logging
* `"more_cells"` or `"even_more_cells"` : increases cell count to 250000 and 2000000, respectively
* `"16_bit"` or `"32_bit"` : changes cell size to the specified width
* `"no_overflow"` : disables cell wrapping on `+` and `-`
* `"pointer_wrapping"` : enables pointer wrapping on `<` and `>`
* `"eof_0"` or `"eof_unchanged"` : changes `,` to return 0 or to not change the cell value on EOF, respectively
 
To compile with, e.g. the `"debug"` & `"eof_unchanged"` features, run:

```bash
cargo build --release --flags "debug eof_unchanged"
```

# License

This project is licensed under the MIT license.