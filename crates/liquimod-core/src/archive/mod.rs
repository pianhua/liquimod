pub mod detect;
pub mod zip_extract;

use crate::db::Database;
use crate::error::Result;

pub struct PasswordBook<'a> {
    db: &'a Database,
}

impl<'a> PasswordBook<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn candidates(&self) -> Result<Vec<String>> {
        self.db.list_passwords()
    }

    pub fn learn(&self, password: &str) -> Result<()> {
        self.db.add_password(password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_then_candidates() {
        let db = Database::open_in_memory().unwrap();
        let book = PasswordBook::new(&db);
        book.learn("x").unwrap();
        assert!(book.candidates().unwrap().contains(&"x".to_string()));
    }
}
