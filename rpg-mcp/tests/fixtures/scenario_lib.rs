//! A multi-definition Rust fixture with known relationships for MCP tool
//! scenario tests. Every relationship here is asserted by a test, so changes
//! to the encoder that break edge resolution will fail loudly.
//!
//! Structure:
//! - trait PaymentProcessor (find/save/refund methods)
//! - struct StripeProcessor implements PaymentProcessor
//! - struct PaypalProcessor implements PaymentProcessor
//! - fn process_payment(processor) calls processor.find + processor.save
//! - fn refund_payment(processor) calls processor.refund
//! - fn main() calls process_payment + refund_payment
//! - dead function: unused_helper (never called)
//! - #[no_mangle] extern "C" export: rpg_exported_fn
//! - extern "C" { fn rpg_imported_fn(); } import

pub trait PaymentProcessor {
    fn find(&self, id: u64) -> Option<u64>;
    fn save(&mut self, amount: u64) -> bool;
    fn refund(&mut self, id: u64) -> bool;
}

pub struct StripeProcessor {
    transactions: Vec<u64>,
}

impl PaymentProcessor for StripeProcessor {
    fn find(&self, id: u64) -> Option<u64> {
        self.transactions.iter().find(|t| **t == id).copied()
    }
    fn save(&mut self, amount: u64) -> bool {
        self.transactions.push(amount);
        true
    }
    fn refund(&mut self, id: u64) -> bool {
        let before = self.transactions.len();
        self.transactions.retain(|t| *t != id);
        self.transactions.len() < before
    }
}

pub struct PaypalProcessor {
    balance: u64,
}

impl PaymentProcessor for PaypalProcessor {
    fn find(&self, id: u64) -> Option<u64> {
        if id == self.balance { Some(self.balance) } else { None }
    }
    fn save(&mut self, amount: u64) -> bool {
        self.balance += amount;
        true
    }
    fn refund(&mut self, id: u64) -> bool {
        self.balance = self.balance.saturating_sub(id);
        true
    }
}

pub fn process_payment(processor: &mut dyn PaymentProcessor, amount: u64) -> bool {
    processor.save(amount);
    processor.find(amount).is_some()
}

pub fn refund_payment(processor: &mut dyn PaymentProcessor, id: u64) -> bool {
    processor.refund(id)
}

pub fn unused_helper() -> u64 {
    // This function is never called — candidate for dead-code detection.
    42
}

#[no_mangle]
pub extern "C" fn rpg_exported_fn(x: u64) -> u64 {
    x.wrapping_mul(2)
}

extern "C" {
    fn rpg_imported_fn();
}

pub fn call_imported() {
    unsafe {
        rpg_imported_fn();
    }
}
