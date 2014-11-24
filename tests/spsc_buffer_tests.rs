#![feature(phase)]
#[phase(plugin)] extern crate assertions;
extern crate spsc_buffer;

use std::cmp::min;
use std::rand::random;
use spsc_buffer::{CircularBuffer, Consumer, Producer};

#[test]
fn test_buf() {
    let calib : uint = ::std::rand::random();
    let (p, c) : (Producer, Consumer) =
        CircularBuffer::new_calibrated(65536, calib);

    assert!(c.is_empty());
    assert!(!p.is_full());
    assert_eq!(c.size(), 0);
    assert_eq!(p.available_capacity(), p.max_capacity());
    assert_eq!(c.next(), calib);
    assert_eq!(p.next(), calib);

    let mut rbuf : [u8, .. 1024] = [0, .. 1024];
    assert_eq!(c.read(rbuf.as_mut_slice()), 0);

    let mut wbuf : [u8, .. 1024] = [0, .. 1024];
    for _ in range(0u, 8u) {
        for i in range(0, 1024) {
            wbuf[i] = ::std::rand::random();
        }

        assert_eq!(p.write(wbuf.as_slice()), 1024);
        assert!(!c.is_empty());
        assert_eq!(c.size(), 1024);
        assert_eq!(p.available_capacity(), p.max_capacity() - 1024);

        assert_eq!(c.read(rbuf.as_mut_slice()), 1024);
        assert!(c.is_empty());
        assert_eq!(c.size(), 0);
        assert_eq!(p.available_capacity(), p.max_capacity());
        
        for i in range(0, 1024) {
            assert_eq!(wbuf[i], rbuf[i]);
        }
    }

    /* fill the buffer */
    let mut i : uint = 0;
    loop {
        assert_eq!(p.available_capacity(), p.max_capacity() - i * 1024);
        assert_eq!(c.size(), i * 1024);

        let expected_write : uint = min(1024, p.max_capacity() - i * 1024);
        assert!(!p.is_full() || expected_write == 0);
        assert_eq!(p.write(wbuf.as_slice()), expected_write);

        if expected_write < 1024 {
            break;
        }

        i += 1;
    }
    assert_gt!(i, 0);

    println!("buffer full yo");

    assert!(p.is_full());
    assert_eq!(c.size(), p.max_capacity());
    assert_eq!(p.available_capacity(), 0);
    assert_eq!(p.write(wbuf.as_slice()), 0);

    /* now read everything */
    for j in range(0, i) {
        assert!(!c.is_empty());
        println!("reading from dat full buffer yo");
        assert_eq!(c.read(rbuf.as_mut_slice()), 1024);
        println!("mm read from dat full buffer yo");

        assert_eq!(p.available_capacity(), (j + 1) * 1024);
        assert_eq!(c.size(), p.max_capacity() - (j + 1) * 1024);
    }
    println!("read most yo");
    assert_eq!(c.read(rbuf.as_mut_slice()), p.max_capacity() - i * 1024);
    assert!(c.is_empty());

    assert_eq!(p.write(wbuf.as_slice()), 1024);
    assert_eq!(c.size(), 1024);
    /* doesn't read beyond what is available */
    let mut rbuf2 : [u8, .. 2048] = [0, .. 2048];
    assert_eq!(c.read(rbuf2.as_mut_slice()), 1024);
    assert_eq!(c.read(rbuf2.as_mut_slice()), 0);
    for i in range(0, 1024) {
        assert_eq!(rbuf2[i], wbuf[i]);
    }
}

#[test]
fn test_copy_advance() {
    let calibration : uint = ::std::rand::random();
    let (p, c) : (Producer, Consumer) = CircularBuffer::new_calibrated(65536, calibration);

    let mut write_buf : [u8, .. 1024] = [0, .. 1024];
    for i in range(0, 1024) {
        write_buf[i] = ::std::rand::random();
    }
    assert_eq!(p.write(&write_buf), 1024);

    let mut read_buf : [u8, .. 1024] = [0, .. 1024];
    assert_eq!(c.copy_data(calibration, read_buf.as_mut_slice()), 1024);
    assert_eq!(write_buf.as_slice(), read_buf.as_slice());
    /* try again - should not have advanced */
    assert_eq!(c.copy_data(calibration, read_buf.as_mut_slice()), 1024);
    assert_eq!(write_buf.as_slice(), read_buf.as_slice());

    assert_eq!(c.advance(512), 512);
    assert_eq!(c.copy_data(calibration + 512, read_buf.slice_to_mut(512)), 512);
    assert_eq!(write_buf.slice_from(512), read_buf.slice_to(512));

    assert_eq!(c.advance(1024), 512);
    assert_eq!(c.copy_data(calibration + 1024, &mut read_buf), 0);
}

