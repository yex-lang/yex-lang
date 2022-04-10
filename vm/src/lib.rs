#![deny(missing_docs)]
#![allow(unused_unsafe)]
#![deny(clippy::all)]
#![allow(clippy::unit_arg, clippy::option_map_unit_fn)]
//! Virtual Machine implementation for the yex programming language
mod env;
mod error;
#[doc(hidden)]
pub mod gc;
mod literal;
mod opcode;
mod prelude;
mod stack;

use gc::GcRef;
use literal::{
    fun::{FnArgs, NativeFn},
    instance::Instance,
    tuple::Tuple,
    yextype::instantiate,
    TryGet,
};

use crate::error::InterpretResult;

pub use crate::{
    env::EnvTable,
    literal::{
        fun::{Fn, FnKind},
        list::List,
        symbol::Symbol,
        yextype::YexType,
        Value,
    },
    opcode::{OpCode, OpCodeMetadata},
    stack::StackVec,
};

const STACK_SIZE: usize = 512;
const NIL: Value = Value::Nil;

static mut LINE: usize = 1;
static mut COLUMN: usize = 1;

#[macro_export]
#[doc(hidden)]
macro_rules! raise {
    ($err: ident) => {{
        Err($crate::raise_err!($err))
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! raise_err {
    ($error: ident) => {
        unsafe {
            let msg = $crate::Symbol::new(stringify!($error));
            $crate::error::InterpretError {
                line: $crate::LINE,
                column: $crate::COLUMN,
                err: msg,
            }
        }
    };
}

type Stack = StackVec<Value, STACK_SIZE>;

/// Bytecode for the virtual machine, contains the instructions to be executed and the constants to
/// be loaded
pub type Bytecode = Vec<OpCodeMetadata>;

type BytecodeRef<'a> = &'a Bytecode;
use std::{mem::swap, ops};
/// Implements the Yex virtual machine, which runs the [`crate::OpCode`] instructions in a stack
/// model
pub struct VirtualMachine {
    stack: Stack,
    locals: [Value; 1024],
    used_locals: usize,
    constants: Vec<Value>,
    globals: EnvTable,
}

impl VirtualMachine {
    /// Reset the instruction pointer and the stack
    pub fn reset(&mut self) {
        self.stack = stackvec![];
    }

    /// sets the constants for execution
    pub fn set_consts(&mut self, constants: Vec<Value>) {
        self.constants = constants;
    }

    /// Pop's the last value on the stack
    pub fn pop_last(&self) -> &Value {
        self.stack.last().unwrap_or(&Value::Nil)
    }

    /// Get the value of a global variable
    pub fn get_global<T: Into<Symbol>>(&self, name: T) -> Option<Value> {
        self.globals.get(&name.into())
    }

    /// Set the value of a global variable
    pub fn set_global<T: Into<Symbol>>(&mut self, name: T, value: Value) {
        self.globals.insert(name.into(), value);
    }

    /// Executes a given set of bytecode instructions
    pub fn run(&mut self, bytecode: BytecodeRef) -> InterpretResult<()> {
        let bytecode = &*bytecode;
        let mut try_stack = vec![];

        let mut ip = 0;
        let mut frame_locals = 0;

        while ip < bytecode.len() {
            let op = unsafe {
                let op = bytecode[ip];
                LINE = op.line;
                COLUMN = op.column;
                op.opcode
            };

            self.debug_stack(&op);

            let res = match op {
                OpCode::Try(offset) => {
                    try_stack.push(offset);
                    Ok(())
                }

                OpCode::EndTry => {
                    try_stack.pop();
                    Ok(())
                }

                OpCode::Jmp(offset) => {
                    ip = offset;
                    continue;
                }

                OpCode::Jmf(offset) => {
                    if !self.pop().to_bool() {
                        ip = offset;
                        continue;
                    }
                    Ok(())
                }

                OpCode::TCall(arity) => {
                    self.valid_tail_call(arity, bytecode)?;
                    ip = 0;
                    continue;
                }

                _ => self.run_op(op, &mut frame_locals),
            };

            if let Err(e) = res {
                if try_stack.is_empty() {
                    return Err(e);
                }

                let try_ip = try_stack.pop().unwrap();
                self.push(e.err.into());
                ip = try_ip;
            }

            ip += 1;
        }

        self.used_locals -= frame_locals;

        Ok(())
    }

    #[inline(always)]
    fn run_op(&mut self, op: OpCode, frame_locals: &mut usize) -> InterpretResult<()> {
        match op {
            OpCode::Nop => (),

            // Stack manipulation
            OpCode::Push(value) => {
                let value = self.constants[value].clone();
                self.push(value);
            }
            OpCode::Pop => {
                self.pop();
            }

            OpCode::Dup => {
                let value = self.pop();
                self.push(value.clone());
                self.push(value);
            }

            OpCode::Rev => {
                let (a, b) = self.pop_two();
                self.push(b);
                self.push(a);
            }

            // function calls
            OpCode::Call(arity) => self.call(arity)?,

            // mathematical operators
            OpCode::Add => self.binop(|a, b| a + b)?,
            OpCode::Sub => self.binop(|a, b| a - b)?,
            OpCode::Mul => self.binop(|a, b| a * b)?,
            OpCode::Div => self.binop(|a, b| a / b)?,
            OpCode::Rem => self.binop(|a, b| a % b)?,

            // bitwise operators
            OpCode::BitAnd => self.binop(|a, b| a & b)?,
            OpCode::BitOr => self.binop(|a, b| a | b)?,
            OpCode::Xor => self.binop(|a, b| a ^ b)?,
            OpCode::Shl => self.binop(|a, b| a << b)?,
            OpCode::Shr => self.binop(|a, b| a >> b)?,

            // comparison operators
            OpCode::Eq => self.binop(|a, b| Ok(a == b))?,
            OpCode::Less => {
                let (a, b) = self.pop_two();
                self.push(a.ord_cmp(&b)?.is_lt().into());
            }
            OpCode::LessEq => {
                let (a, b) = self.pop_two();
                self.push(a.ord_cmp(&b)?.is_le().into());
            }

            // unary operators
            OpCode::Not => {
                let value = self.pop();
                self.push(!value);
            }
            OpCode::Len => {
                let value = self.pop();
                self.push(Value::Num(value.len() as f64));
            }
            OpCode::Neg => {
                let value = self.pop();
                self.try_push(-value)?;
            }

            // locals manipulation
            OpCode::Load(offset) => {
                let value = self.locals[offset + self.used_locals - *frame_locals].clone();
                self.push(value);
            }
            OpCode::Save(offset) => {
                let value = self.pop();

                self.used_locals += 1;
                *frame_locals += 1;
                self.locals[offset + (self.used_locals - *frame_locals)] = value;
            }
            OpCode::Drop(_) => {
                *frame_locals -= 1;
                self.used_locals -= 1;
            }

            // globals manipulation
            OpCode::Loag(name) => {
                let value = match self.get_global(name) {
                    Some(value) => value,
                    None => raise!(NameError)?,
                };
                self.push(value);
            }
            OpCode::Savg(name) => {
                let value = self.pop();
                self.set_global(name, value);
            }

            // list manipulation
            OpCode::Prep => {
                let value = self.pop();
                let list: List = self.pop().get()?;

                self.push(list.prepend(value).into());
            }

            OpCode::New(arity) => {
                let ty: GcRef<YexType> = self.pop().get()?;

                let mut args = vec![];
                for _ in 0..arity {
                    args.push(self.pop());
                }

                instantiate(self, ty, args)?;
            }
            OpCode::Get(field) => {
                let obj: GcRef<Instance> = self.pop().get()?;

                let value = obj
                    .fields
                    .get(&field)
                    .ok_or_else(|| raise_err!(FieldError))?;

                self.push(value);
            }
            OpCode::Type => {
                let value = self.pop();
                self.push(Value::Type(value.type_of()));
            }
            OpCode::Ref(method) => {
                let ty: GcRef<YexType> = self.pop().get()?;

                let method = ty.fields.get(&method).ok_or(raise_err!(FieldError))?;

                self.push(method);
            }

            OpCode::Tup(len) => {
                let mut tup = vec![];
                for _ in 0..len {
                    tup.push(self.pop());
                }
                self.push(tup.into());
            }

            OpCode::TupGet(index) => {
                let tup: Tuple = self.pop().get()?;
                let elem = tup.0.get(index).unwrap(); // this SHOULD be unreachable
                self.push(elem.clone());
            }

            // these opcodes are handled by the run function, since they can manipulate the ip
            OpCode::Try(..)
            | OpCode::EndTry
            | OpCode::Jmp(..)
            | OpCode::Jmf(..)
            | OpCode::TCall(..) => unreachable!(),
        };

        Ok(())
    }

    #[cfg(debug_assertions)]
    /// Debug the values on the stack and in the bytecode
    pub fn debug_stack(&self, instruction: &OpCode) {
        eprintln!("Stack: {:?} ({instruction:?})", self.stack);
    }

    #[cfg(not(debug_assertions))]
    /// Debug the values on the stack and in the bytecode
    pub fn debug_stack(&self, _: &OpCode) {}

    #[inline(always)]
    fn call_args(&mut self, arity: usize, fun: &Fn) -> FnArgs {
        if fun.arity == arity && fun.is_bytecode() && fun.args.is_empty() {
            return stackvec![];
        }

        let mut args = stackvec![];

        let mut i = 1;
        for _ in 0..arity {
            unsafe { args.insert_at(arity - i, self.pop()) };
            i += 1;
        }

        unsafe { args.set_len(arity) };

        for arg in fun.args.iter() {
            args.push(arg.clone());
        }

        args
    }

    pub(crate) fn call(&mut self, arity: usize) -> InterpretResult<()> {
        let fun: GcRef<Fn> = self.pop().get()?;

        let args = self.call_args(arity, &fun);

        if arity > fun.arity {
            raise!(CallError)?;
        }

        if arity < fun.arity {
            self.push(Value::Fn(GcRef::new(fun.apply(args))));
            return Ok(());
        }

        match &*fun.body {
            FnKind::Bytecode(bytecode) => self.call_bytecode(bytecode, args),
            FnKind::Native(ptr) => self.call_native(*ptr, args),
        }
    }

    #[inline(always)]
    fn call_bytecode(&mut self, bytecode: BytecodeRef, args: FnArgs) -> InterpretResult<()> {
        self.used_locals += 1;
        for arg in args {
            self.push(arg);
        }

        self.run(bytecode)?;
        self.used_locals -= 1;
        Ok(())
    }

    #[inline(always)]
    fn call_native(&mut self, fp: NativeFn, args: FnArgs) -> InterpretResult<()> {
        let args = args.reverse().into();
        let result = fp(self, args);
        self.try_push(result)
    }

    #[inline]
    fn valid_tail_call(&mut self, arity: usize, frame: BytecodeRef) -> InterpretResult<()> {
        let fun: GcRef<Fn> = self.pop().get()?;

        match &*fun.body {
            FnKind::Bytecode(_) if fun.arity != arity => {
                raise!(TailCallError)
            }
            FnKind::Bytecode(bytecode) if bytecode != frame => {
                raise!(TailCallError)
            }
            FnKind::Native(_) => {
                raise!(TailCallError)
            }
            FnKind::Bytecode(_) => Ok(()),
        }
    }

    #[track_caller]
    pub(crate) fn push(&mut self, constant: Value) {
        self.stack.push(constant)
    }

    #[track_caller]
    pub(crate) fn pop(&mut self) -> Value {
        self.stack.pop()
    }

    fn binop<T, F>(&mut self, f: F) -> InterpretResult<()>
    where
        T: Into<Value>,
        F: ops::Fn(Value, Value) -> InterpretResult<T>,
    {
        let a = self.pop();
        let b = self.pop();
        Ok(self.push(f(b, a)?.into()))
    }

    fn pop_two(&mut self) -> (Value, Value) {
        let mut ret = (self.pop(), self.pop());
        swap(&mut ret.0, &mut ret.1);
        ret
    }

    fn try_push(&mut self, constant: InterpretResult<Value>) -> InterpretResult<()> {
        Ok(self.push(constant?))
    }
}

impl Default for VirtualMachine {
    fn default() -> Self {
        const STACK: Stack = StackVec::new();

        let prelude = prelude::prelude();
        Self {
            stack: STACK,
            locals: [NIL; 1024],
            used_locals: 0,
            constants: Vec::new(),
            globals: prelude,
        }
    }
}
