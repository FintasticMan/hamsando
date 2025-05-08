//! Traits for implementing and using DSTs.
//!
//! The design is inspired by the great [slice-dst] crate, but with more of a
//! focus on implementability and use of modern Rust features.
//!
//! [slice-dst]: https://lib.rs/crates/slice-dst

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

mod errors;

#[cfg(feature = "alloc")]
use alloc::{
    alloc::{alloc, dealloc, handle_alloc_error},
    boxed::Box,
};
use core::{
    alloc::{Layout, LayoutError},
    convert::Infallible,
    error::Error,
    ptr,
};

pub use errors::*;

/// A dynamically sized type.
///
/// # Safety
///
/// Must be implemented as described.
// FUTURE: switch to metadata rather than length once the `ptr_metadata` feature
// has stabilised.
pub unsafe trait Dst {
    /// The length of the DST.
    ///
    /// This is NOT the size of the type, for that you should use [Self::layout].
    fn len(&self) -> usize;

    /// Returns the layout of the DST, assuming it has the given length.
    fn layout(len: usize) -> Result<Layout, LayoutError>;

    /// Convert the given thin pointer to a fat pointer to the DST, adding the
    /// length to the metadata.
    ///
    /// # Safety
    ///
    /// This function is safe but the returned pointer is not necessarily safe
    /// to dereference.
    fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self>;

    /// Writes the data contained in this DST to the pointer given.
    ///
    /// # Safety
    ///
    /// The given pointer must be valid for the DST and have the same length as
    /// `self`.
    unsafe fn clone_to_raw(&self, ptr: ptr::NonNull<Self>);
}

unsafe impl<T> Dst for [T] {
    fn len(&self) -> usize {
        self.len()
    }

    fn layout(len: usize) -> Result<Layout, LayoutError> {
        Layout::array::<T>(len)
    }

    fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self> {
        // FUTURE: switch to ptr::NonNull:from_raw_parts() when it has stabilised.
        let ptr = ptr::NonNull::slice_from_raw_parts(ptr.cast::<()>(), len);
        unsafe { ptr::NonNull::new_unchecked(ptr.as_ptr() as *mut _) }
    }

    unsafe fn clone_to_raw(&self, ptr: ptr::NonNull<Self>) {
        unsafe { ptr::copy_nonoverlapping(self.as_ptr(), ptr.as_ptr().cast(), self.len()) };
    }
}

unsafe impl Dst for str {
    fn len(&self) -> usize {
        self.len()
    }

    fn layout(len: usize) -> Result<Layout, LayoutError> {
        Layout::array::<u8>(len)
    }

    fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self> {
        // FUTURE: switch to ptr::NonNull:from_raw_parts() when it has stabilised.
        let ptr = ptr::NonNull::slice_from_raw_parts(ptr.cast::<()>(), len);
        unsafe { ptr::NonNull::new_unchecked(ptr.as_ptr() as *mut _) }
    }

    unsafe fn clone_to_raw(&self, ptr: ptr::NonNull<Self>) {
        unsafe { ptr::copy_nonoverlapping(self.as_ptr(), ptr.as_ptr().cast(), self.len()) };
    }
}

/// Type that can allocate a DST and store it inside it.
///
/// # Safety
///
/// Must be implemented as described.
// FUTURE: use the Allocator trait once it has stabilised.
pub unsafe trait AllocDst<T: ?Sized + Dst>: Sized {
    /// Allocate the DST with the given length, initialize the data with the
    /// given function, and store it in the type.
    ///
    /// # Safety
    ///
    /// The `init` function may not panic, otherwise there will be a memory leak.
    unsafe fn new_dst<F>(len: usize, init: F) -> Result<Self, AllocDstError>
    where
        F: FnOnce(ptr::NonNull<T>) -> ();
}

/// Blanket implementation for all types that implement [TryAllocDst].
unsafe impl<A, T: ?Sized + Dst> AllocDst<T> for A
where
    A: TryAllocDst<T>,
{
    unsafe fn new_dst<F>(len: usize, init: F) -> Result<Self, AllocDstError>
    where
        F: FnOnce(ptr::NonNull<T>) -> (),
    {
        match unsafe { Self::try_new_dst(len, |ptr| Ok::<(), Infallible>(init(ptr))) } {
            Ok(value) => Ok(value),
            Err(TryAllocDstError::Layout(e)) => Err(AllocDstError::Layout(e)),
            Err(TryAllocDstError::Init(infallible)) => match infallible {},
        }
    }
}

/// Type that can allocate a DST and store it inside it.
///
/// # Safety
///
/// Must be implemented as described. The `try_new_dst` function must not leak
/// memory in the case of `init` returning an error.
pub unsafe trait TryAllocDst<T: ?Sized + Dst>: Sized + AllocDst<T> {
    /// Allocate the DST with the given length, initialize the data with the
    /// given function, and store it in the type.
    ///
    /// # Safety
    ///
    /// The `init` function may not panic, otherwise there will be a memory leak.
    unsafe fn try_new_dst<F, E: Error>(len: usize, init: F) -> Result<Self, TryAllocDstError<E>>
    where
        F: FnOnce(ptr::NonNull<T>) -> Result<(), E>;
}

