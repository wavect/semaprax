//! Exact borrowed node/definition index. Sorting touches private lookup rows,
//! never HIR, cleanup vectors, or runtime evaluation order.
use super::*;
#[derive(Clone, Copy)]
pub(super) struct Node<'a> {
    pub(super) expression: &'a ResolvedExpr,
    pub(super) start: usize,
    pub(super) end: usize,
}
#[derive(Clone, Copy)]
pub(super) struct Definition<'a> {
    pub(super) id: &'a ValueId,
    pub(super) value: Option<&'a ResolvedExpr>,
    pub(super) mutable: bool,
    pub(super) ty: &'a ResolvedType,
}
pub(super) struct Index<'a> {
    nodes: Vec<Node<'a>>,
    definitions: Vec<Definition<'a>>,
    pub(super) regions: usize,
}
impl<'a> Index<'a> {
    pub(super) fn build(
        function: &'a ResolvedFunction,
        storage: &Storage,
        walker: &mut Walker<'a>,
    ) -> Result<Self, Failure> {
        storage
            .reserve(std::mem::size_of::<Self>() + std::mem::size_of::<Result<Self, Failure>>())?;
        let (mut nodes, mut definitions, mut regions) = (0usize, function.params.len(), 0usize);
        walker.function(function, |event| {
            if let Event::Enter { expression, .. } = event {
                nodes = nodes.checked_add(1).ok_or_else(|| {
                    Failure::semantic("borrowed byte traversal census node count overflows")
                })?;
                if shape::shape(expression)?.is_some() {
                    regions = regions.checked_add(1).ok_or_else(|| {
                        Failure::semantic("borrowed byte traversal census region count overflows")
                    })?;
                }
                if let ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
                {
                    definitions = definitions
                        .checked_add(
                            statements
                                .iter()
                                .filter(|statement| {
                                    matches!(statement, ResolvedStatement::Let { .. })
                                })
                                .count(),
                        )
                        .ok_or_else(|| {
                            Failure::semantic(
                                "borrowed byte traversal census binding count overflows",
                            )
                        })?;
                }
            }
            Ok(())
        })?;
        if regions < 2 {
            return Err(Failure::semantic(
                "borrowed byte traversal is nested in another traversal",
            ));
        }
        let mut result = Self {
            nodes: storage.vector(nodes)?,
            definitions: storage.vector(definitions)?,
            regions,
        };
        for parameter in &function.params {
            storage.push(
                &mut result.definitions,
                Definition {
                    id: &parameter.id,
                    value: None,
                    mutable: false,
                    ty: &parameter.ty,
                },
            )?;
        }
        walker.function(function, |event| {
            match event {
                Event::Enter {
                    expression,
                    position,
                } => {
                    storage.push(
                        &mut result.nodes,
                        Node {
                            expression,
                            start: position,
                            end: 0,
                        },
                    )?;
                    if let ResolvedExprKind::Block { statements, .. }
                    | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
                    {
                        for statement in statements {
                            if let ResolvedStatement::Let {
                                binding,
                                mutable,
                                value,
                                ..
                            } = statement
                            {
                                storage.push(
                                    &mut result.definitions,
                                    Definition {
                                        id: &binding.id,
                                        value: Some(value),
                                        mutable: *mutable,
                                        ty: &binding.ty,
                                    },
                                )?;
                            }
                        }
                    }
                }
                Event::Exit { position, end, .. } => result.nodes[position].end = end,
            }
            Ok(())
        })?;
        result
            .nodes
            .sort_unstable_by(|a, b| a.expression.id.cmp(&b.expression.id));
        if result
            .nodes
            .windows(2)
            .any(|pair| pair[0].expression.id == pair[1].expression.id)
        {
            return Err(Failure::semantic(
                "borrowed byte traversal census has duplicate expression identity",
            ));
        }
        result.definitions.sort_unstable_by(|a, b| a.id.cmp(b.id));
        if result
            .definitions
            .windows(2)
            .any(|pair| pair[0].id == pair[1].id)
        {
            return Err(Failure::semantic(
                "borrowed byte traversal census has duplicate binding identity",
            ));
        }
        Ok(result)
    }
    pub(super) fn node(&self, expression: &ResolvedExpr) -> Result<Node<'a>, Failure> {
        let node = self
            .nodes
            .binary_search_by(|row| row.expression.id.cmp(&expression.id))
            .ok()
            .map(|index| self.nodes[index]);
        node.filter(|node| std::ptr::eq(node.expression, expression))
            .ok_or_else(|| {
                Failure::semantic(
                    "borrowed byte traversal census expression differs from current function",
                )
            })
    }
    pub(super) fn available(&self, view: &ResolvedExpr, before: usize) -> Result<(), Failure> {
        let place = bare_place(view)
            .ok_or_else(|| Failure::semantic("borrowed byte traversal view is not a bare place"))?;
        let definition = self
            .definitions
            .binary_search_by(|row| row.id.cmp(&place.root))
            .ok()
            .map(|index| self.definitions[index])
            .ok_or_else(|| {
                Failure::semantic(
                    "nested borrowed byte traversal view is not bound at outermost entry",
                )
            })?;
        if definition.mutable
            || *definition.ty != ResolvedType::SliceU8
            || definition
                .value
                .is_some_and(|value| self.node(value).map_or(true, |node| node.end > before))
        {
            return Err(Failure::semantic(
                "nested borrowed byte traversal view is not immutable at outermost entry",
            ));
        }
        Ok(())
    }
}
