//! Allocation-free ordered cursors; their bound follows from AVL construction.
use super::*;
// Minimum AVL counts satisfy N(h)=1+N(h-1)+N(h-2)>=1+2*N(h-2).
// Thus N(2m)>=3*2^(m-1)-1 (N(2)=2). At m=usize::BITS this
// exceeds usize::MAX, so every privately constructed tree has height
// < 2*usize::BITS. This is an address-space proof, not a program depth cap.
pub(super) const HEIGHT: usize = 2 * usize::BITS as usize;
pub(in crate::hir::validation) struct Paid<'a, K> {
    front: [Option<&'a Node<K>>; HEIGHT],
    back: [Option<&'a Node<K>>; HEIGHT],
    front_len: usize,
    back_len: usize,
    remaining: usize,
}
impl<'a, K> Paid<'a, K> {
    pub(super) fn new(root: Option<&'a Node<K>>, remaining: usize) -> Self {
        let mut cursor = Self {
            front: [None; HEIGHT],
            back: [None; HEIGHT],
            front_len: 0,
            back_len: 0,
            remaining,
        };
        cursor.push_front(root);
        cursor.push_back(root);
        cursor
    }
    fn push_front(&mut self, mut node: Option<&'a Node<K>>) {
        while let Some(current) = node {
            self.front[self.front_len] = Some(current);
            self.front_len += 1;
            node = current.left.as_deref();
        }
    }
    fn push_back(&mut self, mut node: Option<&'a Node<K>>) {
        while let Some(current) = node {
            self.back[self.back_len] = Some(current);
            self.back_len += 1;
            node = current.right.as_deref();
        }
    }
}
impl<'a, K> Iterator for Paid<'a, K> {
    type Item = &'a K;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.front_len -= 1;
        let node = self.front[self.front_len].take().unwrap();
        self.push_front(node.right.as_deref());
        self.remaining -= 1;
        Some(&node.key)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K> DoubleEndedIterator for Paid<'_, K> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.back_len -= 1;
        let node = self.back[self.back_len].take().unwrap();
        self.push_back(node.left.as_deref());
        self.remaining -= 1;
        Some(&node.key)
    }
}
impl<K> ExactSizeIterator for Paid<'_, K> {}
impl<K> std::iter::FusedIterator for Paid<'_, K> {}
#[allow(
    clippy::large_enum_variant,
    reason = "Paid traversal uses inline prepaid storage; boxing would allocate."
)]
pub(in crate::hir::validation) enum Iter<'a, K> {
    Paid(Paid<'a, K>),
    #[cfg(test)]
    Legacy(std::collections::btree_set::Iter<'a, K>),
}
impl<'a, K> Iterator for Iter<'a, K> {
    type Item = &'a K;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Paid(cursor) => cursor.next(),
            #[cfg(test)]
            Self::Legacy(cursor) => cursor.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Paid(cursor) => cursor.size_hint(),
            #[cfg(test)]
            Self::Legacy(cursor) => cursor.size_hint(),
        }
    }
}
impl<K> DoubleEndedIterator for Iter<'_, K> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Paid(cursor) => cursor.next_back(),
            #[cfg(test)]
            Self::Legacy(cursor) => cursor.next_back(),
        }
    }
}
impl<K> ExactSizeIterator for Iter<'_, K> {}
impl<K> std::iter::FusedIterator for Iter<'_, K> {}
