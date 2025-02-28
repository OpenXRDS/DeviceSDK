use std::marker::PhantomData;

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetId {
    Uuid(Uuid),
    Key(String),
}

#[derive(Debug, Clone)]
pub struct AssetStrongHandle<A>
where
    A: Clone,
{
    id: AssetId,
    asset: A,
}

#[derive(Debug, Clone)]
pub struct AssetHandle<A>
where
    A: Clone,
{
    id: AssetId,
    _p: PhantomData<A>,
}

impl<A> AssetStrongHandle<A>
where
    A: Clone,
{
    pub fn new(id: AssetId, asset: A) -> Self {
        Self { id, asset }
    }

    pub fn asset(&self) -> &A {
        &self.asset
    }

    pub fn as_weak_handle(&self) -> AssetHandle<A> {
        AssetHandle::new(&self.id)
    }
}

impl<A> AssetHandle<A>
where
    A: Clone,
{
    pub fn new(id: &AssetId) -> Self {
        Self {
            id: id.clone(),
            _p: PhantomData::default(),
        }
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }
}
