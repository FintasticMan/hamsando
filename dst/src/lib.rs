#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::{
    alloc::{alloc, handle_alloc_error},
    boxed::Box,
};
use core::{
    alloc::{Layout, LayoutError},
    convert::Infallible,
    error::Error,
    ptr,
};

pub unsafe trait Dst {
    fn len(&self) -> usize;

    fn layout(len: usize) -> Result<(Layout, impl IntoIterator<Item = usize>), LayoutError>;

    unsafe fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self>;

    unsafe fn clone_to_raw(&self, ptr: ptr::NonNull<Self>);
}

unsafe impl<T> Dst for [T] {
    fn len(&self) -> usize {
        self.len()
    }

    fn layout(len: usize) -> Result<(Layout, impl IntoIterator<Item = usize>), LayoutError> {
        Ok((Layout::array::<T>(len)?, [0]))
    }

    unsafe fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self> {
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

    fn layout(len: usize) -> Result<(Layout, impl IntoIterator<Item = usize>), LayoutError> {
        Ok((Layout::array::<u8>(len)?, [0]))
    }

    unsafe fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self> {
        // FUTURE: switch to ptr::NonNull:from_raw_parts() when it has stabilised.
        let ptr = ptr::NonNull::slice_from_raw_parts(ptr.cast::<()>(), len);
        unsafe { ptr::NonNull::new_unchecked(ptr.as_ptr() as *mut _) }
    }

    unsafe fn clone_to_raw(&self, ptr: ptr::NonNull<Self>) {
        unsafe { ptr::copy_nonoverlapping(self.as_ptr(), ptr.as_ptr().cast(), self.len()) };
    }
}

pub unsafe trait AllocDst<T: ?Sized + Dst>: Sized {
    unsafe fn new_dst<F>(len: usize, init: F) -> Self
    where
        F: FnOnce(ptr::NonNull<T>) -> ();
}

unsafe impl<A, T: ?Sized + Dst> AllocDst<T> for A
where
    A: TryAllocDst<T>,
{
    unsafe fn new_dst<F>(len: usize, init: F) -> Self
    where
        F: FnOnce(ptr::NonNull<T>) -> (),
    {
        match unsafe { Self::try_new_dst(len, |ptr| Ok::<(), Infallible>(init(ptr))) } {
            Ok(value) => value,
            Err(infallible) => match infallible {},
        }
    }
}

pub unsafe trait TryAllocDst<T: ?Sized + Dst>: Sized + AllocDst<T> {
    unsafe fn try_new_dst<F, E: Error>(len: usize, init: F) -> Result<Self, E>
    where
        F: FnOnce(ptr::NonNull<T>) -> Result<(), E>;
}

#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized + Dst> TryAllocDst<T> for Box<T> {
    unsafe fn try_new_dst<F, E: Error>(len: usize, init: F) -> Result<Self, E>
    where
        F: FnOnce(ptr::NonNull<T>) -> Result<(), E>,
    {
        let (layout, _) = T::layout(len).expect("invalid layout");

        unsafe {
            let ptr = if layout.size() == 0 {
                // FUTURE: switch to ptr::NonNull::without_provenance() when it has stabilised.
                ptr::NonNull::new(ptr::without_provenance_mut(layout.align()))
            } else {
                ptr::NonNull::new(alloc(layout))
            }
            .unwrap_or_else(|| handle_alloc_error(layout));
            let ptr = T::retype(ptr, len);
            init(ptr)?;
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
                ptr::copy_nonoverlapping(str.as_ptr(), ptr.as_ptr().cast(), str.len());
            })
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

        fn layout(len: usize) -> Result<(Layout, impl IntoIterator<Item = usize>), LayoutError> {
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

        unsafe fn retype(ptr: ptr::NonNull<u8>, len: usize) -> ptr::NonNull<Self> {
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
        unsafe fn write_to_raw(
            ptr: ptr::NonNull<Self>,
            data1: i16,
            data2: usize,
            data3: u32,
            slice: &[i128],
        ) {
            let (layout, offsets) = Self::layout(slice.len()).unwrap();
            let mut offsets = offsets.into_iter();
            let (Some(data1_offset), Some(data2_offset), Some(data3_offset), Some(slice_offset)) = (
                offsets.next(),
                offsets.next(),
                offsets.next(),
                offsets.next(),
            ) else {
                panic!();
            };
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
        assert_eq!(v.slice.len(), 3);
        assert_eq!(v.len(), v.slice.len());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn clone_test() {
        let v1 = Type::new(-12, 65537, 50, &[-2, 5, 20]);

        let v2 = unsafe { Box::new_dst(v1.slice.len(), |ptr| v1.clone_to_raw(ptr)) };
        assert_eq!(v2.data1, v1.data1);
        assert_eq!(v2.data2, v1.data2);
        assert_eq!(v2.data3, v1.data3);
        assert_eq!(v2.slice[0], v1.slice[0]);
        assert_eq!(v2.slice[1], v1.slice[1]);
        assert_eq!(v2.slice[2], v1.slice[2]);
        assert_eq!(v2.slice.len(), v1.slice.len());
    }
}
