#![deny(missing_docs)]
//! Virtual Machine implementation for the yex programming language
mod either;
mod env;
mod error;
#[doc(hidden)]
pub mod gc;
mod list;
mod literal;
mod opcode;
mod prelude;
mod stack;
mod table;

#[cfg(test)]
mod tests;

use std::{cmp::Ordering, mem};

use env::EnvTable;
use gc::GcRef;

use crate::{
    env::Env,
    error::InterpretResult,
    literal::{nil, FunArgs, FunBody},
};

pub use crate::{
    either::Either,
    list::List,
    literal::{symbol::Symbol, Constant, Fun},
    opcode::{OpCode, OpCodeMetadata},
    stack::StackVec,
    table::Table,
};

const STACK_SIZE: usize = 512;
const RECURSION_LIMIT: usize = 768;

static mut LINE: usize = 1;
static mut COLUMN: usize = 1;

#[macro_export]
#[doc(hidden)]
macro_rules! panic {
    ($($tt:tt)+) => {
        unsafe {
            let msg = format!($($tt)+);
            Err($crate::error::InterpretError { line: $crate::LINE, column: $crate::COLUMN, err: msg })
        }
    }
}

struct CallFrame {
    ip: *const OpCodeMetadata,
    len: usize,
    index: usize,
}

impl CallFrame {
    pub fn new(bytecode: BytecodeRef) -> Self {
        Self {
            ip: bytecode.as_ptr(),
            len: bytecode.len(),
            index: 0,
        }
    }

    pub fn bytecode(&self) -> BytecodeRef {
        unsafe { std::slice::from_raw_parts(self.ip, self.len) }
    }

    pub fn jump(&mut self, count: usize) {
        self.ip = unsafe { self.ip.offset((count as isize) - (self.index as isize)) };
        self.index = count;
    }

    pub fn offset(&self) -> usize {
        self.index
    }

    pub fn add(&mut self, count: usize) {
        self.index += count;
        unsafe { self.ip = self.ip.add(count) }
    }

    pub fn advance(&mut self) -> OpCodeMetadata {
        let op = unsafe { *self.ip };
        self.add(1);
        op
    }
}

type CallStack = StackVec<CallFrame, RECURSION_LIMIT>;
type Stack = StackVec<Constant, STACK_SIZE>;

/// Bytecode for the virtual machine, contains the instructions to be executed and the constants to
/// be loaded
pub type Bytecode = Vec<OpCodeMetadata>;
type BytecodeRef<'a> = &'a [OpCodeMetadata];
use dlopen::raw::Library;
use std::collections::HashMap;
/// Implements the Yex virtual machine, which runs the [`crate::OpCode`] instructions in a stack
/// model
pub struct VirtualMachine {
    constants: Vec<Constant>,
    call_stack: CallStack,
    dlopen_libs: HashMap<String, GcRef<Library>>,
    stack: Stack,
    variables: Env,
    globals: EnvTable,
}

impl VirtualMachine {
    /// Reset the instruction pointer and the stack
    pub fn reset(&mut self) {
        self.call_stack = StackVec::new();
        self.stack = StackVec::new();
    }

    /// sets the constants for execution
    pub fn set_consts(&mut self, constants: Vec<Constant>) {
        self.constants = constants.into_iter().collect();
    }

    /// Pop's the last value on the stack
    pub fn pop_last(&self) -> &Constant {
        self.stack.last().unwrap_or(&Constant::Nil)
    }

    /// Executes a given set of bytecode instructions
    pub fn run(&mut self, bytecode: BytecodeRef) -> InterpretResult<Constant> {
        self.call_stack.push(CallFrame::new(bytecode));

        while self.call_frame().offset() < bytecode.len() {
            self.run_instruction()?;
        }

        self.call_stack.pop();

        Ok(Constant::Nil)
    }

