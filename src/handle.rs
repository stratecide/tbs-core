use std::fmt::Debug;

use uniform_smart_pointer::*;



pub struct Handle<T> {
    inner: Urc<UrwLock<T>>,
    attack_config_limit: Urc<Umutex<Option<usize>>>,
    unit_config_limit: Urc<Umutex<Option<usize>>>,
    terrain_config_limit: Urc<Umutex<Option<usize>>>,
}

impl<T> Handle<T> {
    pub fn new(t: T) -> Self {
        Self {
            inner: Urc::new(UrwLock::new(t)),
            attack_config_limit: Urc::new(Umutex::new(None)),
            unit_config_limit: Urc::new(Umutex::new(None)),
            terrain_config_limit: Urc::new(Umutex::new(None)),
        }
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let t = self.inner.read();
        f(&*t)
    }

    pub fn with_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut t = self.inner.write();
        f(&mut *t)
    }

    pub fn borrow<'a, R>(&'a self, f: impl FnOnce(&T) -> &R) -> ReadGuard<'a, R> {
        self.inner.read().map(f)
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
        self.attack_config_limit.lock().clone()
    }
    pub(crate) fn set_attack_config_limit(&self, limit: Option<usize>) {
        *self.attack_config_limit.lock() = limit;
    }
    pub(crate) fn get_unit_config_limit(&self) -> Option<usize> {
        self.unit_config_limit.lock().clone()
    }
    pub(crate) fn set_unit_config_limit(&self, limit: Option<usize>) {
        *self.unit_config_limit.lock() = limit;
    }
    pub(crate) fn get_terrain_config_limit(&self) -> Option<usize> {
        self.terrain_config_limit.lock().clone()
    }
    pub(crate) fn set_terrain_config_limit(&self, limit: Option<usize>) {
        *self.terrain_config_limit.lock() = limit;
    }
}

impl<T: Debug> Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.with(|t| t.fmt(f))
    }
}

impl<T: Clone> Clone for Handle<T> {
    fn clone(&self) -> Self {
        let inner = Urc::new(UrwLock::new(self.with(|t| t.clone())));
        // TODO: should this panic if self.unit_config_limit or self.terrain_config_limit aren't None?
        Self {
            inner,
            attack_config_limit: Urc::new(Umutex::new(None)),
            unit_config_limit: Urc::new(Umutex::new(None)),
            terrain_config_limit: Urc::new(Umutex::new(None)),
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
