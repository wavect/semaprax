//! One actual core invocation retains one base owner and its first typed failure.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InvocationFailure<E> {
    Bytes(E),
    Work(WorkRefusal),
    ExtensionWork(WorkRefusal),
    Producer(ProducerFailure),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProducerFailure {
    Layout,
    Allocation,
    Capacity,
    Schema,
    Detached,
}
pub(crate) struct Invocation<E> {
    bundle: Option<BasePreludeBundle>,
    first: Option<InvocationFailure<E>>,
    receipt: Option<WorkReceipt>,
    vector: Option<VectorExtension>,
    extension_receipts: [Option<WorkReceipt>; 9],
    #[cfg(test)]
    extension_limit: Option<usize>,
    #[cfg(test)]
    work_limit: Option<usize>,
}
impl<E> Invocation<E> {
    pub(crate) fn bind_owner(&mut self, current: bool) {
        if !current && self.first.is_none() {
            self.first = Some(InvocationFailure::Producer(ProducerFailure::Detached));
        }
    }
    pub(crate) fn new() -> Self {
        Self {
            bundle: None,
            first: None,
            receipt: None,
            vector: None,
            extension_receipts: [None; 9],
            #[cfg(test)]
            extension_limit: None,
            #[cfg(test)]
            work_limit: None,
        }
    }
}
impl<E: Copy> Invocation<E> {
    pub(crate) fn ensure(
        &mut self,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<&BasePreludeBundle, InvocationFailure<E>> {
        if let Some(first) = self.first {
            return Err(first);
        }
        let result = (|| {
            reserve(0).map_err(InvocationFailure::Bytes)?;
            if self.bundle.is_none() {
                reserve(
                    std::mem::size_of::<Option<BasePreludeBundle>>()
                        + std::mem::size_of::<Result<BasePreludeBundle, Failure<FixedFailure<E>>>>(
                        )
                        + std::mem::size_of::<Result<&BasePreludeBundle, InvocationFailure<E>>>(),
                )
                .map_err(InvocationFailure::Bytes)?;
                let mut payer =
                    FixedSchemaPayer::acquire(reserve).map_err(InvocationFailure::Bytes)?;
                #[cfg(test)]
                if let Some(limit) = self.work_limit {
                    payer.restrict_for_test(limit);
                }
                let result = BasePreludeBundle::acquire(&mut payer);
                self.receipt = Some(payer.receipt());
                self.bundle = Some(result.map_err(|failure| match failure {
                    Failure::Payer(FixedFailure::Bytes(error)) => InvocationFailure::Bytes(error),
                    Failure::Payer(FixedFailure::Work(error)) => InvocationFailure::Work(error),
                    Failure::Layout => InvocationFailure::Producer(ProducerFailure::Layout),
                    Failure::Allocation => InvocationFailure::Producer(ProducerFailure::Allocation),
                    Failure::Capacity => InvocationFailure::Producer(ProducerFailure::Capacity),
                    Failure::Schema => InvocationFailure::Producer(ProducerFailure::Schema),
                })?);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.first = Some(error);
            return Err(error);
        }
        Ok(self
            .bundle
            .as_ref()
            .expect("success retains the complete owner"))
    }
    #[cfg(test)]
    pub(crate) fn restrict_work(&mut self, limit: usize) {
        assert!(self.bundle.is_none() && self.first.is_none());
        self.work_limit = Some(limit);
    }
    #[cfg(test)]
    pub(crate) fn receipt(&self) -> Option<WorkReceipt> {
        self.receipt
    }
}

impl<E: Copy> Invocation<E> {
    pub(crate) fn ensure_selected(
        &mut self,
        prelude: &[crate::ast::TypeDeclaration],
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(&BasePreludeBundle, Option<&VectorExtension>), InvocationFailure<E>> {
        if let Some(first) = self.first {
            return Err(first);
        }
        let source = crate::prelude::declarations();
        if !matches!(prelude.len(), 2 | 3 | 4 | 6 | 8 | 9 | 11)
            || !std::ptr::eq(prelude, &source[..prelude.len()])
        {
            let first = InvocationFailure::Producer(ProducerFailure::Schema);
            self.first = Some(first);
            return Err(first);
        }
        self.ensure(reserve)?;
        let result = (|| {
            reserve(
                std::mem::size_of::<
                    Result<(&BasePreludeBundle, Option<&VectorExtension>), InvocationFailure<E>>,
                >() + std::mem::size_of::<Option<&VectorExtension>>(),
            )
            .map_err(InvocationFailure::Bytes)?;
            if prelude.len() > 2 && self.vector.is_none() {
                reserve(
                    2 * std::mem::size_of::<VectorExtension>()
                        + std::mem::size_of::<Option<VectorExtension>>(),
                )
                .map_err(InvocationFailure::Bytes)?;
                self.vector = Some(VectorExtension::empty());
            }
            for slot in 0..prelude.len() - 2 {
                if self.vector.as_ref().expect("selected catalog").values[slot].is_some() {
                    continue;
                }
                reserve(std::mem::size_of::<Result<super::extension_work::ExtensionPayer<'_, E>, InvocationFailure<E>>>()
                    + std::mem::size_of::<Result<ResolvedTypeDeclaration, InvocationFailure<E>>>())
                    .map_err(InvocationFailure::Bytes)?;
                let mut payer = super::extension_work::ExtensionPayer::acquire_for(slot, reserve)
                    .map_err(InvocationFailure::Bytes)?;
                #[cfg(test)]
                if let Some(limit) = self.extension_limit {
                    payer.restrict_for_test(limit);
                }
                let produced = super::extension_producer::acquire(
                    slot,
                    &prelude[slot + 2],
                    &self.vector.as_ref().expect("selected catalog").values,
                    &mut payer,
                );
                self.extension_receipts[slot] = Some(payer.receipt());
                let produced = produced.map_err(|failure| match failure {
                    Failure::Payer(FixedFailure::Bytes(error)) => InvocationFailure::Bytes(error),
                    Failure::Payer(FixedFailure::Work(error)) => {
                        InvocationFailure::ExtensionWork(error)
                    }
                    Failure::Layout => InvocationFailure::Producer(ProducerFailure::Layout),
                    Failure::Allocation => InvocationFailure::Producer(ProducerFailure::Allocation),
                    Failure::Capacity => InvocationFailure::Producer(ProducerFailure::Capacity),
                    Failure::Schema => InvocationFailure::Producer(ProducerFailure::Schema),
                })?;
                self.vector.as_mut().expect("selected catalog").values[slot] = Some(produced);
            }
            Ok(())
        })();
        if let Err(first) = result {
            self.first = Some(first);
            return Err(first);
        }
        Ok((
            self.bundle.as_ref().expect("successful base acquisition"),
            (prelude.len() > 2)
                .then_some(self.vector.as_ref())
                .flatten(),
        ))
    }
    #[cfg(test)]
    pub(crate) fn extension_receipt(&self) -> Option<WorkReceipt> {
        self.extension_receipts[0]
    }
    #[cfg(test)]
    pub(crate) fn extension_receipts(&self) -> &[Option<WorkReceipt>; 9] {
        &self.extension_receipts
    }
    #[cfg(test)]
    pub(crate) fn restrict_extension_work(&mut self, limit: usize) {
        assert!(limit <= super::extension_work::VECTOR_WORK);
        assert!(self.vector.is_none() && self.first.is_none());
        self.extension_limit = Some(limit);
    }
}
