use std::marker::PhantomData;

pub struct Id<T> {
    id: usize,
    marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Copy for Id<T> {}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Id<T> {
        *self
    }
}

pub struct Arena<T> {
    items: Vec<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Arena<T> {
        Arena { items: vec![] }
    }

    pub fn insert(&mut self, t: T) -> Id<T> {
        let id = self.items.len();
        self.items.push(t);
        Id {
            id,
            marker: PhantomData,
        }
    }

    pub fn get(&self, id: Id<T>) -> &T {
        &self.items[id.id]
    }

    pub fn get_mut(&mut self, id: Id<T>) -> &mut T {
        &mut self.items[id.id]
    }
}
