pub enum Status {
    Active,
    Inactive,
    Pending,
}

pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub status: Status,
}

pub trait Repository<T> {
    fn find(&self, id: u64) -> Option<T>;
    fn save(&mut self, entity: T) -> bool;
    fn delete(&mut self, id: u64) -> bool;
}

pub struct UserRepository {
    users: Vec<User>,
}

impl Repository<User> for UserRepository {
    fn find(&self, id: u64) -> Option<User> {
        self.users.iter().find(|u| u.id == id).cloned()
    }

    fn save(&mut self, entity: User) -> bool {
        self.users.push(entity);
        true
    }

    fn delete(&mut self, id: u64) -> bool {
        let len_before = self.users.len();
        self.users.retain(|u| u.id != id);
        self.users.len() < len_before
    }
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Cache<K, V> {
    data: std::collections::HashMap<K, V>,
}

impl<K: std::hash::Hash + Eq, V> Cache<K, V> {
    pub fn new() -> Self {
        Self {
            data: std::collections::HashMap::new(),
        }
    }
}
