#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use anyhow::{Ok, Result};
use std::cmp::{self};
use std::collections::binary_heap::PeekMut;
use std::collections::BinaryHeap;

use crate::key::KeySlice;

use super::StorageIterator;

struct HeapWrapper<I: StorageIterator>(pub usize, pub Box<I>);

impl<I: StorageIterator> PartialEq for HeapWrapper<I> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == cmp::Ordering::Equal
    }
}

impl<I: StorageIterator> Eq for HeapWrapper<I> {}

impl<I: StorageIterator> PartialOrd for HeapWrapper<I> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<I: StorageIterator> Ord for HeapWrapper<I> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.1
            .key()
            .cmp(&other.1.key())
            .then(self.0.cmp(&other.0))
            .reverse()
    }
}

/// Merge multiple iterators of the same type. If the same key occurs multiple times in some
/// iterators, prefer the one with smaller index.
pub struct MergeIterator<I: StorageIterator> {
    iters: BinaryHeap<HeapWrapper<I>>,
    current: Option<HeapWrapper<I>>,
    // [NOTE BY CHENHAO]
    // Q: why we need to keep `current`? can't we just use the top element of the heap as the current?
}

impl<I: StorageIterator> MergeIterator<I> {
    pub fn create(iters: Vec<Box<I>>) -> Self {
        if iters.is_empty() {
            return Self {
                iters: BinaryHeap::new(),
                current: None,
            };
        }

        // handle if all are iterator are invalid
        if iters.iter().all(|x| !x.is_valid()) {
            // iters was an immutable variable but with ownership
            // move to a new one with ownership but mutable
            // let mut iters = iters;
            return Self {
                iters: BinaryHeap::new(),
                current: None,
                // current: Some(HeapWrapper(0, iters.pop().unwrap())),
            };
            // [NOTE BY CHENHAO]
            // Q: why we use the last one? why not just None?
        }
        // all elements are valid, so just push into the heap
        let mut heap = BinaryHeap::new();
        for (i, it) in iters.into_iter().enumerate() {
            if it.is_valid() {
                heap.push(HeapWrapper(i, it));
            }
        }

        let current = heap.pop().unwrap();
        Self {
            iters: heap,
            current: Some(current),
        }

        // [NOTE BY CHENHAO]
        // Consider the code below. Is that right? (probably the tricky piece
        // is using None if all are invalid)
        //
        // let mut heap = BinaryHeap::new();
        // for (i, it) in iters.into_iter().enumerate() {
        //     if it.is_valid() {
        //         heap.push(HeapWrapper(i, it));
        //     }
        // }
        // if let Some(current) = heap.pop() {
        //     Self {
        //         iters: heap,
        //         current: Some(current),
        //     }
        // } else {
        //     Self {
        //         iters: heap,
        //         current: None,
        //     }
        // }
    }
}

impl<I: 'static + for<'a> StorageIterator<KeyType<'a> = KeySlice<'a>>> StorageIterator
    for MergeIterator<I>
{
    type KeyType<'a> = KeySlice<'a>;

    fn key(&self) -> KeySlice {
        self.current.as_ref().unwrap().1.key()
    }

    fn value(&self) -> &[u8] {
        self.current.as_ref().unwrap().1.value()
    }

    fn is_valid(&self) -> bool {
        self.current
            .as_ref()
            .map(|x| x.1.is_valid())
            .unwrap_or(false)
    }

    fn next(&mut self) -> Result<()> {
        // [NOTE BY CHENHAO]
        // tricky: note there is shadowing among iterators!
        // if a key has appear in (X, iterX), then the same key in (Y, iterY)
        // with Y > X must be ignored

        let current = self.current.as_mut().unwrap();
        // advance the iterators with the same key
        while let Some(mut inner_iter) = self.iters.peek_mut() {
            debug_assert!(
                inner_iter.1.key() >= current.1.key(),
                "heap invariant violated"
            );
            if current.1.key() != inner_iter.1.key() {
                break;
            }

            if let Err(e) = inner_iter.1.next() {
                // such error is unexpected!
                std::collections::binary_heap::PeekMut::pop(inner_iter);
                return Err(e);
            }
            if !inner_iter.1.is_valid() {
                std::collections::binary_heap::PeekMut::pop(inner_iter);
            }
        }

        current.1.next()?;
        if !current.1.is_valid() {
            if let Some(iter) = self.iters.pop() {
                *current = iter;
            }
            return Ok(());
        }

        if let Some(mut inner_iter) = self.iters.peek_mut() {
            if *current < *inner_iter {
                std::mem::swap(&mut *inner_iter, current);
            }
        }

        Ok(())

        // let mut old_curr = self.current.take().unwrap();
        // old_curr.1.next()?;

        // if old_curr.1.is_valid() {
        //     self.iters.push(old_curr);
        // }

        // self.current = self.iters.pop();

        // Ok(())
    }
}
