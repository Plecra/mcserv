
#[derive(Debug)]
pub struct SlotMap<T> {
    head: u32,
    entries: Vec<Result<T, u32>>,
}
impl<T> SlotMap<T> {
    pub fn new() -> Self {
        Self {
            head: u32::MAX,
            entries: vec![],
        }
    }
    pub fn get(&mut self, i: usize) -> Option<&mut T> {
        self.entries.get_mut(i).and_then(|r| r.as_mut().ok())
    }
    pub fn next_idx(&self) -> usize {
        if self.head == u32::MAX {
            self.entries.len()
        } else {
            self.head as usize
        }
    }
    pub fn insert(&mut self, value: T) -> usize {
        if self.head == u32::MAX {
            let i = self.entries.len();
            self.entries.push(Ok(value));
            i
        } else {
            let id = self.head as usize;
            let slot = &mut self.entries[id as usize];
            self.head = core::mem::replace(slot, Ok(value))
                .map(|_| ()).expect_err("corrupted slotmap");
            id
        }
    }
    pub fn release(&mut self, i: usize) -> Option<T> {
        self.entries.get_mut(i).and_then(|r| {
            match core::mem::replace(r, Err(self.head)) {
                Ok(v) => {
                    self.head = i as u32;
                    Some(v)
                }
                Err(i) => {
                    *r = Err(i);
                    None
                }
            }
        })
    }
}
pub struct IterMut<'a, T> {
    entries: core::iter::Enumerate<core::slice::IterMut<'a, Result<T, u32>>>,
}
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (usize, &'a mut T);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((i, v)) = self.entries.next() {
            if let Ok(v) = v {
                return Some((i, v));
            }
        }
        None
    }
}
impl<T> SlotMap<T> {
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut { entries: self.entries.iter_mut().enumerate() }
    }
    pub fn retain(&mut self, mut f: impl FnMut(usize, &mut T) -> bool) {
        for (i, entry) in self.entries.iter_mut().enumerate() {
            if let Ok(item) = entry {
                if !f(i, item) {
                    *entry = Err(core::mem::replace(&mut self.head, i as u32));
                }
            }
        }
    }
}