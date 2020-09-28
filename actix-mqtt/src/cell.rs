//! Custom cell impl
use std::cell::UnsafeCell;
use std::ops::Deref;
use std::rc::Rc;
use std::task::{Context, Poll};

use actix_service::Service;

pub(crate) struct Cell<T> {
    inner: Rc<UnsafeCell<T>>,
}

impl<T> Clone for Cell<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Deref for Cell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get_ref()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Cell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> Cell<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Rc::new(UnsafeCell::new(inner)),
        }
    }

    pub fn get_ref(&self) -> &T {
        unsafe { &*self.inner.as_ref().get() }
    }

    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.inner.as_ref().get() }
    }
}

impl<T: Service> Service for Cell<T> {
    type Request = T::Request;
    type Response = T::Response;
    type Error = T::Error;
    type Future = T::Future;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), T::Error>> {
        self.get_mut().poll_ready(cx)
    }

    fn call(&mut self, req: T::Request) -> T::Future {
        self.get_mut().call(req)
    }
}
