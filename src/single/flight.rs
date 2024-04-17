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
    panic,
    rc::Rc,
    any::Any,
    cell::UnsafeCell,
    mem::MaybeUninit,
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use omango_util::{
    lock::Spinlock,
};

use crate::{
    error::Error,
    wg::WaitGroup,
};

type Fn<T> = fn() -> Result<T, Error>;

macro_rules! get_result {
    ($call:expr) => {
        unsafe { (*$call.result.get()).assume_init_ref().clone() }
    };
}
macro_rules! set_result {
    ($call:expr, $result:expr) => {
        unsafe { $call.result.get().write(MaybeUninit::new(Arc::new($result))) }
    };
}

struct Call<T: Any> {
    wg: WaitGroup,
    count: AtomicU32,

    // These fields are written once before the WaitGroup is done
    // and are only read after the WaitGroup is done.
    result: UnsafeCell<MaybeUninit<Arc<Result<T, Error>>>>,
}

impl<T: Any> Default for Call<T> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            wg: WaitGroup::default(),
            count: AtomicU32::new(0),
            result: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

/// [Group] represents a struct of work and forms a namespace in
/// which units of work can be executed with duplicate suppression.
pub struct Group {
    guard: Spinlock<HashMap<String, Rc<dyn Any>>>,
}

impl Default for Group {
    #[inline(always)]
    fn default() -> Self {
        Self {
            guard: Spinlock::new(HashMap::default()),
        }
    }
}

impl Group {
    /// [exec] executes and returns the results of the given function, making
    /// sure that only one execution is in-flight for a given key at a
    /// time. If a duplicate comes in, the duplicate caller waits for the
    /// original to complete and receives the same results.
    /// The return value shared indicates whether v was given to multiple callers.
    pub fn exec<T: Any>(&self, key: String, func: Fn<T>) -> (Arc<Result<T, Error>>, bool) {
        match self.get(key.as_str()) {
            Some(any) => {
                let call = any.downcast_ref::<Call<T>>().unwrap();
                call.count.fetch_add(1, Ordering::Relaxed);
                call.wg.wait();

                (get_result!(call), true)
            }
            None => {
                let oc = Rc::<Call<T>>::default();
                let call = oc.clone();
                oc.wg.add(1);
                self.guard.lock().insert(key, oc);

                let result = panic::catch_unwind(|| {
                    func()
                });
                let out = match result {
                    Ok(result) => {
                        set_result!(call, result);
                        (get_result!(call), call.count.load(Ordering::Relaxed) > 0)
                    }
                    Err(_) => {
                        set_result!(call, Err(Error("function of user panic".to_string())));
                        (get_result!(call), false)
                    }
                };
                call.wg.done();
                out
            }
        }
    }

    /// [forgot] tells the single-flight to forget about a key.  Future calls
    /// to [exec] for this key will call the function rather than waiting for
    /// an earlier call to complete.
    /// 
    /// NOTE: If [forgot] can not call, the future calls will get result of the
    /// last calling.
    #[inline(always)]
    pub fn forgot(&self, key: &str) -> bool {
        self.guard.lock().remove(key).is_some()
    }

    /// [size] returns number of waiting threads by the key. If key does not 
    /// exist, result will be zero.
    #[inline(always)]
    pub fn size<T: Any>(&self, key: &str) -> u32 {
        match self.guard.lock().get(key) {
            Some(any) => {
                let call = any.downcast_ref::<Call<T>>().unwrap();
                call.count.load(Ordering::Relaxed)
            },
            None => 0,
        }
    }

    #[allow(clippy::map_clone)]
    #[inline(always)]
    fn get(&self, key: &str) -> Option<Rc<dyn Any>> {
        self.guard.lock().get(key).map(|v| v.clone())
    }
}

unsafe impl Send for Group {}
unsafe impl Sync for Group {}

mod test {
    #[test]
    fn test() {
        let g1 = std::sync::Arc::new(crate::single::flight::Group::default());
        let g2 = g1.clone();

        let thread = std::thread::spawn(move || {
            let (rs, _) = g1.exec("google".to_string(), move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                Ok(1i32)
            });
            match rs.as_ref() {
                Ok(v) => assert_eq!(v, &1i32),
                Err(_) => panic!("should be success"),
            }
        });

        let (rs, is_dup) = g2.exec("google".to_string(), move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            Ok(1i32)
        });
        thread.join().unwrap();

        match rs.as_ref() {
            Ok(v) => assert_eq!(v, &1i32),
            Err(_) => panic!("should be success"),
        }
        assert!(is_dup);

        g2.forgot("google");
    }
}