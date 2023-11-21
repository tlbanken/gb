//! Common utility functions and helpers.

use std::{
  cell::{Ref, RefCell, RefMut},
  rc::Rc,
};

pub trait LazyDref<T> {
  fn lazy_dref(&self) -> Ref<T>;

  fn lazy_dref_mut(&self) -> RefMut<T>;
}

impl<T> LazyDref<T> for Option<Rc<RefCell<T>>> {
  fn lazy_dref(&self) -> Ref<T> {
    self.as_ref().unwrap().borrow()
  }

  fn lazy_dref_mut(&self) -> RefMut<T> {
    self.as_ref().unwrap().borrow_mut()
  }
}
