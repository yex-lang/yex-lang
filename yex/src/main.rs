use front::compile;

#[cfg(feature = "repl")]
use {front::compile_expr, rustyline::Editor};
use std::{env::args, fs::read_to_string, process::exit};
use vm::{VirtualMachine, Constant, gc::GcRef, Bytecode, OpCode, OpCodeMetadata};

fn byte_code_debug() -> Result<i32, front::ParseError> {
    let mut vm = VirtualMachine::default();

    vm.set_consts(vec![Constant::Num(20.1), Constant::Num(20.0)]);
    let bytecode: Bytecode = vec![
        OpCodeMetadata {
            line: 1 as usize,
            opcode: OpCode::Push(0),
            column: 0 as usize  
        },
        OpCodeMetadata {
            line: 1 as usize,
            opcode: OpCode::Push(1),
            column: 0 as usize  
        },
        OpCodeMetadata {
            line: 1 as usize,
            opcode: OpCode::LessEq,
            column: 0 as usize  
        },
        OpCodeMetadata {
            line: 1 as usize,
            opcode: OpCode::Pop,
            column: 0 as usize  
        }
        ]; 
        vm.run(bytecode).unwrap();
        

    Ok(0)
}

fn eval_file(file: &str) -> Result<i32, front::ParseError> {
    let mut vm = VirtualMachine::default();

    let file = read_to_string(file).unwrap_or_else(|_| {
        eprintln!("File not found");
        exit(1)
    });

    if file.is_empty() {
        return Ok(0);
    }

    let (bytecode, constants) = compile(file)?;
    #[cfg(debug_assertions)]
    {
        println!("bytecode: {:#?}", &bytecode);
        println!("constants: {:#?}", &constants);
    }
    vm.set_consts(constants);
    if let Err(e) = vm.run(bytecode) {
        eprintln!("{}", e)
    }

    Ok(0)
}

#[cfg(feature = "repl")]
fn start(args: Vec<String>) -> i32 {
    let mut vm = VirtualMachine::default();
    let mut repl = Editor::<()>::new();

    if args.len() > 1 {
        if args[1].as_str() == "bytecode" {
            println!("sexo");
            return match byte_code_debug() {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            };
        }else { 
            
            return match eval_file(&args[1]) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            };
        }
    }
    loop {
        let line = match repl.readline("yex> ").map(|it| it.trim().to_string()) {
            Ok(str) => {
                repl.add_history_entry(&str);
                str
            }
            Err(_) => return 0,
        };
        if line.is_empty() {
            continue;
        }

        let (bytecode, constants) = {
            if line.trim().starts_with("let") {
                compile(line)
            } else {
                compile_expr(line)
            }
            .unwrap_or_else(|e| {
                eprintln!("{}", e);
                (vec![], vec![])
            })
        };

        #[cfg(debug_assertions)]
        {
            println!("bytecode: {:#?}", &bytecode);
            println!("constants: {:#?}", &constants);
        }
        vm.set_consts(constants);
        if let Err(e) = vm.run(bytecode) {
            eprintln!("{}", e)
        }

        println!(">> {}", vm.pop_last());
        vm.reset();
    }
}

#[cfg(feature = "repl")]
fn main() {
    let args = args().collect();
    exit(start(args));
}

#[cfg(not(feature = "repl"))]
fn main() {
    let args = args().collect::<Vec<_>>();
    if args[1] == "bytecode" {
        exit(match byte_code_debug() {
            Ok(n) => n,
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        });
    }
    let file = match args.get(1) {
        Some(file) => eval_file(file),
        None => {
            eprintln!("Error: expected file name");
            exit(1);
        }
    };

    match file {
        Ok(n) => exit(n),
        Err(e) => {
            eprintln!("{}", e);
            exit(1)
        }
    }
}
