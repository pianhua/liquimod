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
