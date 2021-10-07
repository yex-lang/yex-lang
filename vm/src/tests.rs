#[test]
fn test_add() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Push(Literal::Num(137.0)),
        Instruction::Add,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num(112.0 + 137.0))
}

#[test]
fn test_sub() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Push(Literal::Num(137.0)),
        Instruction::Sub,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num(112.0 - 137.0))
}

#[test]
fn test_mul() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Push(Literal::Num(137.0)),
        Instruction::Mul,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num(112.0 * 137.0))
}

#[test]
fn test_div() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Push(Literal::Num(137.0)),
        Instruction::Div,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num(112.0 / 137.0))
}

#[test]
fn test_string_concat() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Str("Hello, ".to_string())),
        Instruction::Push(Literal::Str("World".to_string())),
        Instruction::Add,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Str("Hello, World".to_string()));
}

#[test]
fn test_neg() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Neg,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num(-112.0))
}

#[test]
fn test_xor() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Push(Literal::Num(317.0)),
        Instruction::Xor,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num((112 ^ 317) as f64))
}

#[test]
fn test_bitand() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Push(Literal::Num(317.0)),
        Instruction::BitAnd,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num((112 & 317) as f64))
}

#[test]
fn test_bitor() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(112.0)),
        Instruction::Push(Literal::Num(317.0)),
        Instruction::BitOr,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num((112 | 317) as f64))
}

#[test]
fn test_shr() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(12.0)),
        Instruction::Push(Literal::Num(17.0)),
        Instruction::Shr,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num((12_i64 >> 17_i64) as f64))
}

#[test]
fn test_shl() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    let test = vm.run(vec![
        Instruction::Push(Literal::Num(12.0)),
        Instruction::Push(Literal::Num(17.0)),
        Instruction::Shl,
        Instruction::Ret,
    ]);

    assert_eq!(test, Literal::Num((12_i64 << 17_i64) as f64))
}

#[test]
#[should_panic]
fn should_panic_string_add_number() {
    use crate::{Instruction, Literal, VirtualMachine};

    let mut vm = VirtualMachine::default();

    vm.run(vec![
        Instruction::Push(Literal::Str("Hello, ".to_string())),
        Instruction::Push(Literal::Num(1.0)),
        Instruction::Add,
        Instruction::Ret,
    ]);
}