    fn run_instruction(&mut self) -> InterpretResult<()> {
        macro_rules! binop {
            ($op:tt) => {{
                let right = self.pop();
                let left = self.pop();

                self.push((left $op right)?)
            }}
        }

        macro_rules! unaop {
            ($op:tt) => {{
                let right = self.pop();

                self.push(($op right)?)
            }};
        }

        self.debug_stack();

        let inst = self.call_frame().advance();

        unsafe {
            LINE = inst.line;
            COLUMN = inst.column;
        }

        use OpCode::*;
        match inst.opcode {
            Halt => std::process::exit(0),
            Push(n) => {
                if self.constants.len() <= n {
                    panic!("err: can't find consts. Are you in repl?")?;
                }

                let val = self.constants[n].clone();
                self.push(val)
            }
            Pop => {
                self.pop();
            }

            Save(val) => {
                let value = self.pop();
                self.variables.insert(val, value);
            }

            Savg(val) => {
                let value = self.pop();
                self.globals.insert(val, value)
            }

            Load(val) => {
                let val = match self.variables.get(&val) {
                    Some(v) => v,
                    None => match self.globals.get(&val) {
                        Some(v) => v,
                        None => return panic!("unknown variable {}", val),
                    },
                };

                self.push(val);
            }

            Drop(val) => {
                self.variables.remove(&val);
            }

            Drpg(val) => self.globals.remove(&val),

            Jmf(offset) => {
                if Into::<bool>::into(!self.pop()) {
                    self.call_frame().jump(offset);
                    return Ok(());
                }
            }
            Jmp(offset) => {
                self.call_frame().jump(offset);
                return Ok(());
            }

            Nsc => self.variables.nsc(),

            Esc => self.variables.esc(),

            Call(carity) => self.call(carity)?,
            TCall(carity) => self.tcall(carity)?,

            Prep => {
                let val = self.pop();

                match self.pop() {
                    Constant::List(xs) => self.push(Constant::List(GcRef::new(xs.prepend(val)))),
                    other => return panic!("Expected a list, found a `{}`", other),
                };
            }

            Insert(key) => {
                let value = self.pop();

                match self.pop() {
                    Constant::Table(ts) => {
                        self.push(Constant::Table(GcRef::new(ts.insert(key, value))))
                    }
                    other => return panic!("Expected a table, found a `{}`", other),
                };
            }

            Index => self.index()?,

            Rev => {
                let a = self.pop();
                let b = self.pop();
                self.push(a);
                self.push(b);
            }

            Add => binop!(+),
            Sub => binop!(-),
            Mul => binop!(*),
            Div => binop!(/),
            Xor => binop!(^),
            Shl => binop!(>>),
            Shr => binop!(<<),
            BitAnd => binop!(&),
            BitOr => binop!(|),

            Eq => {
                let right = self.pop();
                let left = self.pop();
                self.push(Constant::Bool(left == right))
            }
            Greater => {
                let right = self.pop();
                let left = self.pop();

                self.push(Constant::Bool(left.ord_cmp(&right)?.is_gt()))
            }
            GreaterEq => {
                let right = self.pop();
                let left = self.pop();

                self.push(Constant::Bool(left.ord_cmp(&right)?.is_ge()))
            }

            Less => {
                let right = self.pop();
                let left = self.pop();

                self.push(Constant::Bool(left.ord_cmp(&right)?.is_lt()))
            }
            LessEq => {
                let right = self.pop();
                let left = self.pop();

                self.push(Constant::Bool(left.ord_cmp(&right)?.is_le()))
            }

            Neg => unaop!(-),
            Len => {
                let len = self.pop().len();
                self.push(Constant::Num(len as f64))
            }
            Not => {
                let right = self.pop();
                self.push(!right)
            }
        }

        Ok(())
    }

