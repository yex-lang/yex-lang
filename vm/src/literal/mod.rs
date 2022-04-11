use std::{
    cmp::Ordering,
    mem,
    ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub},
};

pub mod file;
pub mod fun;
pub mod instance;
pub mod list;
pub mod str;
pub mod symbol;
pub mod table;
pub mod tuple;
pub mod yextype;

use crate::{error::InterpretResult, gc::GcRef, raise};

use fun::Fn;
use instance::Instance;
use list::List;
use symbol::Symbol;
use yextype::YexType;

use self::{table::Table, tuple::Tuple, symbol::YexSymbol};

pub fn nil() -> Value {
    Value::Nil
}

impl From<Vec<Value>> for Value {
    fn from(vec: Vec<Value>) -> Self {
        Value::Tuple(Tuple::from(vec))
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<f64> for Value {
    fn from(i: f64) -> Self {
        Value::Num(i)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(GcRef::new(s))
    }
}

impl From<Symbol> for Value {
    fn from(s: Symbol) -> Self {
        Value::Sym(s.into())
    }
}

impl From<Instance> for Value {
    fn from(i: Instance) -> Self {
        Value::Instance(GcRef::new(i))
    }
}

impl From<YexType> for Value {
    fn from(y: YexType) -> Self {
        Value::Type(GcRef::new(y))
    }
}

impl From<List> for Value {
    fn from(l: List) -> Self {
        Value::List(l)
    }
}

impl From<Table> for Value {
    fn from(t: Table) -> Self {
        Value::Table(t)
    }
}

/// Immediate values that can be consumed
#[derive(Debug, PartialEq)]
pub enum Value {
    /// float-precision numbers
    Num(f64),
    /// Strings
    Str(GcRef<String>),
    /// erlang-like atoms
    Sym(YexSymbol),
    /// Booleans
    Bool(bool),
    /// Fnctions
    Fn(GcRef<Fn>),
    /// Tables
    Table(Table),
    /// Yex lists
    List(List),
    /// Yex user-defined types
    Type(GcRef<YexType>),
    /// Yex instances
    Instance(GcRef<Instance>),
    /// Tuples
    Tuple(Tuple),
    /// null
    Nil,
}

impl Clone for Value {
    fn clone(&self) -> Self {
        use Value::*;

        match self {
            List(xs) => List(xs.clone()),
            Str(str) => Str(GcRef::clone(str)),
            Fn(f) => Fn(GcRef::clone(f)),
            Bool(b) => Bool(*b),
            Num(n) => Num(*n),
            Sym(s) => Sym(*s),
            Type(t) => Type(t.clone()),
            Instance(i) => Instance(i.clone()),
            Table(t) => Table(t.clone()),
            Tuple(t) => Tuple(t.clone()),
            Nil => Nil,
        }
    }
}

impl Value {
    /// checks if the constant is `nil`
    pub fn is_nil(&self) -> bool {
        self == &Self::Nil
    }

    /// Returns the size of `self`
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            Value::List(xs) => xs.len(),
            Value::Num(_) => mem::size_of::<f64>(),
            Value::Sym(_) => mem::size_of::<Symbol>(),
            Value::Str(s) => s.len(),
            Value::Fn(f) => mem::size_of_val(&f),
            Value::Bool(_) => mem::size_of::<bool>(),
            Value::Type(t) => mem::size_of_val(&t),
            Value::Instance(i) => mem::size_of_val(&i),
            Value::Table(t) => t.items.len(),
            Value::Tuple(t) => t.len(),
            Value::Nil => 4,
        }
    }

    /// Compares the left and the right value
    pub fn ord_cmp(&self, rhs: &Self) -> InterpretResult<Ordering> {
        let (left, right) = match (self, rhs) {
            (Self::Num(left), Self::Num(right)) => (left, right),
            (l, r) => raise!(TypeError, "cmp not supported with '{}' and '{}'", l, r)?,
        };

        match left.partial_cmp(right) {
            Some(ord) => Ok(ord),
            None => raise!(TypeError, "Cannot compare '{}' and '{}'", left, right),
        }
    }

    /// Convert the constant to a boolean
    pub fn to_bool(&self) -> bool {
        use Value::*;

        match self {
            Bool(b) => *b,
            Sym(_) => true,
            Str(s) if s.is_empty() => false,
            Str(_) => true,
            Num(n) if *n == 0.0 => false,
            Num(_) => true,
            Nil => false,
            List(xs) => !xs.is_empty(),
            Fn(_) => true,
            Type(_) => true,
            Table(_) => true,
            Tuple(_) => true,
            Instance(_) => true,
        }
    }

    /// returns the type of the value
    pub fn type_of(&self) -> GcRef<YexType> {
        use Value::*;

        match self {
            Type(t) => return t.clone(),
            Instance(i) => return i.ty.clone(),
            _ => {}
        };

        let ty = match self {
            List(_) => YexType::list(),
            Fn(_) => YexType::fun(),
            Num(_) => YexType::num(),
            Str(_) => YexType::str(),
            Bool(_) => YexType::bool(),
            Nil => YexType::nil(),
            Sym(_) => YexType::sym(),
            Table(_) => YexType::table(),
            Tuple(_) => YexType::tuple(),
            Type(_) | Instance(_) => unreachable!(),
        };

        GcRef::new(ty)
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::Nil
    }
}

