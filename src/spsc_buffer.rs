#![feature(phase)]
#[phase(plugin)] extern crate assertions;

use std::cell::UnsafeCell;
use std::cmp::{max, min};
use std::num::UnsignedInt;
use std::sync::Arc;

/// producer end of the buffer.
/// write()s bytes into the buffer
pub struct Producer {
    inner : Arc<CircularBuffer>
}
/// consumer end of the buffer.
/// read()s bytes from the buffer.
pub struct Consumer {
    inner : Arc<CircularBuffer>
}

/// circular buffer that can be safely shared between at most one producer and
/// one consumer.
pub struct CircularBuffer {
    capacity : uint,
    interior : UnsafeCell<(uint, uint, uint, Vec<u8>)>,
    meta : UnsafeCell<uint>
}

impl CircularBuffer {
    fn makebuf(size : uint, calib : uint) -> CircularBuffer {
        let capacity : uint = UnsignedInt::next_power_of_two(size);
        CircularBuffer {
            capacity : capacity,
            interior : UnsafeCell::new((calib, calib, 0, Vec::from_elem(capacity, 0))),
            meta : UnsafeCell::new(calib)
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

pub trait CircularBufferUser {
    fn buffer<'a>(&'a self) -> &'a CircularBuffer;
    fn next(&self) -> uint;

    fn size(&self) -> uint {
        unsafe {
            match *(self.buffer().interior.get()) {
                (rp, wp, cp, _) => if !self.closed() || wp > rp {
                    assert!(cp == 0 || wp == cp);
                    wp - rp
                }
                else {
                    0
                }
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.size() == 0
    }

    fn is_full(&self) -> bool {
        self.available_capacity() == 0
    }

    fn closed(&self) -> bool {
        unsafe {
            match *(self.buffer().interior.get()) {
                (_, _, cp, _) => cp != 0
            }
        }
    }

    fn max_capacity(&self) -> uint {
        self.buffer().capacity
    }

    fn available_capacity(&self) -> uint {
        if self.closed() {
            0 /* since no more characters can be written */
        }
        else {
            self.max_capacity() - self.size()
        }
    }
}

/// Producers are cloneable, and can be safely shared provided the write()s
/// of no two threads ever overlap.  Any other methods may overlap
impl Clone for Producer {
    fn clone(&self) -> Producer {
        Producer { inner : self.inner.clone() }
    }
}

impl CircularBufferUser for Producer {
    fn buffer<'a>(&'a self) -> &'a CircularBuffer {
        &*self.inner
    }
    fn next(&self) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (_, wp, _, _) => wp
            }
        }
    }
}

impl Producer {
    pub fn close(&self) {
        unsafe {
            match *(self.inner.interior.get()) {
                (_, wp, ref mut cp, _) => if *cp == 0 { *cp = wp; }
            }
        }
    }

    /// store bytes into the buffer and return the number of bytes written
    /// linearizable since there is only one producer, i.e. the thread calling
    /// this is the only thread that can close the buffer
    pub fn write(&self, buf : &[u8]) -> uint {
        if self.closed() {
            0
        }
        else {
            unsafe {
                match *(self.inner.interior.get()) {
                    (rp, ref mut wp, cp, ref mut cbuf) => {
                        let to_write : uint = min(self.available_capacity(), buf.len());
                        for i in range(0, to_write) {
                            cbuf[(*wp + i) % self.inner.capacity] = buf[i];
                        }

                        *wp += to_write;

                        assert_le!(rp, *wp);
                        assert_eq!(cp, 0);

                        to_write
                    }
                }
            }
        }
    }
}

/// Consumers are cloneable, and can be safely shared if at most one thread
/// is read()ing at a time.  This object is linearizable as long as that
/// constraint is not violated.
impl Clone for Consumer {
    fn clone(&self) -> Consumer {
        Consumer { inner : self.inner.clone() }
    }
}

impl CircularBufferUser for Consumer {
    fn buffer<'a>(&'a self) -> &'a CircularBuffer {
        &*self.inner
    }
    fn next(&self) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, _, _, _) => rp
            }
        }
    }
}

impl Consumer {
    pub fn next(&self) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, _, _, _) => rp
            }
        }
    }
    /// copies data out of the buffer but does not
    /// advance through the buffer.
    /// subsequent read()s or more copies at the same index
    /// will return the same bytes
    pub fn copy_data(&self, start : uint, buf : &mut [u8]) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (rp, wp, _, ref cbuf) => {
                    if start < wp - self.max_capacity() || start < rp {
                        0
                    }
                    else {
                        let to_read : uint = min(self.size(), buf.len());
                        for i in range(0, to_read) {
                            buf[i] = cbuf[(start + i) % self.inner.capacity];
                        }
                        *(self.inner.meta.get()) =
                            max(*(self.inner.meta.get()), start + to_read);

                        to_read
                    }
                }
            }
        }
    }

    /// return the index of the highest-read character in the buffer
    /// as indicated by copy_data() and read()
    pub fn highest_read(&self) -> uint {
        unsafe {
            *(self.inner.meta.get())
        }
    }

    /// advances the read index the indicated number of bytes
    pub fn advance(&self, count : uint) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (ref mut rp, wp, _, _) => {
                    let prev : uint = *rp;
                    *rp = min(prev + count, wp);
                    *rp - prev
                }
            }
        }
    }

    pub fn advance_to(&self, end : uint) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (ref mut rp, wp, _, _) => {
                    let prev : uint = *rp;
                    *rp = min(max(prev, end), wp);
                    *rp - prev
                }
            }
        }
    }

    pub fn read(&self, buf : &mut [u8]) -> uint {
        unsafe {
            match *(self.inner.interior.get()) {
                (ref mut rp, wp, cp, _) => {
                    assert!(cp == 0 || wp == cp);
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
