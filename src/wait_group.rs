use std::{
    ops::{AddAssign, SubAssign, Deref},
    sync::atomic::{AtomicU32, Ordering},
};

use omango_util::{
    hint::{likely},
    lock::Spinlock,
};

pub struct WaitGroup {
    guard: Spinlock<i32>,
    flag: AtomicU32,
}

impl Default for WaitGroup {
    #[inline(always)]
    fn default() -> Self {
        Self::new(0)
    }
}

impl WaitGroup {
    #[inline(always)]
    pub fn new(n: u32) -> Self {
        Self {
            guard: Spinlock::new(n as i32),
            flag: AtomicU32::new(0),
        }
    }
    
    #[inline(always)]
    pub fn add(&self, n: u32) {
        let mut count = self.guard.lock();
        count.add_assign(n as i32);
    }
    
    #[inline(always)]
    pub fn done(&self) {
        let mut count = self.guard.lock();
        let limit = 1i32;
        assert!(count.deref() >= &limit);

        count.sub_assign(1);
        if likely(count.deref() >= &limit) {
            return;
        }
        self.flag.store(1, Ordering::Release);
        omango_futex::wake_one(&self.flag);      
    }
    
    pub fn wait(&self) {
        while self.should_wait() {
            omango_futex::wait(&self.flag, 0);
        }
        self.flag.store(0, Ordering::Release);
    }
    
    #[inline(always)]
    fn should_wait(&self) -> bool {
        let count = self.guard.lock();
        count.deref() > &0i32
    }
}

mod test {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicU32, Ordering},
        },
        {panic, thread},
    };
    
    use crate::wait_group::WaitGroup;

    #[test]
    fn test_success() {
        let wg = Arc::new(WaitGroup::new(1));
        let wg_clone = wg.clone();
        
        let count = Arc::new(AtomicU32::new(0));
        let count_clone1 = count.clone();

        let thread = thread::spawn(move || {
            wg_clone.add(1);
            count_clone1.fetch_add(1, Ordering::Relaxed);
            wg_clone.done();
        });
        thread.join().unwrap();
        
        count.fetch_add(1, Ordering::Relaxed);
        wg.done();
        wg.wait();
        
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }
    
    #[test]
    fn test_done_without_size() {
        let result = panic::catch_unwind(|| {
            let wg = WaitGroup::default();
            wg.done();
        });
        assert!(result.is_err());
    }
}