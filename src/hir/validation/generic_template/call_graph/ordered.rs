//! Private paid insertion-only AVL ordinal set. Stable indices survive rotations.
//! The sentinel cannot be an index: `Layout::array<Node>` limits every real Vec.
use super::{corrupt, layout_bytes, physical, reserve, settle, Diagnostic};
const NONE: usize = usize::MAX;
struct Node {
    key: usize,
    left: usize,
    right: usize,
    height: usize,
}
pub(super) struct Outgoing {
    root: usize,
    nodes: Vec<Node>,
}
impl Outgoing {
    pub(super) fn new() -> Self {
        Self {
            root: NONE,
            nodes: Vec::new(),
        }
    }
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.nodes.capacity()
    }
    fn height(&self, node: usize) -> usize {
        if node == NONE {
            0
        } else {
            self.nodes[node].height
        }
    }
    fn update(&mut self, node: usize) {
        self.nodes[node].height = 1 + self
            .height(self.nodes[node].left)
            .max(self.height(self.nodes[node].right));
    }
    fn right(&mut self, root: usize) -> usize {
        let next = self.nodes[root].left;
        self.nodes[root].left = self.nodes[next].right;
        self.nodes[next].right = root;
        self.update(root);
        self.update(next);
        next
    }
    fn left(&mut self, root: usize) -> usize {
        let next = self.nodes[root].right;
        self.nodes[root].right = self.nodes[next].left;
        self.nodes[next].left = root;
        self.update(root);
        self.update(next);
        next
    }
    fn balance(&mut self, root: usize) -> usize {
        self.update(root);
        let left = self.nodes[root].left;
        let right = self.nodes[root].right;
        if self.height(left) > self.height(right) + 1 {
            if self.height(self.nodes[left].right) > self.height(self.nodes[left].left) {
                self.nodes[root].left = self.left(left);
            }
            return self.right(root);
        }
        if self.height(right) > self.height(left) + 1 {
            if self.height(self.nodes[right].left) > self.height(self.nodes[right].right) {
                self.nodes[root].right = self.right(right);
            }
            return self.left(root);
        }
        root
    }
    fn insert_at(
        &mut self,
        root: usize,
        key: usize,
        inserted: &mut bool,
    ) -> Result<usize, Diagnostic> {
        reserve(0)?;
        if root == NONE {
            if self.nodes.len() == self.nodes.capacity() {
                let capacity = self
                    .nodes
                    .capacity()
                    .checked_mul(2)
                    .ok_or_else(physical)?
                    .max(1);
                reserve(layout_bytes::<Node>(capacity)?)?;
                self.nodes
                    .try_reserve_exact(capacity - self.nodes.len())
                    .map_err(|_| physical())?;
                settle(&self.nodes, capacity)?;
            }
            let index = self.nodes.len();
            if index == NONE {
                return Err(corrupt());
            }
            self.nodes.push(Node {
                key,
                left: NONE,
                right: NONE,
                height: 1,
            });
            *inserted = true;
            return Ok(index);
        }
        match key.cmp(&self.nodes[root].key) {
            std::cmp::Ordering::Equal => return Ok(root),
            std::cmp::Ordering::Less => {
                let child = self.insert_at(self.nodes[root].left, key, inserted)?;
                self.nodes[root].left = child;
            }
            std::cmp::Ordering::Greater => {
                let child = self.insert_at(self.nodes[root].right, key, inserted)?;
                self.nodes[root].right = child;
            }
        }
        Ok(self.balance(root))
    }
    pub(super) fn insert(&mut self, key: usize) -> Result<bool, Diagnostic> {
        reserve(0)?;
        let mut inserted = false;
        self.root = self.insert_at(self.root, key, &mut inserted)?;
        Ok(inserted)
    }
    fn visit_at(
        &self,
        root: usize,
        visit: &mut impl FnMut(usize) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        reserve(0)?;
        if root == NONE {
            return Ok(());
        }
        let node = &self.nodes[root];
        self.visit_at(node.left, visit)?;
        visit(node.key)?;
        self.visit_at(node.right, visit)
    }
    pub(super) fn visit(
        &self,
        visit: &mut impl FnMut(usize) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        self.visit_at(self.root, visit)
    }
}