#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized + Dst> TryAllocDst<T> for Box<T> {
    unsafe fn try_new_dst<F, E: Error>(len: usize, init: F) -> Result<Self, TryAllocDstError<E>>
    where
        F: FnOnce(ptr::NonNull<T>) -> Result<(), E>,
    {
        let layout = T::layout(len)?;

        unsafe {
            let raw = if layout.size() == 0 {
                // FUTURE: switch to ptr::NonNull::without_provenance() when it has stabilised.
                ptr::NonNull::new(ptr::without_provenance_mut(layout.align()))
            } else {
                ptr::NonNull::new(alloc(layout))
            }
            .unwrap_or_else(|| handle_alloc_error(layout));
            let ptr = T::retype(raw, len);
            match init(ptr) {
                Ok(()) => (),
                Err(e) => {
                    if layout.size() != 0 {
                        dealloc(raw.as_ptr(), layout);
                    }
                    return Err(TryAllocDstError::Init(e));
                }
            }
            Ok(Box::from_raw(ptr.as_ptr()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "alloc")]
    #[test]
    fn str_test() {
        let str = "thisisatest";
        let boxed: Box<str> = unsafe {
            Box::new_dst(str.len(), |ptr: ptr::NonNull<str>| {
                str.clone_to_raw(ptr);
            })
            .unwrap()
        };

        assert_eq!(boxed.len(), str.len());
        assert_eq!(boxed.as_ref(), str);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn zst_test() {
        let arr: [(); 0] = [];
        let boxed: Box<[()]> = unsafe {
            Box::new_dst(arr.len(), |ptr: ptr::NonNull<[()]>| {
                arr.clone_to_raw(ptr);
            })
            .unwrap()
        };

        assert_eq!(boxed.len(), arr.len());
        assert_eq!(boxed.as_ref(), arr);
    }

    #[repr(C)]
    struct Type {
        data1: i16,
        data2: usize,
        data3: u32,
        slice: [i128],
    }

    unsafe impl Dst for Type {
        fn len(&self) -> usize {
            self.slice.len()
        }

        fn layout(len: usize) -> Result<Layout, LayoutError> {
            let (layout, _) = Self::layout_offsets(len)?;
            Ok(layout)
        }

        fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self> {
            // FUTURE: switch to ptr::NonNull:from_raw_parts() when it has stabilised.
            let ptr = ptr::NonNull::slice_from_raw_parts(ptr.cast::<()>(), len);
            unsafe { ptr::NonNull::new_unchecked(ptr.as_ptr() as *mut _) }
        }

        unsafe fn clone_to_raw(&self, ptr: ptr::NonNull<Self>) {
            unsafe {
                Self::write_to_raw(ptr, self.data1, self.data2, self.data3, &self.slice);
            }
        }
    }

    impl Type {
        fn layout_offsets(len: usize) -> Result<(Layout, [usize; 4]), LayoutError> {
            let data1_layout = Layout::new::<i16>();
            let data2_layout = Layout::new::<usize>();
            let data3_layout = Layout::new::<u32>();
            let slice_layout = Layout::array::<i128>(len)?;
            let fields = [data1_layout, data2_layout, data3_layout, slice_layout];
            let mut layout = fields[0];
            let mut offsets = [0; 4];
            for i in 1..4 {
                let (new_layout, offset) = layout.extend(fields[i])?;
                layout = new_layout;
                offsets[i] = offset;
            }
            Ok((layout.pad_to_align(), offsets))
        }

        unsafe fn write_to_raw(
            ptr: ptr::NonNull<Self>,
            data1: i16,
            data2: usize,
            data3: u32,
            slice: &[i128],
        ) {
            let (layout, offsets) = Self::layout_offsets(slice.len()).unwrap();
            let (data1_offset, data2_offset, data3_offset, slice_offset) =
                (offsets[0], offsets[1], offsets[2], offsets[3]);
            unsafe {
                let raw = ptr.as_ptr().cast::<u8>();
                ptr::write(raw.add(data1_offset).cast(), data1);
                ptr::write(raw.add(data2_offset).cast(), data2);
                ptr::write(raw.add(data3_offset).cast(), data3);
                ptr::copy_nonoverlapping(slice.as_ptr(), raw.add(slice_offset).cast(), slice.len());
                assert_eq!(Layout::for_value(ptr.as_ref()), layout);
            }
        }

        fn new(data1: i16, data2: usize, data3: u32, slice: &[i128]) -> Box<Self> {
            unsafe {
                Box::new_dst(slice.len(), |ptr: ptr::NonNull<Self>| {
                    Self::write_to_raw(ptr, data1, data2, data3, slice)
                })
                .unwrap()
            }
        }
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn complex_test() {
        let v = Type::new(-12, 65537, 50, &[-2, 5, 20]);
        assert_eq!(v.data1, -12);
        assert_eq!(v.data2, 65537);
        assert_eq!(v.data3, 50);
        assert_eq!(v.slice[0], -2);
        assert_eq!(v.slice[1], 5);
        assert_eq!(v.slice[2], 20);
        assert_eq!(v.len(), 3);
        assert_eq!(v.len(), v.slice.len());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn clone_test() {
        let v1 = Type::new(-12, 65537, 50, &[-2, 5, 20]);

        let v2 = unsafe { Box::new_dst(v1.len(), |ptr| v1.clone_to_raw(ptr)).unwrap() };
        assert_eq!(v2.data1, v1.data1);
        assert_eq!(v2.data2, v1.data2);
        assert_eq!(v2.data3, v1.data3);
        assert_eq!(v2.slice[0], v1.slice[0]);
        assert_eq!(v2.slice[1], v1.slice[1]);
        assert_eq!(v2.slice[2], v1.slice[2]);
        assert_eq!(v2.len(), v1.len());
    }
}
