//! Ordered borrowed declarations. The projected variant preserves the exact
//! ordinary view/iterator carrier size while lending source-held payloads.
pub(crate) trait DeclarationDirectory<T> {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> Option<&T>;
}
pub(crate) enum DeclSlices<'a, T> {
    Ordinary {
        authored: &'a [T],
        indices: Option<&'a [usize]>,
        generated: &'a [T],
    },
    Projected {
        authored: &'a [T],
        directory: &'a dyn DeclarationDirectory<T>,
        start: usize,
    },
}
impl<'a, T> DeclSlices<'a, T> {
    pub(crate) fn one(authored: &'a [T]) -> Self {
        Self::Ordinary {
            authored,
            indices: None,
            generated: &[],
        }
    }
    pub(super) fn append(authored: &'a [T], generated: &'a [T]) -> Option<Self> {
        authored.len().checked_add(generated.len())?;
        Some(Self::Ordinary {
            authored,
            indices: None,
            generated,
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
        indices.len().checked_add(generated.len())?;
        Some(Self::Ordinary {
            authored,
            indices: Some(indices),
            generated,
        })
    }
    pub(super) fn projected(
        authored: &'a [T],
        directory: &'a dyn DeclarationDirectory<T>,
    ) -> Option<Self> {
        authored.len().checked_add(directory.len())?;
        Some(Self::Projected {
            authored,
            directory,
            start: 0,
        })
    }
    fn authored_len(&self) -> usize {
        match self {
            Self::Ordinary {
                authored, indices, ..
            } => indices.map_or(authored.len(), <[usize]>::len),
            Self::Projected { authored, .. } => authored.len(),
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Ordinary { generated, .. } => self.authored_len() + generated.len(),
            Self::Projected {
                directory, start, ..
            } => self.authored_len() + directory.len() - start,
        }
    }
    pub(crate) fn first(&self) -> Option<&'a T> {
        self.get(0)
    }
    pub(crate) fn get(&self, index: usize) -> Option<&'a T> {
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let count = indices.map_or(authored.len(), <[usize]>::len);
                if index < count {
                    authored.get(indices.map_or(index, |indices| indices[index]))
                } else {
                    generated.get(index.checked_sub(count)?)
                }
            }
            Self::Projected {
                authored,
                directory,
                start,
            } => {
                if index < authored.len() {
                    authored.get(index)
                } else if index < self.len() {
                    directory.get(start + index - authored.len())
                } else {
                    None
                }
            }
        }
    }
    pub(crate) fn suffix(&self, start: usize) -> Option<Self> {
        if start > self.len() {
            return None;
        }
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let count = self.authored_len();
                if start < count {
                    match indices {
                        Some(indices) => Self::selected(authored, &indices[start..], generated),
                        None => Self::append(&authored[start..], generated),
                    }
                } else {
                    Some(Self::one(&generated[start - count..]))
                }
            }
            Self::Projected {
                authored,
                directory,
                start: offset,
            } => {
                if start < authored.len() {
                    Some(Self::Projected {
                        authored: &authored[start..],
                        directory: *directory,
                        start: *offset,
                    })
                } else {
                    Some(Self::Projected {
                        authored: &[],
                        directory: *directory,
                        start: offset + start - authored.len(),
                    })
                }
            }
        }
    }
    pub(crate) fn iter(&self) -> DeclIter<'a, T> {
        DeclIter {
            view: match self {
                Self::Ordinary {
                    authored,
                    indices,
                    generated,
                } => Self::Ordinary {
                    authored,
                    indices: *indices,
                    generated,
                },
                Self::Projected {
                    authored,
                    directory,
                    start,
                } => Self::Projected {
                    authored,
                    directory: *directory,
                    start: *start,
                },
            },
            front: 0,
            back: self.len(),
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

#[cfg(test)]
#[path = "decl_slices/alias_tests.rs"]
mod alias_tests;
#[cfg(test)]
#[path = "decl_slices/before_alias.rs"]
mod before_alias;
