//! Implements shared and thread-/core-local singletons

use crate::runtime;
use core::cell::UnsafeCell;

/// A globally shared, lazy singleton
pub struct SharedSingleton<I, T> {
    /// The singleton value
    inner: UnsafeCell<(Option<I>, Option<T>)>,
}
impl<I, T> SharedSingleton<I, T>
where
    I: FnOnce() -> T,
{
    /// Creates a new lazy singleton with the given initializer
    pub const fn new(init: I) -> Self {
        let value = (Some(init), None);
        Self { inner: UnsafeCell::new(value) }
    }

    /// Provides scoped access to the underlying value
    pub fn scope<F, FR>(&self, scope: F) -> FR
    where
        F: FnOnce(&mut T) -> FR,
    {
        // Create mutable slots to transfer state to/from the closure
        let mut scope = Some(scope);
        let mut result: Option<FR> = None;

        // Create the caller
        let mut call_scope = || {
            // Get the inner state
            let inner_ptr = self.inner.get();
            let (init, value) = unsafe { inner_ptr.as_mut() }.expect("unexpected NULL pointer inside cell");

            // Initialize the value if necessary
            if let Some(init) = init.take() {
                let value_ = init();
                *value = Some(value_);
            }

            // Take the initialized value
            let Some(value) = value.as_mut() else {
                unreachable!("initialized singleton is not ready");
            };

            // Call the scope
            let scope = scope.take().expect("missing scope function");
            result = Some(scope(value));
        };

        // Run the implementation in a threadsafe context and return the result
        unsafe { runtime::_runtime_threadsafe_e0LtH0x3(&mut call_scope) };
        result.expect("implementation scope did not set result value")
    }
}
unsafe impl<I, T> Sync for SharedSingleton<I, T>
where
    I: Send,
    T: Send,
{
    // Marker trait, no members to implement
}

/// A fast, thread local singleton
///
/// # Warning
/// This singleton must not be accessed from interrupts; doing so will raise a panic. For interrupt-safe singletons, use
/// [`SharedSingleton`].
pub struct LocalSingleton<I, T, const THREADS_MAX: usize> {
    /// The initializer
    init: I,
    /// The per-thread values
    cells: [UnsafeCell<Option<T>>; THREADS_MAX],
}
impl<I, T, const THREADS_MAX: usize> LocalSingleton<I, T, THREADS_MAX>
where
    I: Fn() -> T + Copy,
{
    /// The default value for non-copy const-time initialization
    const INIT: UnsafeCell<Option<T>> = UnsafeCell::new(None);

    /// Creates a new thread local singleton
    pub const fn new(init: I) -> Self {
        Self { init, cells: [Self::INIT; THREADS_MAX] }
    }

    /// Provides scoped access to the underlying value
    ///
    /// # Panic
    /// This function will panic if called from an interrupt context
    pub fn scope<F, FR>(&self, scope: F) -> FR
    where
        F: FnOnce(&mut T) -> FR,
    {
        // Ensure that we are not in an interrupt handler
        let is_interrupted = unsafe { runtime::_runtime_isinterrupted_v5tnnoC7() };
        assert!(!is_interrupted, "local singleton must not be called from an interrupt handler");

        // Lookup our slot
        let thread_id = unsafe { runtime::_runtime_threadid_ZhZIZBv3() };
        assert!(thread_id < THREADS_MAX, "invalid thread ID");

        // Get the inner state
        let inner_ptr = self.cells[thread_id].get();
        let value = unsafe { inner_ptr.as_mut() }.expect("unexpected NULL pointer inside cell");

        // Call the scope
        let value = value.get_or_insert((self.init)());
        scope(value)
    }
}
unsafe impl<I, T, const THREADS_MAX: usize> Sync for LocalSingleton<I, T, THREADS_MAX>
where
    I: Send,
    T: Send,
{
    // Marker trait, no members to implement
}
