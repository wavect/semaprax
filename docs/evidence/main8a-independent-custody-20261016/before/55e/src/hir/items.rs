//! Read-only sequence access; indices are access data, never validation authority.
use std::ops::Range;

pub(crate) trait ItemSource<T> {
    fn view(&self) -> View<'_, T>;
    fn len(&self) -> usize {
        self.view().len()
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn iter(&self) -> Iter<'_, T> {
        self.view().iter()
    }
}

pub(crate) struct View<'a, T> {
    source: &'a [T],
    ordinals: Option<&'a [usize]>,
}
impl<T> Copy for View<'_, T> {}
impl<T> Clone for View<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T> View<'a, T> {
    pub(crate) fn slice(source: &'a [T]) -> Self {
        Self {
            source,
            ordinals: None,
        }
    }
    pub(crate) fn indexed(source: &'a [T], ordinals: &'a [usize]) -> Self {
        Self {
            source,
            ordinals: Some(ordinals),
        }
    }
    pub(crate) fn len(self) -> usize {
        self.ordinals.map_or(self.source.len(), <[usize]>::len)
    }
    pub(crate) fn get(self, index: usize) -> Option<&'a T> {
        match self.ordinals {
            None => self.source.get(index),
            Some(ordinals) => self.source.get(*ordinals.get(index)?),
        }
    }
    pub(crate) fn iter(self) -> Iter<'a, T> {
        Iter {
            view: self,
            range: 0..self.len(),
        }
    }
}
impl<T> ItemSource<T> for [T] {
    fn view(&self) -> View<'_, T> {
        View::slice(self)
    }
}
impl<T> ItemSource<T> for Vec<T> {
    fn view(&self) -> View<'_, T> {
        View::slice(self)
    }
}
impl<T, const N: usize> ItemSource<T> for [T; N] {
    fn view(&self) -> View<'_, T> {
        View::slice(self)
    }
}
impl<T> ItemSource<T> for View<'_, T> {
    fn view(&self) -> View<'_, T> {
        *self
    }
}
pub(crate) struct Iter<'a, T> {
    view: View<'a, T>,
    range: Range<usize>,
}
impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Self {
            view: self.view,
            range: self.range.clone(),
        }
    }
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        self.view.get(self.range.next()?)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}
impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.view.get(self.range.next_back()?)
    }
}
impl<T> ExactSizeIterator for Iter<'_, T> {}
impl<'a, T> IntoIterator for View<'a, T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
