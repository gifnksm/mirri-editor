pub(crate) trait SliceExt {
    type Item;

    fn get3(&self, at: usize) -> [Option<&Self::Item>; 3];
    fn get3_mut(&mut self, at: usize) -> [Option<&mut Self::Item>; 3];
}

impl<T> SliceExt for [T] {
    type Item = T;

    fn get3(&self, at: usize) -> [Option<&T>; 3] {
        let (head, tail) = self.split_at(at);
        let prev = head.last();
        if let Some((elem, tail)) = tail.split_first() {
            [prev, Some(elem), tail.first()]
        } else {
            [prev, None, None]
        }
    }

    fn get3_mut(&mut self, at: usize) -> [Option<&mut T>; 3] {
        let (head, tail) = self.split_at_mut(at);
        let prev = head.last_mut();
        if let Some((elem, tail)) = tail.split_first_mut() {
            [prev, Some(elem), tail.first_mut()]
        } else {
            [prev, None, None]
        }
    }
}
