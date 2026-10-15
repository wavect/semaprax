//! Borrowed ordered declarations: an authored selection and generated suffix.
//! Numeric ordinals borrow the exact retained owner; no reference directory.
pub(crate) struct DeclSlices<'a, T> {
    authored: &'a [T],
    indices: Option<&'a [usize]>,
    generated: &'a [T],
    length: usize,
}
impl<'a, T> DeclSlices<'a, T> {
    pub(crate) fn one(authored: &'a [T]) -> Self {
        Self {
            authored,
            indices: None,
            generated: &[],
            length: authored.len(),
        }
    }
    pub(super) fn append(authored: &'a [T], generated: &'a [T]) -> Option<Self> {
        Some(Self {
            authored,
            indices: None,
            generated,
            length: authored.len().checked_add(generated.len())?,
        })
    }
    pub(super) fn selected(
        authored: &'a [T],
        indices: &'a [usize],
        generated: &'a [T],
    ) -> Option<Self> {
        if indices.iter().any(|index| *index >= authored.len())
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return None;
        }
        Some(Self {
            authored,
            indices: Some(indices),
            generated,
            length: indices.len().checked_add(generated.len())?,
        })
    }
    fn authored_len(&self) -> usize {
        self.indices.map_or(self.authored.len(), <[usize]>::len)
    }
    pub(crate) fn len(&self) -> usize {
        self.length
    }
    pub(crate) fn first(&self) -> Option<&'a T> {
        self.get(0)
    }
    pub(crate) fn get(&self, index: usize) -> Option<&'a T> {
        let authored_len = self.authored_len();
        if index < authored_len {
            self.authored
                .get(self.indices.map_or(index, |indices| indices[index]))
        } else {
            self.generated.get(index.checked_sub(authored_len)?)
        }
    }
    pub(crate) fn suffix(&self, start: usize) -> Option<Self> {
        if start > self.length {
            return None;
        }
        let authored_len = self.authored_len();
        if start < authored_len {
            match self.indices {
                Some(indices) => Self::selected(self.authored, &indices[start..], self.generated),
                None => Self::append(&self.authored[start..], self.generated),
            }
        } else {
            Some(Self::one(&self.generated[start - authored_len..]))
        }
    }
    pub(crate) fn iter(&self) -> DeclIter<'a, T> {
        DeclIter {
            view: Self {
                authored: self.authored,
                indices: self.indices,
                generated: self.generated,
                length: self.length,
            },
            front: 0,
            back: self.length,
        }
    }
}
impl<T> std::ops::Index<usize> for DeclSlices<'_, T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("declaration index in bounds")
    }
}
pub(crate) struct DeclIter<'a, T> {
    view: DeclSlices<'a, T>,
    front: usize,
    back: usize,
}
impl<'a, T> Iterator for DeclIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let next = self.view.get(self.front);
        self.front += 1;
        next
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }
}
impl<T> DoubleEndedIterator for DeclIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        self.view.get(self.back)
    }
}
impl<T> ExactSizeIterator for DeclIter<'_, T> {}
impl<T> std::iter::FusedIterator for DeclIter<'_, T> {}
impl<'a, T> IntoIterator for DeclSlices<'a, T> {
    type Item = &'a T;
    type IntoIter = DeclIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
