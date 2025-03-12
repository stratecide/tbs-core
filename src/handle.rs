use std::fmt::Debug;
use std::ops::Deref;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard};
use std::sync::MappedRwLockReadGuard;



pub struct Handle<T: ?Sized> {
    inner: Arc<RwLock<T>>,
    attack_config_limit: Arc<Mutex<Option<usize>>>,
    unit_config_limit: Arc<Mutex<Option<usize>>>,
    terrain_config_limit: Arc<Mutex<Option<usize>>>,
}

impl<T> Handle<T> {
    pub fn new(t: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(t)),
            attack_config_limit: Arc::new(Mutex::new(None)),
            unit_config_limit: Arc::new(Mutex::new(None)),
            terrain_config_limit: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let t = self.inner.read().expect("Unable to read Handle");
        f(&*t)
    }

    pub fn with_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut t = self.inner.write().expect("Unable to write Handle");
        f(&mut *t)
    }

    pub fn borrow<'a, R>(&'a self, f: impl FnOnce(&T) -> &R) -> BorrowedHandle<'a, R> {
        let guard = self.inner.read().expect("Unable to borrow Handle");
        BorrowedHandle::Guard(RwLockReadGuard::map(guard, f))
    }

    /**
     * DANGEROUS METHOD!
     * using this method creates the possibility for deadlocks if both clones want competing locks at the same time
     */
    pub(crate) fn cloned(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            attack_config_limit: self.terrain_config_limit.clone(),
            unit_config_limit: self.unit_config_limit.clone(),
            terrain_config_limit: self.terrain_config_limit.clone(),
        }
    }

    pub(crate) fn get_attack_config_limit(&self) -> Option<usize> {
        self.attack_config_limit.lock().unwrap().clone()
    }
    pub(crate) fn set_attack_config_limit(&self, limit: Option<usize>) {
        *self.attack_config_limit.lock().unwrap() = limit;
    }
    pub(crate) fn get_unit_config_limit(&self) -> Option<usize> {
        self.unit_config_limit.lock().unwrap().clone()
    }
    pub(crate) fn set_unit_config_limit(&self, limit: Option<usize>) {
        *self.unit_config_limit.lock().unwrap() = limit;
    }
    pub(crate) fn get_terrain_config_limit(&self) -> Option<usize> {
        self.terrain_config_limit.lock().unwrap().clone()
    }
    pub(crate) fn set_terrain_config_limit(&self, limit: Option<usize>) {
        *self.terrain_config_limit.lock().unwrap() = limit;
    }
}

impl<T: Debug> Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.with(|t| t.fmt(f))
    }
}

impl<T: Clone> Clone for Handle<T> {
    fn clone(&self) -> Self {
        let inner = Arc::new(RwLock::new(self.with(|t| t.clone())));
        // TODO: should this panic if self.unit_config_limit or self.terrain_config_limit aren't None?
        Self {
            inner,
            attack_config_limit: Arc::new(Mutex::new(None)),
            unit_config_limit: Arc::new(Mutex::new(None)),
            terrain_config_limit: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T: PartialEq> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.with(|t1| {
            other.with(|t2| {
                t1.eq(t2)
            })
        })
    }
}

pub enum BorrowedHandle<'a, T> {
    Guard(MappedRwLockReadGuard<'a, T>),
    Ref(&'a T),
}

impl<'a, T> Deref for BorrowedHandle<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Guard(t) => &**t,
            Self::Ref(t) => t
        }
    }
}
