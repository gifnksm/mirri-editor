use crate::input::Input;
use derivative::Derivative;
use std::{
    cell::RefCell,
    collections::{hash_map::Entry, HashMap, VecDeque},
    rc::Rc,
};

#[derive(Derivative)]
#[derivative(Debug(bound = ""), Clone(bound = ""))]
pub(crate) enum Action<T, U> {
    Func(#[derivative(Debug = "ignore")] Rc<dyn FnMut(T) -> U>),
    KeyMap(Rc<RefCell<KeyMap<T, U>>>),
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""), Clone(bound = ""), Default(bound = ""))]
pub(crate) struct KeyMap<T, U> {
    map: HashMap<Input, Action<T, U>>,
}

impl<T, U> KeyMap<T, U> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get(&self, input: &Input) -> Option<Action<T, U>> {
        self.map.get(input).cloned()
    }

    pub(crate) fn insert(
        &mut self,
        mut inputs: impl Iterator<Item = Input> + Clone,
        act: Rc<dyn FnMut(T) -> U>,
    ) -> Option<(VecDeque<Input>, Action<T, U>)> {
        let input = inputs.next().unwrap();

        if inputs.clone().next().is_none() {
            return self.map.insert(input, Action::Func(act)).map(|old| {
                let mut is = VecDeque::new();
                is.push_front(input);
                (is, old)
            });
        }

        match self.map.entry(input) {
            Entry::Occupied(mut e) => match e.get_mut() {
                Action::KeyMap(km) => km.borrow_mut().insert(inputs, act).map(|(mut is, old)| {
                    is.push_front(input);
                    (is, old)
                }),
                Action::Func(..) => {
                    let old = e.insert(Action::Func(act));
                    let mut is = VecDeque::new();
                    is.push_front(input);
                    Some((is, old))
                }
            },
            Entry::Vacant(e) => {
                let mut km = KeyMap::new();
                let inserted = km.insert(inputs, act);
                assert!(inserted.is_none());
                e.insert(Action::KeyMap(Rc::new(RefCell::new(km))));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::InputStrExt;
    use matches::assert_matches;

    #[test]
    fn insert() {
        let mut km = KeyMap::new();
        assert!(km
            .insert("C-x C-x C-x".inputs().map(|i| i.unwrap()), Rc::new(|()| ()),)
            .is_none());

        assert!(km
            .insert("C-x C-x C-y".inputs().map(|i| i.unwrap()), Rc::new(|()| ()),)
            .is_none());

        let (is, act) = km
            .insert("C-x C-x C-x".inputs().map(|i| i.unwrap()), Rc::new(|()| ()))
            .unwrap();
        assert!(is
            .into_iter()
            .eq("C-x C-x C-x".inputs().map(|i| i.unwrap())));
        assert_matches!(act, Action::Func(..));

        let (is, act) = km
            .insert("C-x C-x".inputs().map(|i| i.unwrap()), Rc::new(|()| ()))
            .unwrap();
        assert!(is.into_iter().eq("C-x C-x".inputs().map(|i| i.unwrap())));
        assert_matches!(act, Action::KeyMap(..));

        let (is, act) = km
            .insert("C-x C-x C-z".inputs().map(|i| i.unwrap()), Rc::new(|()| ()))
            .unwrap();
        assert!(is.into_iter().eq("C-x C-x".inputs().map(|i| i.unwrap())));
        assert_matches!(act, Action::Func(..));
    }
}