type ConstantErr = InterpretResult<Value>;

impl From<Value> for bool {
    fn from(o: Value) -> Self {
        o.to_bool()
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Value::*;
        let tk = match self {
            Fn(f) => format!("fn({})", f.arity),
            Nil => "nil".to_string(),
            List(xs) => format!("{}", *xs),
            Str(s) => "\"".to_owned() + s + "\"",
            Sym(s) => format!("{}", s),
            Num(n) => n.to_string(),
            Type(t) => format!("type({})", t.name),
            Instance(i) => format!("instance({})", i.ty.name),
            Table(t) => format!("{t}"),
            Tuple(t) => format!("{t}"),
            Bool(b) => b.to_string(),
        };
        write!(f, "{}", tk)
    }
}

macro_rules! impl_numeric {
    ($($t:ident $op:tt $fn:ident);+$(;)?) => {
        $(
            impl $t for Value {
                type Output = ConstantErr;

                fn $fn(self, rhs: Self) -> Self::Output {
                    match (self, rhs) {
                        (Self::Num(x), Self::Num(y)) => Ok(Self::Num(x $op y)),
                        (Self::Str(x), Self::Str(y)) => Ok(Self::Str(GcRef::new(x.to_string() + &y))),
                        (l, r) => raise!(TypeError, "Cannot apply '{}' operator between '{}' and '{}'", stringify!($t), l, r),
                    }
                }
            }
        )+
    }
}

impl_numeric!(
    Add + add;
    Sub - sub;
    Mul * mul;
    Div / div;
    Rem % rem;
);

macro_rules! impl_bit {
    ($($t:ident $op:tt $opname:literal $fn:ident);+ $(;)? ) => {
        $(
            impl $t for Value {
                type Output = ConstantErr;

                fn $fn(self, rhs: Self) -> Self::Output {
                    match (self, rhs) {
                        (Self::Num(x), Self::Num(y)) if x.fract() == 0.0 && y.fract() == 0.0 => Ok(Self::Num(((x as u64) $op (y as u64)) as f64)),
                        (Self::Str(x), Self::Str(y)) => Ok(Self::Str(GcRef::new(x.to_string() + &y))),
                        (l, r) => raise!(TypeError, "Cannot apply '{}' operator between '{}' and '{}'", $opname, l, r),
                    }
                }
            }
        )+
    }
}

impl_bit!(
    BitAnd & "&&&" bitand;
    BitOr | "|||" bitor;
    BitXor ^ "^^^" bitxor;
    Shl << "<<<" shl;
    Shr >> ">>>" shr;
);

impl Neg for Value {
    type Output = ConstantErr;

    fn neg(self) -> Self::Output {
        match self {
            Self::Num(n) => Ok(Self::Num(-n)),
            _ => raise!(TypeError, "Cannot apply '-' operator on '{}'", self),
        }
    }
}

impl Not for Value {
    type Output = Value;

    fn not(self) -> Self::Output {
        Self::Bool(!self.to_bool())
    }
}

pub trait TryGet<T> {
    fn get(&self) -> InterpretResult<T>;
}

macro_rules! impl_get {
    ($to:ty: $pattern:tt) => {
        impl TryGet<$to> for Value {
            #[inline(always)]
            fn get(&self) -> InterpretResult<$to> {
                match self {
                    Self::$pattern(x) => Ok(x.clone()),
                    _ => crate::raise!(TypeError, "Unexpected type '{}'", self.type_of().name),
                }
            }
        }
    };
    ($to:ty: $pattern:pat => $parse_expr:expr) => {
        impl TryGet<$to> for Value {
            #[inline(always)]
            fn get(&self) -> InterpretResult<$to> {
                use Value::*;
                match self {
                    $pattern => Ok($parse_expr),
                    _ => crate::raise!(TypeError, "Unexpected type '{}'", self.type_of().name),
                }
            }
        }
    };
}

impl_get!(String: Str(s) => s.to_string());
impl_get!(f64: Num);
impl_get!(bool: Bool);
impl_get!(GcRef<YexType>: Type);
impl_get!(GcRef<Fn>: Fn);
impl_get!(GcRef<Instance>: Instance);
impl_get!(Table: Table);
impl_get!(Symbol: Sym(s) => s.0);
impl_get!(List: List);
impl_get!(Tuple: Tuple);
impl_get!(usize: Num(n) => {
    if n.fract() != 0.0 || n.is_nan() || n.is_infinite() || *n < 0.0 {
        return crate::raise!(ValueError, "Expected a positive integer, got '{}'", n);
    }

    n.round() as usize
});

impl_get!(isize: Num(n) => {
    if n.fract() != 0.0 || n.is_nan() || n.is_infinite() {
        return crate::raise!(ValueError, "Expected an integer, got '{}'", n);
    }

    n.round() as isize
});
