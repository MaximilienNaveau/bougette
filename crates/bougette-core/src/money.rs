//! Amounts of money: whole minor units (cents) with their currency.
//!
//! Never floating point: 0.1 + 0.2 != 0.3 in binary floats, and a budget
//! must add up to the cent.

// `use` imports names into this file, like `using std::fmt;` in C++.
// `std::ops` holds the traits behind the operators (+, -, unary -).
use std::fmt;
use std::ops::{Add, Neg, Sub};

/// A currency, by its ISO 4217 code, e.g. `EUR`.
///
/// `#[derive(...)]` asks the compiler to write standard trait
/// implementations for us. A trait is a set of methods a type promises to
/// provide, close to a C++ concept or an abstract base class, except that
/// the implementation can be added outside the type's definition.
/// - `Debug`: printable with `{:?}`, for tests and error messages;
/// - `Clone, Copy`: copied bit by bit on assignment, like a C++ POD.
///   Without `Copy`, `let b = a;` would *move* `a` (as if `std::move`
///   were the default) and using `a` afterwards would not compile;
/// - `PartialEq, Eq`: `==` and `!=`;
/// - `Hash`: usable as a key in a hash map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// A "tuple struct": a struct whose single field has no name, accessed as
// `self.0`. The field is private (no `pub`), so the only way to build a
// Currency is through the constructors below, which check the code.
pub struct Currency([u8; 3]);

// `impl Type { ... }` holds the type's methods, like the member functions
// of a C++ class, but written apart from the data.
impl Currency {
    /// The euro, the default currency of a new budget.
    // An associated constant, like a `static constexpr` member.
    pub const EUR: Currency = Currency(*b"EUR");

    /// Reads an ISO 4217 code: three uppercase ASCII letters.
    ///
    /// Returns `None` if `code` is not one. `Option<T>` is Rust's
    /// replacement for null pointers and sentinel values: it is either
    /// `Some(value)` or `None`, and the compiler makes the caller handle
    /// both cases before using the value (like `std::optional`, but with no
    /// way to dereference an empty one by mistake).
    pub fn from_code(code: &str) -> Option<Currency> {
        // `&str` is a borrowed, read-only view of UTF-8 text, like
        // `std::string_view`. `.as_bytes()` views the same memory as bytes.
        let bytes = code.as_bytes();
        // `.iter().all(...)` is `std::all_of`; `|b| ...` is a lambda
        // (a "closure") taking each byte by reference.
        if bytes.len() == 3 && bytes.iter().all(|b| b.is_ascii_uppercase()) {
            // Copy the three bytes into a fixed-size array. The last
            // expression of a block, with no `;`, is its value: here the
            // value returned by the function.
            Some(Currency([bytes[0], bytes[1], bytes[2]]))
        } else {
            None
        }
    }

    /// The ISO 4217 code, e.g. `"EUR"`.
    // `&self` is a borrowed, read-only `this` (like a `const` member
    // function). The returned `&str` borrows from `self`: the compiler
    // checks it is never used after `self` is gone (no dangling reference),
    // a check C++ doesn't do.
    pub fn code(&self) -> &str {
        // Only from_code and EUR build a Currency, so the bytes are always
        // ASCII and the conversion cannot fail: `expect` would panic with
        // this message if it ever did. A panic is an unrecoverable error
        // for programming bugs, like a failed `assert`.
        std::str::from_utf8(&self.0).expect("a currency code is ASCII")
    }
}

/// An amount of money: a whole number of minor units (cents for the euro)
/// in a given currency. Negative amounts are allowed, e.g. a refund.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money {
    // `i64`: a signed 64-bit integer, like `int64_t`. Rust has no implicit
    // integer conversions: mixing i64 and i32 needs an explicit cast.
    minor_units: i64,
    currency: Currency,
}

impl Money {
    /// An amount in minor units: `Money::new(1234, Currency::EUR)` is €12.34.
    // A constructor in Rust is just a function returning `Self` (the type
    // being implemented). `new` is a naming convention, not a keyword.
    pub fn new(minor_units: i64, currency: Currency) -> Self {
        // Field init shorthand: `minor_units` means `minor_units: minor_units`.
        Money {
            minor_units,
            currency,
        }
    }

    /// Zero in the given currency.
    pub fn zero(currency: Currency) -> Self {
        Money::new(0, currency)
    }

    /// The amount in minor units (cents).
    pub fn minor_units(&self) -> i64 {
        self.minor_units
    }

