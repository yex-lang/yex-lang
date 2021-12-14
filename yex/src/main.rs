use front::{compile, compile_expr};
use rustyline::Editor;
use std::{env::args, fs::read_to_string, process::exit};
use vm::VirtualMachine;

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
    vm.run(bytecode);

    Ok(0)
}

fn start(args: Vec<String>) -> i32 {
    let mut vm = VirtualMachine::default();
    let mut repl = Editor::<()>::new();

    if args.len() > 1 {
        return match eval_file(&args[1]) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        };
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
        vm.run(bytecode);

        println!(">> {}", vm.pop_last());
        vm.reset();
    }
}

fn main() {
    exit(start(args().collect()))
}
