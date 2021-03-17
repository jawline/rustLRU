#![feature(trait_alias)]

use std::hash::Hash;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// This trait alias is the minimum required bounds for an LRUKey
pub trait LRUKey = Eq + Hash + Clone + Copy;

/// This trait alias is the minimum required bounds for an LRUValue
pub trait LRUValue = Clone + Copy;

/// We are using linked lists in Rust, a data structure that doesn't suit it. We use Rc<RefCell<...>> to be able to borrow and mutate the reference in the list
type LRUItemRc<R, T> = Arc<Mutex<Box<LRUItem<R, T>>>>;

/// Each link in the list can be either Some(Reference to LRUItem) or None.
type LRUItemRef<R, T> = Option<LRUItemRc<R, T>>;

/// And LRUItem is an entity on the linked list that the lookup table can also hold a reference to
struct LRUItem<R: LRUKey, T: LRUValue> {
  key: R,
  data: T,
  next: LRUItemRef<R, T>,
  prev: LRUItemRef<R, T>,
}

impl<R: LRUKey, T: LRUValue> LRUItem<R, T> {

  /// Construct a new LRUItem
  pub fn new(key: R, item: T, next: LRUItemRef<R, T>, prev: LRUItemRef<R, T>) -> LRUItemRc<R, T> {
    Arc::new(Mutex::new(Box::new(LRUItem {
      key: key,
      data: item,
      next: next,
      prev: prev,
    })))
  }

  /// Remove the link to this LRU item from the chain
  pub fn remove(&mut self) {

    if let Some(prev) = &self.prev {
      prev.lock().unwrap().next = self.next.clone();
    }

    if let Some(next) = &self.next {
      next.lock().unwrap().prev = self.prev.clone();
    }

    self.next = None;
    self.prev = None;
  }
}

pub struct LRU<R: LRUKey, T: LRUValue> {
  head: LRUItemRef<R, T>,
  tail: LRUItemRef<R, T>,
  lookup_table: HashMap<R, LRUItemRc<R, T>>,
  limit: usize,
}

impl<R: LRUKey, T: LRUValue> LRU<R, T> {

  pub fn new(limit: usize) -> Self {
    LRU { head: None, tail: None, lookup_table: HashMap::new(), limit: limit }
  }

  fn insert_head(&mut self, new_head: LRUItemRc<R, T>) {

    // Insert this item at the head of the linked list
    match self.head.take() {
      Some(old) => {
        // Any existing item => head.prev = new_item && new_item.next = head && head = new_item
        new_head.lock().unwrap().next = Some(old.clone());
        old.lock().unwrap().prev = Some(new_head.clone());
        self.head = Some(new_head)
      },
      None => {
        // No existing item => head and tail are equal to the new item
        self.head = Some(new_head.clone());
        self.tail = Some(new_head);
      }
    }
  }

  pub fn add(&mut self, key: R, item: T) {
    let entry = self.lookup_table.get(&key).cloned();

    if let Some(entry) = entry {
      // It's already here - move it to the head
      entry.lock().unwrap().remove();
      self.insert_head(entry.clone());
    } else {

      // Construct the new element
      let new_head = LRUItem::new(key, item, None, None);

      // Insert this new reference as the head
      self.insert_head(new_head.clone());

      // Add a quick lookup to the hash table
      self.lookup_table.insert(key, new_head);
    }

    // We are too big, remove an entry
    if self.lookup_table.len() > self.limit {

      // Check there is a tail - this should be impossible
      if let Some(tail) = self.tail.take() {
        self.tail = tail.lock().unwrap().prev.clone();

        // If there is a new tail then update it's next pointer
        if let Some(new_tail) = &self.tail {
          new_tail.lock().unwrap().next = None;
        }

        self.lookup_table.remove(&tail.lock().unwrap().key);
      }

      // If we have removed the only item then set head = None, this only matters if limit=1
      if self.tail.is_none() {
        self.head = None;
      }
    }
  }

  pub fn fetch(&mut self, key: &R) -> Option<T> {

    // If we have the entry in the lookup table then return it and bump it to the head.
    // Otherwise return None
    if let Some(entry) = self.lookup_table.get(&key).cloned() {

      // move it to the head
      let return_value = entry.lock().unwrap().data.clone();
      entry.lock().unwrap().remove();
      self.insert_head(entry.clone());
      Some(return_value)
    } else {
      None
    }
  }
}

#[cfg(test)]
mod lru_tests {
  use super::*;

  fn count_forward<R: LRUKey, T: LRUValue>(lru: &LRU<R, T>) -> usize {
    let mut num = 0;
    let mut it = lru.head.clone();
    while let Some(entry) = it.clone() {
      it = entry.lock().unwrap().next.clone();
      num += 1;
    }
    num
  }

  fn count_backward<R: LRUKey, T: LRUValue>(lru: &LRU<R, T>) -> usize {
    let mut num = 0;
    let mut it = lru.tail.clone();
    while let Some(entry) = it.clone() {
      it = entry.lock().unwrap().prev.clone();
      num += 1;
    }
    num
  }

  fn expect_len<R: LRUKey, T: LRUValue>(lru: &LRU<R, T>, len: usize) {
    let head_len = count_forward(&lru);
    let tail_len = count_backward(&lru);
    let table_len = lru.lookup_table.len();
    assert_eq!(table_len, len);
    println!("{} {} {}", head_len, table_len, tail_len);
    assert!(head_len == table_len && head_len == tail_len);
  }

  #[test]
  fn test_simple() {
    let mut lru = LRU::new(5);
    lru.add(4, 65);
    lru.add(9, 23123);
    lru.add(12, 52312);
    expect_len(&lru, 3);
  }

  #[test]
  fn test_simple_evicted() {
    let mut lru = LRU::new(5);

    for i in 0..100 {
      lru.add(i, i * 10);
    }

    expect_len(&lru, 5);
  }
}