    fn call_helper(&mut self, carity: usize) -> InterpretResult<(FunBody, usize, FunArgs)> {
        let mut fargs = StackVec::new();
        let fun = self.pop();

        while fargs.len() < carity {
            fargs.push(self.pop())
        }

        let (farity, body) = match fun {
            Constant::Fun(f) => {
                for elem in f.args.iter() {
                    fargs.push(elem.clone())
                }
                (f.arity, f.body.clone())
            }
            other => return panic!("Can't call {}", other),
        };

        Ok((body, farity, fargs))
    }

    fn call(&mut self, carity: usize) -> InterpretResult<()> {
        let (body, farity, fargs) = self.call_helper(carity)?;
        match carity.cmp(&farity) {
            Ordering::Greater => {
                return panic!(
                    "function expected {} arguments, but received {}",
                    farity, carity
                )
            }
            Ordering::Less => self.push(Constant::Fun(GcRef::new(literal::Fun {
                arity: farity - carity,
                body,
                args: fargs,
            }))),
            Ordering::Equal => {
                let curr_env = mem::replace(&mut self.variables, Env::new());
                match body.get() {
                    Either::Left(bytecode) => {
                        fargs.into_iter().for_each(|it| self.push(it));
                        self.run(bytecode)?;
                    }
                    Either::Right(fp) => {
                        let arr = fargs.into_iter().rev().collect();
                        let ret = fp(self, arr);
                        self.push(ret)
                    }
                }
                self.variables = curr_env;
            }
        }
        Ok(())
    }

    fn tcall(&mut self, carity: usize) -> InterpretResult<()> {
        let (body, farity, fargs) = self.call_helper(carity)?;
        match carity.cmp(&farity) {
            Ordering::Greater => panic!(
                "function expected {} arguments, but received {}",
                farity, carity
            )?,

            Ordering::Less => panic!("Can't use partial application in a tail call")?,
            Ordering::Equal => {
                fargs.into_iter().for_each(|it| self.push(it));

                match body.get() {
                    Either::Left(bytecode) if bytecode == self.bytecode() => {
                        self.call_frame().jump(0);
                    }
                    _ => panic!("Can't use tail calls with different functions")?,
                }
            }
        }
        Ok(())
    }

    fn index(&mut self) -> InterpretResult<()> {
        match self.pop() {
            Constant::Num(n) if n.fract() == 0.0 && n >= 0.0 => match &self.pop() {
                Constant::List(xs) => self.push(xs.index(n as usize)),
                other => panic!("Expected a list to index, found a `{}`", other)?,
            },

            Constant::Sym(key) => match &self.pop() {
                Constant::Table(ts) => self.push(ts.get().get(&key).unwrap_or_else(nil)),
                other => panic!("Expected a table to index, found a `{}`", other)?,
            },

            other => return panic!("Expected a integer to use as index, found a `{}`", other),
        };
        Ok(())
    }

    #[cfg(debug_assertions)]
    /// Debug the values on the stack and in the bytecode
    pub fn debug_stack(&self) {
        eprintln!(
            "stack: {:#?}\n",
            self.stack.iter().rev().collect::<Vec<&Constant>>(),
        );
    }

    #[cfg(not(debug_assertions))]
    /// Debug the values on the stack and in the bytecode
    pub fn debug_stack(&self) {}

    #[track_caller]
    fn push(&mut self, constant: Constant) {
        self.stack.push(constant)
    }

    fn bytecode(&mut self) -> BytecodeRef {
        self.call_stack.last().unwrap().bytecode()
    }

    fn call_frame(&mut self) -> &mut CallFrame {
        self.call_stack.last_mut().unwrap()
    }

    #[track_caller]
    fn pop(&mut self) -> Constant {
        self.stack.pop()
    }
}

impl Default for VirtualMachine {
    fn default() -> Self {
        let prelude = prelude::prelude();

        Self {
            constants: vec![],
            call_stack: StackVec::new(),
            stack: StackVec::new(),
            globals: prelude,
            dlopen_libs: HashMap::new(),
            variables: Env::new(),
        }
    }
}
