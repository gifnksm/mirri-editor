use crate::decode::Input;
use std::{
    collections::{hash_map::Entry, HashMap, VecDeque},
    fmt::{Debug, Formatter, Result as FmtResult},
};

pub(crate) enum Action<T, U> {
    Func(Box<dyn FnMut(T) -> U>),
    KeyMap(Box<KeyMap<T, U>>),
}

impl<T, U> Debug for Action<T, U>
where
    T: Debug,
    U: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Self::Func(_) => write!(f, "Func(..)"),
            Self::KeyMap(km) => write!(f, "KeyMap({:?})", km),
        }
    }
}

#[derive(Debug)]
pub(crate) struct KeyMap<T, U> {
    map: HashMap<Input, Action<T, U>>,
}

impl<T, U> Default for KeyMap<T, U> {
    fn default() -> Self {
        KeyMap {
            map: HashMap::new(),
        }
    }
}

impl<T, U> KeyMap<T, U> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert(
        &mut self,
        mut inputs: impl Iterator<Item = Input> + Clone,
        act: Box<dyn FnMut(T) -> U>,
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
                Action::KeyMap(km) => km.insert(inputs, act).map(|(mut is, old)| {
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
                e.insert(Action::KeyMap(Box::new(km)));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::InputStrExt;
    use matches::assert_matches;

    #[test]
    fn insert() {
        let mut km = KeyMap::new();
        assert!(km
            .insert(
                "C-x C-x C-x".inputs().map(|i| i.unwrap()),
                Box::new(|()| ()),
            )
            .is_none());

        assert!(km
            .insert(
                "C-x C-x C-y".inputs().map(|i| i.unwrap()),
                Box::new(|()| ()),
            )
            .is_none());

        let (is, act) = km
            .insert(
                "C-x C-x C-x".inputs().map(|i| i.unwrap()),
                Box::new(|()| ()),
            )
            .unwrap();
        assert!(is
            .into_iter()
            .eq("C-x C-x C-x".inputs().map(|i| i.unwrap())));
        assert_matches!(act, Action::Func(..));

        let (is, act) = km
            .insert("C-x C-x".inputs().map(|i| i.unwrap()), Box::new(|()| ()))
            .unwrap();
        assert!(is.into_iter().eq("C-x C-x".inputs().map(|i| i.unwrap())));
        assert_matches!(act, Action::KeyMap(..));

        let (is, act) = km
            .insert(
                "C-x C-x C-z".inputs().map(|i| i.unwrap()),
                Box::new(|()| ()),
            )
            .unwrap();
        assert!(is.into_iter().eq("C-x C-x".inputs().map(|i| i.unwrap())));
        assert_matches!(act, Action::Func(..));
    }
}
