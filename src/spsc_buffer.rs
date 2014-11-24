#![feature(phase)]
#[phase(plugin)] extern crate assertions;

use std::cell::UnsafeCell;
use std::cmp::min;
use std::num::UnsignedInt;
use std::sync::Arc;

pub struct Producer {
    inner : Arc<CircularBuffer>
}
pub struct Consumer {
    inner : Arc<CircularBuffer>
}

pub struct CircularBuffer {
    capacity : uint,
    interior : UnsafeCell<(uint, uint, Vec<u8>)>
}

impl CircularBuffer {
    fn makebuf(size : uint, calib : uint) -> CircularBuffer {
        let capacity : uint = UnsignedInt::next_power_of_two(size);
        CircularBuffer {
            capacity : capacity,
            interior : UnsafeCell::new((calib, calib, Vec::from_elem(capacity, 0)))
        }
    }
    pub fn new(size : uint) -> (Producer, Consumer) {
        let buf : Arc<CircularBuffer> = Arc::new(CircularBuffer::makebuf(size, 0));

        (Producer { inner : buf.clone() },
        Consumer { inner : buf })
    }
    
    pub fn new_calibrated(size : uint, start : uint) -> (Producer, Consumer) {
        let buf : Arc<CircularBuffer> = Arc::new(CircularBuffer::makebuf(size, start));

        (Producer { inner : buf.clone() },
        Consumer { inner : buf })
    }
}

impl Producer {
    pub fn is_full(&self) -> bool {
        self.available_capacity() == 0
    }

    pub fn next(&self) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, _, _) => rp
            }
        }
    }

    pub fn max_capacity(&self) -> uint {
        self.inner.capacity
    }

    pub fn available_capacity(&self) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, wp, _) => self.inner.capacity - (wp - rp)
            }
        }
    }

    pub fn write(&self, buf : &[u8]) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, ref mut wp, ref mut cbuf) => {
                    let to_write : uint = min(self.available_capacity(), buf.len());
                    for i in range(0, to_write) {
                        cbuf[(*wp + i) % self.inner.capacity] = buf[i];
                    }

                    *wp += to_write;

                    assert_le!(rp, *wp);

                    to_write
                }
            }
        }
    }
}

impl Consumer {
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }

    pub fn next(&self) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (_, wp, _) => wp
            }
        }
    }

    pub fn size(&self) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, wp, _) => wp - rp
            }
        }
    }

    pub fn copy_data(&self, start : uint, buf : &mut [u8]) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, wp, ref cbuf) => {
                    assert_ge!(start, wp - self.inner.capacity);
                    assert_ge!(start, rp);

                    let to_read : uint = min(self.size(), buf.len());
                    for i in range(0, to_read) {
                        buf[i] = cbuf[(start + i) % self.inner.capacity];
                    }

                    to_read
                }
            }
        }
    }

    pub fn advance(&self, count : uint) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (ref mut rp, wp, _) => {
                    let prev : uint = *rp;
                    *rp = min(prev + count, wp);
                    *rp - prev
                }
            }
        }
    }

    pub fn read(&self, buf : &mut [u8]) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (ref mut rp, wp, _) => {
                    let nread : uint = self.copy_data(*rp, buf);
                    *rp += nread;

                    assert_le!(*rp, wp);
                    assert_le!(wp, *rp + self.inner.capacity);

                    nread
                }
            }
        }
    }
}
