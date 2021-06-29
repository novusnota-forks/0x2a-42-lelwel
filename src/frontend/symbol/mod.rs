mod imp;

use bumpalo::Bump;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

/// A thread unique symbol for a character string.
///
/// A string table is used to identify strings by an integer.
/// This is useful to cheaply compare names in later stages of a compiler.
/// Once a string was converted to a symbol it will remain allocated
/// in the interner for the lifetime of the thread.
///
/// # Examples
/// Basic usage:
/// ```
/// # use lelwel::frontend::symbol::*;
/// let foo1 = "foo".into_symbol();
/// let foo2 = "foo".into_symbol();
/// let bar = "bar".into_symbol();
///
/// assert_eq!(foo1, foo2);
/// assert_ne!(foo1, bar);
/// ```
#[derive(Hash, Ord, PartialOrd, PartialEq, Eq, Copy, Clone, Default)]
pub struct Symbol(pub u32);

impl Symbol {
    pub fn is_empty(self) -> bool {
        self == Symbol::EMPTY
    }
    pub fn as_str(self) -> &'static str {
        STRTBL.with(|table| table.borrow().get_string(self))
    }
    pub fn as_string(self) -> String {
        self.as_str().to_string()
    }
    #[allow(dead_code)]
    pub(crate) fn reset() {
        STRTBL.with(|table| table.borrow_mut().reset())
    }
    #[allow(dead_code)]
    pub(crate) fn allocated_bytes() -> usize {
        STRTBL.with(|table| table.borrow().allocated_bytes())
    }
}

/// A trait for converting a value to a `Symbol`.
pub trait ToSymbol {
    /// Converts the given value to a `Symbol`.
    ///
    /// # Examples
    /// Basic usage:
    /// ```
    /// # use lelwel::frontend::symbol::*;
    /// let name = "foo";
    /// let sym = Symbol::from(name);
    ///
    /// assert_eq!(sym, name.into_symbol());
    /// ```
    fn into_symbol(self) -> Symbol;
}

impl ToSymbol for String {
    fn into_symbol(self) -> Symbol {
        STRTBL.with(|table| table.borrow_mut().get_symbol(&self))
    }
}

impl ToSymbol for &str {
    fn into_symbol(self) -> Symbol {
        STRTBL.with(|table| table.borrow_mut().get_symbol(self))
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        s.into_symbol()
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        s.into_symbol()
    }
}

impl From<Symbol> for &str {
    fn from(s: Symbol) -> Self {
        s.as_str()
    }
}

impl From<Symbol> for String {
    fn from(s: Symbol) -> Self {
        s.as_str().to_string()
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = STRTBL.with(|symbol| symbol.borrow().get_string(*self));
        write!(f, "{}", s)
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Symbol::EMPTY {
            write!(f, "ɛ")
        } else {
            write!(f, "{}", self)
        }
    }
}

thread_local!(
    static STRTBL: RefCell<StringTable> = RefCell::new(StringTable::new())
);

/// A string table to manage `Symbol` creation.
struct StringTable {
    map: HashMap<&'static str, Symbol>,
    table: Vec<&'static str>,
    arena: Bump,
}

impl StringTable {
    fn new() -> Self {
        let mut symbol = Self {
            map: HashMap::new(),
            table: vec![],
            arena: Bump::new(),
        };
        symbol.init();
        symbol
    }
    fn reset(&mut self) {
        self.map.clear();
        self.table.clear();
        self.arena.reset();
    }
    fn allocated_bytes(&self) -> usize {
        self.arena.allocated_bytes()
    }
    fn alloc(&mut self, id: &str) -> Symbol {
        debug_assert!(!self.map.contains_key(id));
        debug_assert!(self.table.len() < u32::MAX as usize);
        let symbol = Symbol(self.table.len() as u32);
        let string = self.arena.alloc_str(id);
        // extend lifetime as the arena also has 'static lifetime
        let string: &'static str = unsafe { &*(string as *const str) };
        self.table.push(string);
        self.map.insert(string, symbol);
        symbol
    }
    fn get_symbol(&mut self, id: &str) -> Symbol {
        if !self.map.contains_key(id) {
            self.alloc(id)
        } else {
            self.map[id]
        }
    }
    fn get_string(&self, symbol: Symbol) -> &'static str {
        let Symbol(num) = symbol;
        self.table[num as usize]
    }
}