    /// The currency of the amount.
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// `self × numerator / denominator`, rounded to the nearest minor unit,
    /// halves away from zero: €4.80 × 1 / 12 is €0.40, and 1 cent / 2 is
    /// 1 cent.
    ///
    /// Used to spread an amount over a period (smoothing), with a single
    /// rounding at the end. Panics if `denominator` is not positive.
    // `self` (not `&self`): the method takes the value itself. Money is
    // `Copy`, so the caller keeps its own copy.
    pub fn mul_div_rounded(self, numerator: i64, denominator: i64) -> Money {
        // `assert!` checks in release builds too (unlike C's assert).
        assert!(denominator > 0, "denominator must be positive");
        // Compute in 128 bits so `minor_units * numerator` cannot overflow.
        // `as` is an explicit numeric cast, like static_cast.
        let product = self.minor_units as i128 * numerator as i128;
        let denominator = denominator as i128; // shadows the i64 parameter
        // Rust's integer `/` truncates towards zero and `%` keeps the sign
        // of the left operand, as in C++.
        let quotient = product / denominator;
        let remainder = product % denominator;
        // Round half away from zero: move one unit further from zero when
        // the remainder is at least half of the denominator.
        // `if` is an expression, so it can initialise a variable (like
        // C++'s ternary operator, but for whole blocks).
        let rounded = if 2 * remainder.abs() >= denominator {
            // `.signum()` is -1, 0 or 1.
            quotient + product.signum()
        } else {
            quotient
        };
        // Back to 64 bits. `try_from` fails if the value doesn't fit;
        // amounts that large are a bug, so we panic.
        let minor_units = i64::try_from(rounded).expect("amount overflows i64");
        Money::new(minor_units, self.currency)
    }
}

// Operator overloading is done by implementing the trait behind the
// operator: `impl Add for Money` defines `Money + Money`, like
// `Money operator+(Money, Money)` in C++.
impl Add for Money {
    // The type `a + b` produces: an "associated type" of the trait.
    type Output = Money;

    fn add(self, other: Money) -> Money {
        // A budget has a single currency, so mixing two is a programming
        // error: panic rather than return a wrong sum.
        assert_eq!(self.currency, other.currency, "adding different currencies");
        // In debug builds, integer overflow panics instead of wrapping
        // silently (in C++ signed overflow is undefined behaviour).
        Money::new(self.minor_units + other.minor_units, self.currency)
    }
}

impl Sub for Money {
    type Output = Money;

    fn sub(self, other: Money) -> Money {
        // Reuse Add and Neg: a - b = a + (-b).
        self + -other
    }
}

/// Unary minus: `-amount`.
impl Neg for Money {
    type Output = Money;

    fn neg(self) -> Money {
        Money::new(-self.minor_units, self.currency)
    }
}

/// Formats as `-12.34 EUR`, for the command line. Formatting for people
/// (thousands separators, decimal comma, symbol) is the UI's job.
// `fmt::Display` is the trait `{}` uses in `format!`/`println!`, like
// overloading `operator<<` for an ostream.
impl fmt::Display for Money {
    // `&mut fmt::Formatter` is a mutable borrow (like `Formatter&`): the
    // compiler guarantees nothing else uses the formatter meanwhile.
    // `fmt::Result` says whether writing succeeded.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sign = if self.minor_units < 0 { "-" } else { "" };
        // `unsigned_abs` avoids overflowing on i64::MIN.
        let cents = self.minor_units.unsigned_abs();
        // `write!` is `format!` into the formatter; `{:02}` pads with zeros.
        // It returns a fmt::Result, which becomes this function's result.
        write!(
            f,
            "{sign}{}.{:02} {}",
            cents / 100,
            cents % 100,
            self.currency.code()
        )
    }
}

// Unit tests live next to the code, in a module compiled only for
// `cargo test` (`#[cfg(test)]` is like `#ifdef TESTING`).
#[cfg(test)]
mod tests {
    // `super` is the parent module (this file's code): import all of it.
    use super::*;

    // Each `#[test]` function is one test case; a panic (failed
    // assertion) fails it.
    #[test]
    fn currency_codes_are_three_uppercase_letters() {
        assert_eq!(Currency::from_code("EUR"), Some(Currency::EUR));
        assert_eq!(
            Currency::from_code("CHF").map(|c| c.code().to_string()),
            Some("CHF".to_string())
        );
        assert_eq!(Currency::from_code("eur"), None);
        assert_eq!(Currency::from_code("EURO"), None);
    }

    #[test]
    fn adds_and_subtracts() {
        let a = Money::new(1050, Currency::EUR);
        let b = Money::new(250, Currency::EUR);
        assert_eq!(a + b, Money::new(1300, Currency::EUR));
        assert_eq!(b - a, Money::new(-800, Currency::EUR));
    }

    // `should_panic` makes the test pass only if the code panics.
    #[test]
    #[should_panic(expected = "different currencies")]
    fn refuses_to_add_different_currencies() {
        let chf = Currency::from_code("CHF").unwrap();
        let _ = Money::new(1, Currency::EUR) + Money::new(1, chf);
    }

    #[test]
    fn spreads_with_one_rounding_half_away_from_zero() {
        let eur = |cents| Money::new(cents, Currency::EUR);
        // €480 a year is €40 a month.
        assert_eq!(eur(48000).mul_div_rounded(1, 12), eur(4000));
        // €100 a month over 3 months: 33.333… rounds down.
        assert_eq!(eur(10000).mul_div_rounded(1, 3), eur(3333));
        // €200 / 3 = 66.666… rounds up.
        assert_eq!(eur(20000).mul_div_rounded(1, 3), eur(6667));
        // Exact halves go away from zero, on both sides.
        assert_eq!(eur(1).mul_div_rounded(1, 2), eur(1));
        assert_eq!(eur(-1).mul_div_rounded(1, 2), eur(-1));
    }

    #[test]
    fn displays_with_two_decimals() {
        assert_eq!(Money::new(123456, Currency::EUR).to_string(), "1234.56 EUR");
        assert_eq!(Money::new(-5, Currency::EUR).to_string(), "-0.05 EUR");
    }
}
