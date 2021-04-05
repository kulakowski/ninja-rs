#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(usize);

pub struct Table {
    hash: std::collections::HashMap<Box<[u8]>, Symbol>,
    ids: Vec<Box<[u8]>>,
}

impl Table {
    pub fn new() -> Table {
        let hash = std::collections::HashMap::new();
        let ids = vec![];
        Table { hash, ids }
    }

    pub fn insert(&mut self, bytes: &[u8]) -> Symbol {
        match self.hash.get(bytes) {
            Some(id) => *id,
            None => {
                let id = Symbol(self.ids.len());
                self.hash.insert(box_slice(bytes), id);
                self.ids.push(box_slice(bytes));
                id
            }
        }
    }
}

fn box_slice(bytes: &[u8]) -> Box<[u8]> {
    let mut vec = vec![];
    vec.extend_from_slice(bytes);
    vec.into_boxed_slice()
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
            let a = box_slice(a_bytes);
            let a_id = arena.insert(&*a);
            ids.push(a_id);

            let b = box_slice(b_bytes);
            let b_id = arena.insert(&*b);
            ids.push(b_id);

            assert!(a_id != b_id);
        }

        let ids: std::collections::HashSet<Symbol> = ids.iter().cloned().collect();
        assert!(ids.len() == 2);
    }
}
