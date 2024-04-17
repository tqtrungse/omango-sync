// Copyright (c) 2024 Trung Tran <tqtrungse@gmail.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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
        omango_futex::wake_all(&self.flag);      
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
    #[test]
    fn test_wait_on_one() {
        let wg = std::sync::Arc::new(crate::wg::WaitGroup::new(1));
        let wg_clone = wg.clone();
        
        let count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let count_clone = count.clone();

        let thread = std::thread::spawn(move || {
            wg_clone.add(1);
            count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            wg_clone.done();
        });
        
        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        wg.done();
        wg.wait();
        thread.join().unwrap();
        
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 2);
    }

    #[test]
    fn test_wait_on_gt_one() {
        let wg = std::sync::Arc::new(crate::wg::WaitGroup::new(1));
        let wg_clone = wg.clone();

        let count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let count_clone = count.clone();

        let thread = std::thread::spawn(move || {
            wg_clone.add(1);
            count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            wg_clone.done();
            wg_clone.wait();

            assert_eq!(count_clone.load(std::sync::atomic::Ordering::Relaxed), 2);
        });

        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        wg.done();
        wg.wait();
        
        thread.join().unwrap();
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 2);
    }
    
    #[test]
    fn test_done_without_size() {
        let result = std::panic::catch_unwind(|| {
            let wg = crate::wg::WaitGroup::default();
            wg.done();
        });
        assert!(result.is_err());
    }
}