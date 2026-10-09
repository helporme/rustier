//! Executes effects with a database-local, per-type least-recently-used result
//! cache.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use crate::compiler::db::Db;

/// A side effect that can be cached and reused across database operations.
pub trait Effect: Any + Clone + Hash + Eq + Send + Sync {
    /// Supplies execution-only inputs that may borrow data for the duration of
    /// a call.
    type Args<'a>;
    /// Represents a successful result that can be cloned into and out of the
    /// cache.
    type Res: Clone + Send + Sync + 'static;
    /// Represents failures from execution or synchronization checks.
    type Err;

    /// Limits cached entries for this effect type.
    const CACHE_CAPACITY: usize = 128;

    /// Performs the operation without consulting or updating the effect cache.
    fn _exec<'a>(&self, db: &mut dyn Db, args: Self::Args<'a>) -> Result<Self::Res, Self::Err>;

    /// Checks whether a cached result is still usable, defaulting to accepting
    /// it unchanged.
    fn _synced(&self, _db: &dyn Db, _res: &Self::Res) -> Result<bool, Self::Err> {
        Ok(true)
    }

    /// Runs an effect or returns a cached result if available.
    fn run<'a>(&self, db: &mut dyn Db, args: Self::Args<'a>) -> Result<Self::Res, Self::Err> {
        Effects::get_or_create(db).run(db, self, args)
    }

    /// The cached result for this effect if any.
    fn latest(&self, db: &dyn Db) -> Result<Option<Self::Res>, Self::Err> {
        Effects::get_or_create(db).latest(db, self)
    }
}

trait EffectKey: Send + Sync {
    fn dyn_eq(&self, other: &dyn EffectKey) -> bool;
    fn dyn_hash(&self, state: &mut dyn Hasher);
    fn key_type_id(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
}

impl<T> EffectKey for T
where
    T: Any + Hash + Eq + Send + Sync,
{
    fn dyn_eq(&self, other: &dyn EffectKey) -> bool {
        other
            .as_any()
            .downcast_ref::<T>()
            .is_some_and(|other| self == other)
    }

    fn dyn_hash(&self, state: &mut dyn Hasher) {
        let mut state = DynHasher(state);
        TypeId::of::<T>().hash(&mut state);
        self.hash(&mut state);
    }

    fn key_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn EffectKey {
    fn eq(&self, other: &Self) -> bool {
        self.dyn_eq(other)
    }
}

impl Eq for dyn EffectKey {}

impl Hash for dyn EffectKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.dyn_hash(state);
    }
}

struct DynHasher<'a>(&'a mut dyn Hasher);

impl Hasher for DynHasher<'_> {
    fn finish(&self) -> u64 {
        self.0.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }
}

struct CachedEffect {
    value: Box<dyn Any + Send + Sync>,
    last_used: u64,
}

#[derive(Default)]
struct EffectCache {
    store: HashMap<Box<dyn EffectKey>, CachedEffect>,
    clock: u64,
}

#[salsa::input(singleton)]
struct Effects {
    #[returns(clone)]
    cache: Arc<Mutex<EffectCache>>,
}

impl Effects {
    pub fn get_or_create(db: &dyn Db) -> Self {
        Self::try_get(db)
            .unwrap_or_else(|| Self::new(db, Arc::new(Mutex::new(EffectCache::default()))))
    }

    pub fn run<'a, E>(
        self,
        db: &mut dyn Db,
        effect: &E,
        args: E::Args<'a>,
    ) -> Result<E::Res, E::Err>
    where
        E: Effect,
    {
        if E::CACHE_CAPACITY == 0 {
            return effect._exec(db, args);
        }

        if let Some(value) = self.latest(db, effect)? {
            return Ok(value);
        }

        let value = effect._exec(db, args)?;

        let cache = self.cache(db);
        let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());

        cache.clock += 1;
        let last_used = cache.clock;

        if let Some(cached) = cache.store.get_mut(effect as &dyn EffectKey) {
            cached.value = Box::new(value.clone());
            cached.last_used = last_used;
            return Ok(value);
        }

        let type_id = TypeId::of::<E>();
        let mut count = 0;
        let mut oldest = None;

        for (key, cached) in &cache.store {
            if key.as_ref().key_type_id() != type_id {
                continue;
            }

            count += 1;

            if oldest.is_none_or(|oldest| cached.last_used < oldest) {
                oldest = Some(cached.last_used);
            }
        }

        if count >= E::CACHE_CAPACITY
            && let Some(oldest) = oldest
        {
            let mut removed = false;

            cache.store.retain(|key, cached| {
                if !removed && key.as_ref().key_type_id() == type_id && cached.last_used == oldest {
                    removed = true;
                    false
                } else {
                    true
                }
            });
        }

        cache.store.insert(
            Box::new(effect.clone()),
            CachedEffect {
                value: Box::new(value.clone()),
                last_used,
            },
        );

        Ok(value)
    }

    pub fn latest<E>(self, db: &dyn Db, effect: &E) -> Result<Option<E::Res>, E::Err>
    where
        E: Effect,
    {
        if E::CACHE_CAPACITY == 0 {
            return Ok(None);
        }

        let latest = {
            let cache = self.cache(db);
            let cache = cache.lock().unwrap_or_else(|error| error.into_inner());
            cache
                .store
                .get(effect as &dyn EffectKey)
                .and_then(|cached| cached.value.downcast_ref::<E::Res>())
                .cloned()
        };

        let Some(value) = latest else {
            return Ok(None);
        };

        if !effect._synced(db, &value)? {
            let cache = self.cache(db);
            let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());

            cache.store.remove(effect as &dyn EffectKey);

            return Ok(None);
        }

        let cache = self.cache(db);
        let mut cache = cache.lock().unwrap_or_else(|error| error.into_inner());

        cache.clock += 1;
        let last_used = cache.clock;

        if let Some(cached) = cache.store.get_mut(effect as &dyn EffectKey) {
            cached.last_used = last_used;
        }

        Ok(Some(value))
    }
}
