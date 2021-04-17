use crate::blob;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(usize);

pub struct Table {
    hash: std::collections::HashMap<blob::Blob, Symbol>,
    ids: Vec<blob::Blob>,
}

impl Table {
    pub fn new() -> Table {
        let hash = std::collections::HashMap::new();
        let ids = vec![];
        Table { hash, ids }
    }

    pub fn insert(&mut self, bytes: &blob::View) -> Symbol {
        match self.hash.get(bytes) {
            Some(id) => *id,
            None => {
                let id = Symbol(self.ids.len());
                self.hash.insert(blob::Blob::new(bytes), id);
                self.ids.push(blob::Blob::new(bytes));
                id
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplication() {
        let mut arena = Table::new();
        let a_bytes = b"aaaaa";
        let b_bytes = b"bbbbb";
        let mut ids = vec![];
        for _ in 0..10 {
            let a = blob::Blob::new(a_bytes);
            let a_id = arena.insert(&a);
            ids.push(a_id);

            let b = blob::Blob::new(b_bytes);
            let b_id = arena.insert(&b);
            ids.push(b_id);

            assert!(a_id != b_id);
        }

        let ids: std::collections::HashSet<Symbol> = ids.iter().cloned().collect();
        assert!(ids.len() == 2);
    }
}
