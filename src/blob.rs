use std::borrow::Borrow;
use std::ops::Deref;

pub type View = [u8];

#[derive(Eq, PartialEq, Hash)]
pub struct Blob {
    bytes: Box<View>,
}

impl Blob {
    pub fn new(view: &View) -> Blob {
        let mut vec = vec![];
        vec.extend_from_slice(view);
        Blob {
            bytes: vec.into_boxed_slice(),
        }
    }

    pub fn empty() -> Blob {
        let vec = vec![];
        Blob {
            bytes: vec.into_boxed_slice(),
        }
    }

    pub fn view(&self) -> &View {
        &*self.bytes
    }
}

impl Borrow<View> for Blob {
    fn borrow(&self) -> &View {
        &self.bytes
    }
}

impl Deref for Blob {
    type Target = View;

    fn deref(&self) -> &View {
        &self.bytes
    }
}

pub struct Builder {
    bytes: Vec<u8>,
}

impl Builder {
    pub fn new() -> Builder {
        Builder { bytes: vec![] }
    }

    pub fn blob(self) -> Blob {
        Blob {
            bytes: self.bytes.into_boxed_slice(),
        }
    }

    pub fn push(&mut self, byte: u8) {
        self.bytes.push(byte)
    }

    pub fn extend(&mut self, bytes: &View) {
        self.bytes.extend_from_slice(bytes)
    }
}
