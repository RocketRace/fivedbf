//! Implementation of a 5D Brainfuck With Multiverse Time Travel interpreter.
//! 
//! Implementation is naive and unoptimized, but behaves according to the spec.
//! 
//! # Usage
//! Interpret a .5dbfwmvtt file by passing the file path to the executable.
//! ```bash
//! fivedbf path_to_file.5dbfwmvtt
//! ```
//! 
//! # Building
//! Requires rustc 1.47 or greater (for const generics in array types). 
//! To update rustc, run `rustup update stable`.
//! 
//! To build, run:
//! ```bash
//! cargo build --release
//! ```
//! 
//! # Configuration
//! Specify the features for cargo (`--features "some_features"`) to alter 
//! the default behavior of the executable. Valid features are:
//!
//! * "debug" : enable debug logging
//! * "more_cells" or "even_more_cells" : increase cell count to 250000 and 2000000, respectively
//! * "16_bit" or "32_bit" : changed cell size to the specified width
//! * "no_overflow" : disable cell wrapping on `+` and `-`
//! * "pointer_wrapping" : enable pointer wrapping on `<` and `>`
//! * "eof_0" or "eof_unchanged" : change EOF to return 0, or to not change the cell value, respectively
//! 
//! To compile with, e.g. the "debug" & "eof_unchanged" features, run:
//! ```bash
//! cargo build --release --flags "debug eof_unchanged"
//! ```
use std::{env, fs::read, io::{stdin, stdout, Read, Write}, process::exit};
// All sorts of configuration, feel free to ignore
#[cfg(not(any(feature = "more_cells", feature = "even_more_cells")))]
const CELL_COUNT: usize = 30_000;
#[cfg(all(feature = "more_cells", not(feature = "even_more_cells")))]
const CELL_COUNT: usize = 250_000;
#[cfg(feature = "even_more_cells")]
const CELL_COUNT: usize = 2_000_000;
#[cfg(not(any(feature = "16_bit", feature = "32_bit")))]
type CellSize = u8;
#[cfg(all(feature = "16_bit", not(feature = "32_bit")))]
type CellSize = u16;
#[cfg(feature = "32_bit")]
type CellSize = u32;
/// Not the most useful for debugging but it'll work
fn _debug(timelines: &[Timeline], step: usize) {
    eprintln!("=== Step {} ===", step);
    for (i, t) in timelines.iter().enumerate() {
        eprintln!("--- Timeline {} ---", i);
        eprintln!("Alive: {}", t.alive);
        eprintln!("Program counter: {}", t.pc);
        eprintln!("Pointers: {:?}", t.ptrs);
        eprintln!("History: {:?}", t.ops);
        eprintln!("Tape: {:?} ...", &t.tape[..100]);
    }
}
/// AST consists of a vector of these tokens
#[derive(Debug)]
enum Token {
    // Standard BF instructions
    Inc, Dec, Right, Left, Read, Write, JumpZero(usize), JumpNonzero(usize), 
    // 5DBF instructions
    Back, Up, Down, Await, Spawn(usize), Kill
}
/// Parses a 5DBF program from source bytes
fn parse(bytes: &[u8]) -> Vec<Token> {
    let mut program = vec![];
    let mut loop_stack = vec![];
    let mut paren_stack = vec![];
    let mut pc = 0usize;
    // i is only kept for error reporting
    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'+' => {program.push(Token::Inc); pc += 1},
            b'-' => {program.push(Token::Dec); pc += 1},
            b'>' => {program.push(Token::Right); pc += 1},
            b'<' => {program.push(Token::Left); pc += 1},
            b',' => {program.push(Token::Read); pc += 1},
            b'.' => {program.push(Token::Write); pc += 1},
            b'[' => {
                loop_stack.push((pc, i));
                program.push(Token::JumpZero(0));
                pc += 1;
            },
            b']' => {
                let (old, _) = match loop_stack.pop() {
                    Some(n) => n,
                    None => panic!(format!("Unmatched `]` at position {}", i)),
                };
                program[old] = Token::JumpZero(pc);
                program.push(Token::JumpNonzero(old));
                pc += 1;
            },
            b'~' => {program.push(Token::Back); pc += 1},
            b'^' => {program.push(Token::Up); pc += 1},
            b'v' => {program.push(Token::Down); pc += 1},
            b'@' => {program.push(Token::Await); pc += 1},
            b'(' => {
                paren_stack.push((pc, i));
                program.push(Token::Spawn(0));
                pc += 1;
            },
            b')' => {
                let (old, _) = match paren_stack.pop() {
                    Some(n) => n,
                    None => panic!(format!("Unmatched `)` at position {}", i)),
                };
                program[old] = Token::Spawn(pc);
                program.push(Token::Kill);
                pc += 1;
            },
            _ => ()
        }
    }
    // pretty rudimentary error handling, but it works
    if loop_stack.len() != 0 {
        panic!(format!("Unmatched `[` at position {}", loop_stack[0].1));
    }
    if paren_stack.len() != 0 {
        panic!(format!("Unmatched `(` at position {}", paren_stack[0].1));
    }
    program
}
#[derive(Debug)]
struct Timeline {
    tape: [CellSize; CELL_COUNT],
    pc: usize,
    ptrs: Vec<usize>,
    ops: Vec<Vec<(usize, CellSize)>>,
    alive: bool,
}
impl Timeline {
    /// Create a copy of this timeline
    fn duplicate(&self, pc: usize) -> Self {
        Timeline { 
            tape: self.tape, 
            pc,
            ptrs: self.ptrs.clone(), 
            ops: vec![],
            alive: true,
        }
    }
    /// Push a minimal snapshot of the tape onto the history, for reversibility
    fn snapshot(&mut self) {
        self.ops.push(
            self.ptrs.iter().map(
                |&ptr| (ptr, self.tape[ptr])
            ).collect()
        );
    }
}
/// Bulk of interpreter
fn run(program: &[Token]) -> ! {
    let mut timelines = vec![Timeline {
        tape: [0; CELL_COUNT],
        pc: 0,
        ptrs: vec![0],
        ops: vec![],
        alive: true,
    }];
    let mut _step = 0usize;
    loop {
        #[cfg(feature = "debug")] _debug(&timelines, _step);
        _step += 1;
        let mut to_spawn = vec![];
        let mut kill = false;
        // Array access is used instead of iter_mut().enumerate() because
        // the ^v instructions mutate adjacent timelines
        let count = timelines.len();
        if count == 0 {
            panic!("how");
        }
        for i in 0..count {
            // split_at_mut is necessary to guarantee to the borrow checker that
            // while `timelines` is mutated multiple times, each mutation is to a different element
            let (head, mid) = timelines.split_at_mut(i);
            let (t, tail) = mid.split_first_mut().unwrap();
            // dbg!(i, &t.ptrs);
            // run off the program
            if t.pc > program.len() - 1 {
                if i == 0 { exit(0); }
                else { kill = true; t.alive = false; }
            }
            else {
                match program[t.pc] {
                    Token::Inc => {
                        t.snapshot();
                        for &ptr in &t.ptrs { 
                            #[cfg(not(feature = "no_overflow"))] { t.tape[ptr] += 1; }
                            #[cfg(feature = "no_overflow")] { t.tape[ptr] = t.tape[ptr].saturating_add(1); }
                        }
                    }
                    Token::Dec => {
                        t.snapshot();
                        for &ptr in &t.ptrs { 
                            #[cfg(not(feature = "no_overflow"))] { t.tape[ptr] -= 1; }
                            #[cfg(feature = "no_overflow")] { t.tape[ptr] = t.tape[ptr].saturating_sub(1); }
                        }
                    }
                    Token::Right => {
                        for ptr in t.ptrs.iter_mut() { 
                            if *ptr == CELL_COUNT - 1 { 
                                #[cfg(not(feature = "pointer_wrapping"))] { panic!("Pointer out of bounds"); }
                                #[cfg(feature = "pointer_wrapping")] { *ptr = 0; }
                            } else { *ptr += 1; } 
                        }
                    }
                    Token::Left => {
                        for ptr in t.ptrs.iter_mut() { 
                            if *ptr == 0 { 
                                #[cfg(not(feature = "pointer_wrapping"))] { panic!("Pointer out of bounds"); }
                                #[cfg(feature = "pointer_wrapping")] { *ptr = CELL_COUNT - 1; }
                            } else { *ptr -= 1; } 
                        }
                    }
                    Token::Read => {
                        t.snapshot();
                        let mut handle = stdin();
                        for &ptr in &t.ptrs {
                            // this is not good, but the alternative
                            // is to rely on "unspecified" EOF behavior
                            // with buffered reads
                            let mut buffer = [0; 1];
                            match handle.read(&mut buffer) {
                                Ok(n) => if n == 0 { 
                                    #[cfg(not(any(feature = "eof_0", feature = "eof_unchanged")))] { t.tape[ptr] = CellSize::MAX; }
                                    #[cfg(feature = "eof_0")] { tape[ptr] = 0; }
                                    #[cfg(all(feature = "eof_unchanged", not(feature = "eof_0")))] {}
                                } 
                                else { 
                                    t.tape[ptr] = buffer[0] as CellSize 
                                },
                                Err(_) => panic!("Failed to read from stdin")
                            }
                        }
                    }
                    Token::Write => {
                        let mut handle = stdout();
                        let mut buffer = Vec::with_capacity(1);
                        for &ptr in &t.ptrs { 
                            buffer.push(t.tape[ptr] as u8);
                        }
                        match handle.write_all(&mut buffer) {
                            Ok(_) => (),
                            Err(_) => panic!("Failed to write to stdout"),
                        }
                        // if flush fails and write doesn't, that's your problem and not mine
                        handle.flush().unwrap();
                    }
                    Token::JumpZero(n) => {
                        if t.ptrs.iter().all(|&ptr| t.tape[ptr] == 0) {
                            t.pc = n;
                        }
                    }
                    Token::JumpNonzero(n) => {
                        if t.ptrs.iter().any(|&ptr| t.tape[ptr] != 0) {
                            t.pc = n;
                        }
                    }
                    Token::Back => {
                        let op = match t.ops.pop() {
                            Some(o) => o,
                            None => panic!("Attempted `~` with no history to unwind"),
                        };
                        for (ptr, value) in op {
                            t.tape[ptr] = value;
                        }
                    }
                    Token::Up => {
                        if i == 0 { t.ptrs.clear(); }
                        else {
                            // unwrap valid since i > 0
                            let upper = head.last_mut().unwrap();
                            upper.ptrs.extend(t.ptrs.drain(..));
                        }
                    }
                    Token::Down => {
                        if i == count - 1 { t.ptrs.clear(); }
                        else {
                            // unwrap valid for similar reasons
                            let lower = tail.first_mut().unwrap();
                            lower.ptrs.extend(t.ptrs.drain(..));
                        }
                    }
                    Token::Await => {
                        if i != count - 1 {
                            // unwrap valid for similar reasons
                            let lower = tail.first_mut().unwrap();
                            if lower.ptrs.len() != 0 {
                                t.pc -= 1;
                            }
                        }
                    }
                    Token::Spawn(n) => {
                        to_spawn.push((i, t.pc + 1));
                        t.pc = n;
                    }
                    Token::Kill => {
                        kill = true;
                        t.alive = false;
                    }
                }
                t.pc += 1;
            }
        }
        // Spawn new timelines in appropriate positions
        if to_spawn.len() != 0 {
            for &(i, pc) in to_spawn.iter().rev() {
                timelines.insert(i + 1, timelines[i].duplicate(pc));
            }
        }
        // Any timelines were killed during execution
        if kill {
            let to_kill: Vec<usize> = timelines.iter()
                .enumerate()
                .filter_map(|(i, t)| if t.alive { None } else { Some(i)})
                .rev()
                .collect();
            for i in to_kill {
                timelines.remove(i);
            }
        }
    }
}
fn main() {
    let fp = match env::args().skip(1).next() {
        Some(s) => s,
        None => panic!("File path not supplied"),
    };
    let bytes = match read(fp) {
        Ok(b) => b,
        Err(_) => panic!("File not found!"),
    };
    let program = parse(&bytes);
    #[cfg(feature = "debug")] dbg!(&program);
    run(&program);
}
