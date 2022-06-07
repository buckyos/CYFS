use crate::*;

use generic_array::typenum::{U32, U48};
use generic_array::{ArrayLength, GenericArray};
use std::convert::From;

//--------------------------
// DiffOpCode/DiffOpRef/DiffOp/RawDiff/RawPatch
//--------------------------

#[derive(Debug)]
pub enum DiffOpCode {
    // Scalar
    None,
    Set,

    // Option<T>
    SetNone,

    // Vec<T>
    Add,
    Remove,
    TrimEnd,
}

impl DiffOpCode {
    pub fn eq_or_set<T: PartialEq>(l: &T, r: &T) -> Self {
        if l.eq(r) {
            DiffOpCode::None
        } else {
            DiffOpCode::Set
        }
    }
}

impl From<&u8> for DiffOpCode {
    fn from(req_type: &u8) -> Self {
        match req_type {
            0u8 => DiffOpCode::None,
            1u8 => DiffOpCode::Set,
            2u8 => DiffOpCode::SetNone,
            3u8 => DiffOpCode::Add,
            4u8 => DiffOpCode::Remove,
            5u8 => DiffOpCode::TrimEnd,
            _ => DiffOpCode::None, // TODO
        }
    }
}

impl From<u8> for DiffOpCode {
    fn from(req_type: u8) -> Self {
        (&req_type).into()
    }
}

impl From<&DiffOpCode> for u8 {
    fn from(t: &DiffOpCode) -> u8 {
        match t {
            DiffOpCode::None => 0u8,
            DiffOpCode::Set => 1u8,
            DiffOpCode::SetNone => 2u8,
            DiffOpCode::Add => 3u8,
            DiffOpCode::Remove => 4u8,
            DiffOpCode::TrimEnd => 5u8,
        }
    }
}

impl From<DiffOpCode> for u8 {
    fn from(t: DiffOpCode) -> u8 {
        (&t).into()
    }
}

pub struct DiffOpRef<'op, T> {
    pub code: DiffOpCode,
    pub value: &'op T,
}

impl<'op, T> RawEncode for DiffOpRef<'op, T>
where
    T: RawEncode,
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        match self.code {
            DiffOpCode::None => Ok(u8::raw_bytes().unwrap()),
            _ => Ok(u8::raw_bytes().unwrap() + self.value.raw_measure(purpose)?),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for DiffOpRef",
            ));
        }

        let code: u8 = (&self.code).into();
        let buf = code.raw_encode(buf, purpose)?;

        match self.code {
            DiffOpCode::None => {
                println!("scalar type diff is none");
                Ok(buf)
            }
            _ => {
                let buf = self.value.raw_encode(buf, purpose)?;
                Ok(buf)
            }
        }
    }
}

pub struct DiffOp<T> {
    pub code: DiffOpCode,
    pub value: Option<T>,
}

impl<'de, T> RawDecode<'de> for DiffOp<T>
where
    T: RawDecode<'de>,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (code, buf) = u8::raw_decode(buf)?;

        match code.into() {
            DiffOpCode::None => Ok((
                Self {
                    code: code.into(),
                    value: None,
                },
                buf,
            )),
            _ => {
                let (value, buf) = T::raw_decode(buf)?;

                Ok((
                    Self {
                        code: code.into(),
                        value: Some(value),
                    },
                    buf,
                ))
            }
        }
    }
}

pub trait RawDiff: RawEncode {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize>;
    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]>;
}

pub trait RawPatch<'de>: RawDecode<'de> {
    fn patch(self, diff: &'de [u8]) -> BuckyResult<(Self, &'de [u8])>;
}

pub trait RawDiffWithContext<'v, Context>: RawEncode {
    fn diff_measure(&self, right: &'v Self, _: &mut Context) -> BuckyResult<usize>;
    fn diff<'d>(
        &self,
        right: &Self,
        buf: &'d mut [u8],
        _: &mut Context,
    ) -> BuckyResult<&'d mut [u8]>;
}

pub trait RawPatchWithContext<'de, Context>: RawDecode<'de> {
    fn patch(self, diff: &'de [u8], _: &mut Context) -> BuckyResult<(Self, &'de [u8])>;
}

//--------------------------
// u8
//--------------------------

impl RawDiff for u8 {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let op = DiffOpRef::<u8> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_measure(&None)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let op = DiffOpRef::<u8> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_encode(buf, &None)
    }
}

impl<'de> RawPatch<'de> for u8 {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = DiffOp::<u8>::raw_decode(buf)?;

        match op.code {
            DiffOpCode::None => Ok((self, buf)),
            DiffOpCode::Set => Ok((op.value.unwrap(), buf)),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                op.code
            ))),
        }
    }
}

//--------------------------
// u16
//--------------------------

impl RawDiff for u16 {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let op = DiffOpRef::<u16> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_measure(&None)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let op = DiffOpRef::<u16> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_encode(buf, &None)
    }
}

impl<'de> RawPatch<'de> for u16 {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = DiffOp::<u16>::raw_decode(buf)?;
        match op.code {
            DiffOpCode::None => Ok((self, buf)),
            DiffOpCode::Set => Ok((op.value.unwrap(), buf)),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                op.code
            ))),
        }
    }
}

//--------------------------
// u32
//--------------------------

impl RawDiff for u32 {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let op = DiffOpRef::<u32> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_measure(&None)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let op = DiffOpRef::<u32> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_encode(buf, &None)
    }
}

impl<'de> RawPatch<'de> for u32 {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = DiffOp::<u32>::raw_decode(buf)?;
        match op.code {
            DiffOpCode::None => Ok((self, buf)),
            DiffOpCode::Set => Ok((op.value.unwrap(), buf)),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                op.code
            ))),
        }
    }
}

//--------------------------
// u64
//--------------------------

impl RawDiff for u64 {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let op = DiffOpRef::<u64> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_measure(&None)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let op = DiffOpRef::<u64> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_encode(buf, &None)
    }
}

impl<'de> RawPatch<'de> for u64 {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = DiffOp::<u64>::raw_decode(buf)?;
        match op.code {
            DiffOpCode::None => Ok((self, buf)),
            DiffOpCode::Set => Ok((op.value.unwrap(), buf)),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                op.code
            ))),
        }
    }
}

//--------------------------
// u128
//--------------------------

impl RawDiff for u128 {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let op = DiffOpRef::<u128> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_measure(&None)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let op = DiffOpRef::<u128> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_encode(buf, &None)
    }
}

impl<'de> RawPatch<'de> for u128 {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = DiffOp::<u128>::raw_decode(buf)?;
        match op.code {
            DiffOpCode::None => Ok((self, buf)),
            DiffOpCode::Set => Ok((op.value.unwrap(), buf)),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                op.code
            ))),
        }
    }
}

//--------------------------
// String/str
//--------------------------

impl RawDiff for String {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let op = DiffOpRef::<String> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_measure(&None)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let op = DiffOpRef::<String> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_encode(buf, &None)
    }
}

impl<'de> RawPatch<'de> for String {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = DiffOp::<String>::raw_decode(buf)?;
        match op.code {
            DiffOpCode::None => Ok((self, buf)),
            DiffOpCode::Set => Ok((op.value.unwrap(), buf)),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                op.code
            ))),
        }
    }
}

impl RawFixedBytes for str {
    fn raw_min_bytes() -> Option<usize> {
        u16::raw_bytes()
    }
}

impl RawEncode for str {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(u16::raw_bytes().unwrap() + self.len())
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for str",
            ));
        }

        let buf = (self.len() as u16).raw_encode(buf, purpose)?;
        if self.len() == 0 {
            Ok(buf)
        } else {
            unsafe {
                std::ptr::copy::<u8>(self.as_ptr() as *mut u8, buf.as_mut_ptr(), self.len());
            }
            println!("buf len {}, self len {}", buf.len(), self.len());
            Ok(&mut buf[self.len()..])
        }
    }
}

impl RawDiff for str {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let code = if self == right {
            DiffOpCode::None
        } else {
            DiffOpCode::Set
        };
        match code {
            DiffOpCode::None => Ok(u8::raw_bytes().unwrap()),
            DiffOpCode::Set => Ok(u8::raw_bytes().unwrap() + right.raw_measure(&None)?),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                code
            ))),
        }
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let size = self.raw_measure(&None).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_diff] not enough buffer for str diff",
            ));
        }

        let code = if self == right {
            DiffOpCode::None
        } else {
            DiffOpCode::Set
        };

        let ucode: u8 = (&code).into();
        let buf = ucode.raw_encode(buf, &None)?;

        match code {
            DiffOpCode::None => Ok(buf),
            DiffOpCode::Set => {
                let buf = right.raw_encode(buf, &None)?;
                Ok(buf)
            }
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                code
            ))),
        }
    }
}

//--------------------------
// GenericArray
//--------------------------
impl<T: RawEncode + PartialEq, U: ArrayLength<T>> RawDiff for GenericArray<T, U> {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let op = DiffOpRef::<Self> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_measure(&None)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let op = DiffOpRef::<Self> {
            code: DiffOpCode::eq_or_set(self, right),
            value: right,
        };
        op.raw_encode(buf, &None)
    }
}

impl<'de, T: RawEncode + RawDecode<'de> + Default, U: ArrayLength<T>> RawPatch<'de>
    for GenericArray<T, U>
{
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = DiffOp::<Self>::raw_decode(buf)?;
        match op.code {
            DiffOpCode::None => {
                println!("genneric array patch is none");
                Ok((self, buf))
            }
            DiffOpCode::Set => Ok((op.value.unwrap(), buf)),
            _ => Err(BuckyError::from(format!(
                "Scalar Type Can not diff by opcode:{:?}",
                op.code
            ))),
        }
    }
}

//--------------------------
// Option<T>
//--------------------------
impl<T: RawDiff> RawDiff for Option<T> {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        match self {
            Some(left) => {
                match right {
                    Some(right) => {
                        // Compare
                        left.diff_measure(right)
                    }
                    None => {
                        // SetNone
                        Ok(u8::raw_bytes().unwrap())
                    }
                }
            }
            None => {
                match right {
                    Some(right) => {
                        // Set+right
                        let op = DiffOpRef::<T> {
                            code: DiffOpCode::Set,
                            value: right,
                        };
                        op.raw_measure(&None)
                    }
                    None => {
                        // None
                        Ok(u8::raw_bytes().unwrap())
                    }
                }
            }
        }
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        match self {
            Some(left) => {
                match right {
                    Some(right) => {
                        // Compare
                        left.diff(right, buf)
                    }
                    None => {
                        // SetNone
                        let code: u8 = DiffOpCode::SetNone.into();
                        code.raw_encode(buf, &None)
                    }
                }
            }
            None => {
                match right {
                    Some(right) => {
                        // Set+right
                        let op = DiffOpRef::<T> {
                            code: DiffOpCode::Set,
                            value: right,
                        };
                        op.raw_encode(buf, &None)
                    }
                    None => {
                        // None
                        let code: u8 = DiffOpCode::None.into();
                        code.raw_encode(buf, &None)
                    }
                }
            }
        }
    }
}

impl<'v, U, T: RawDiffWithContext<'v, VecDiffContext<'v, U>>>
    RawDiffWithContext<'v, VecDiffContext<'v, U>> for OptionRef<'v, T>
{
    fn diff_measure(&self, right: &'v Self, ctx: &mut VecDiffContext<'v, U>) -> BuckyResult<usize> {
        let left: Option<&'v T> = self.option();
        let right: Option<&'v T> = right.option();
        match left {
            Some(left) => {
                match right {
                    Some(right) => {
                        // Compare
                        left.diff_measure(right, ctx)
                    }
                    None => {
                        // SetNone
                        Ok(u8::raw_bytes().unwrap())
                    }
                }
            }
            None => {
                match right {
                    Some(right) => {
                        // Set+right
                        let op = DiffOpRef::<T> {
                            code: DiffOpCode::Set,
                            value: right,
                        };
                        op.raw_measure(&None)
                    }
                    None => {
                        // None
                        Ok(u8::raw_bytes().unwrap())
                    }
                }
            }
        }
    }

    fn diff<'d>(
        &self,
        right: &Self,
        buf: &'d mut [u8],
        ctx: &mut VecDiffContext<'v, U>,
    ) -> BuckyResult<&'d mut [u8]> {
        let left: Option<&'v T> = self.option();
        let right: Option<&'v T> = right.option();
        match left {
            Some(left) => {
                match right {
                    Some(right) => {
                        // Compare
                        left.diff(right, buf, ctx)
                    }
                    None => {
                        // SetNone
                        let code: u8 = DiffOpCode::SetNone.into();
                        code.raw_encode(buf, &None)
                    }
                }
            }
            None => {
                match right {
                    Some(right) => {
                        // Set+right
                        let op = DiffOpRef::<T> {
                            code: DiffOpCode::Set,
                            value: right,
                        };
                        op.raw_encode(buf, &None)
                    }
                    None => {
                        // None
                        let code: u8 = DiffOpCode::None.into();
                        code.raw_encode(buf, &None)
                    }
                }
            }
        }
    }
}

impl<'de, T> RawPatch<'de> for Option<T>
where
    T: RawDecode<'de>,
{
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = u8::raw_decode(buf)?;
        let code = op.into();
        match code {
            DiffOpCode::None => {
                // 没有改变
                Ok((self, buf))
            }
            DiffOpCode::Set => {
                // 设置为right
                let (right, buf) = T::raw_decode(buf)?;
                Ok((Some(right), buf))
            }
            DiffOpCode::SetNone => {
                // 设置为None
                Ok((None, buf))
            }
            _ => Err(BuckyError::from(format!(
                "Opton<T> Type Can not patch by opcode:{:?}",
                code
            ))),
        }
    }
}

//--------------------------
// Vec<T>
//--------------------------
pub struct ItemChangeRef<'v, T> {
    pub code: DiffOpCode,
    pub index: usize,
    pub value: Option<&'v T>,
}

impl<'v, T> ItemChangeRef<'v, T> {
    pub fn new(code: DiffOpCode, index: usize, value: Option<&'v T>) -> Self {
        Self { code, index, value }
    }
}

impl<'v, T> RawEncode for ItemChangeRef<'v, T>
where
    T: RawEncode,
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let mut size = u8::raw_bytes().unwrap() + u32::raw_bytes().unwrap();

        if self.value.is_some() {
            size = size + self.value.unwrap().raw_measure(purpose)?;
        } else {
            //
        }

        Ok(size)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose)?;
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for ItemChangeRef",
            ));
        }

        let code: u8 = (&self.code).into();
        let buf = code.raw_encode(buf, purpose)?;
        let buf = (self.index as u32).raw_encode(buf, purpose)?;

        if self.value.is_some() {
            let buf = self.value.unwrap().raw_encode(buf, purpose)?;
            Ok(buf)
        } else {
            Ok(buf)
        }
    }
}

pub struct ItemChange<T> {
    pub code: DiffOpCode,
    pub index: usize,
    pub value: Option<T>,
}

impl<T> RawEncode for ItemChange<T>
where
    T: RawEncode,
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let mut size = u8::raw_bytes().unwrap() + u32::raw_bytes().unwrap();

        if self.value.is_some() {
            size = size + self.value.as_ref().unwrap().raw_measure(purpose)?;
        } else {
            //
        }

        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose)?;
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for ItemChange",
            ));
        }

        let code: u8 = (&self.code).into();
        let buf = code.raw_encode(buf, purpose)?;
        let buf = (self.index as u32).raw_encode(buf, purpose)?;

        if self.value.is_some() {
            let buf = self.value.as_ref().unwrap().raw_encode(buf, purpose)?;
            Ok(buf)
        } else {
            Ok(buf)
        }
    }
}

impl<'de, T> RawDecode<'de> for ItemChange<T>
where
    T: RawDecode<'de>,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 1 {
            let msg = format!(
                "not enough buffer for encode ItemChange, min bytes={}, got={}",
                1,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let (code, buf) = u8::raw_decode(buf)?;
        let (index, buf) = u32::raw_decode(buf)?;

        let code: DiffOpCode = code.into();
        match code {
            DiffOpCode::Add => {
                let (value, buf) = T::raw_decode(buf)?;
                Ok((
                    Self {
                        code,
                        index: index as usize,
                        value: Some(value),
                    },
                    buf,
                ))
            }
            _ => Ok((
                Self {
                    code,
                    index: index as usize,
                    value: None,
                },
                buf,
            )),
        }
    }
}

pub struct VecDiffContext<'v, T> {
    change_list: Vec<ItemChangeRef<'v, T>>,
}

impl<'v, T> Default for VecDiffContext<'v, T> {
    fn default() -> Self {
        Self {
            change_list: Vec::new(),
        }
    }
}

impl<'v, T> VecDiffContext<'v, T> {
    pub fn change(&mut self, range: ItemChangeRef<'v, T>) {
        self.change_list.push(range);
    }

    pub fn change_list(&self) -> &Vec<ItemChangeRef<'v, T>> {
        &self.change_list
    }
}

impl<'v, T: RawDiff + PartialEq> RawDiffWithContext<'v, VecDiffContext<'v, T>> for Vec<T> {
    fn diff_measure(&self, right: &'v Self, ctx: &mut VecDiffContext<'v, T>) -> BuckyResult<usize> {
        // diff here

        if self.len() == 0 && right.len() == 0 {
            // 两边都是空，没有变化，None
            Ok(u8::raw_bytes().unwrap())
        } else if self.len() == 0 {
            // 左边为空，右边非空，设置为右边 Set
            Ok(u8::raw_bytes().unwrap() + right.raw_measure(&None)?)
        } else if right.len() == 0 {
            // 左边非空，右边为空，清空 SetNone
            Ok(u8::raw_bytes().unwrap())
        } else {
            // 两边都非空

            let mut right_start = 0;

            // 比对
            {
                let mut i = 0;
                let mut k = 0;

                while i < self.len() {
                    // 左边还有剩余，右边结束，左边剩余的全部移除
                    if right_start == right.len() {
                        println!("==> truncate at:{}", k);
                        ctx.change(ItemChangeRef::<T>::new(DiffOpCode::TrimEnd, k, None));
                        break;
                    }

                    // 从right_start开始扫描右边，查找和self[i]相等的元素
                    // 如果找不到，则移除self[i]
                    // 否则，添加right[right_start...j]
                    let mut hint = None;
                    for j in right_start..right.len() {
                        if self[i] == right[j] {
                            hint = Some(j)
                        }
                    }

                    // 添加或移除
                    match hint {
                        Some(hint) => {
                            // 反复插入元素，并更新下一个变动点k
                            for j in right_start..hint {
                                println!("==> add at:{}, hint:{}", k, hint);
                                ctx.change(ItemChangeRef::<T>::new(
                                    DiffOpCode::Add,
                                    k,
                                    Some(&right[j]),
                                ));
                                k = k + 1;
                            }

                            println!("==> keep at:{}", k);
                            k = k + 1;

                            // 到hint为止的right已经被处理
                            right_start = hint + 1;
                        }
                        None => {
                            println!("==> remove at:{}", k);
                            // 在k处移除元素，k不动
                            ctx.change(ItemChangeRef::<T>::new(DiffOpCode::Remove, k, None));
                        }
                    }

                    i = i + 1;
                }

                // 右边还有剩余，左边结束，把右边剩余的全部添加
                if right_start < right.len() {
                    for j in right_start..right.len() {
                        println!("==> add at:{}, right_start:{}", k, right_start);
                        ctx.change(ItemChangeRef::<T>::new(DiffOpCode::Add, k, Some(&right[j])));
                        k = k + 1;
                    }
                }
            }

            Ok(u8::raw_bytes().unwrap() + ctx.change_list().raw_measure(&None)?)
        }
    }

    fn diff<'d>(
        &self,
        right: &Self,
        buf: &'d mut [u8],
        ctx: &mut VecDiffContext<'v, T>,
    ) -> BuckyResult<&'d mut [u8]> {
        let size = self.raw_measure(&None)?;
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_diff] not enough buffer for VecDiffContext",
            ));
        }

        if self.len() == 0 && right.len() == 0 {
            // 两边都是空，没有变化，None
            let code: u8 = DiffOpCode::None.into();
            let buf = code.raw_encode(buf, &None)?;
            Ok(buf)
        } else if self.len() == 0 {
            // 左边为空，右边非空，设置为右边 Set
            let code: u8 = DiffOpCode::Set.into();
            let buf = code.raw_encode(buf, &None)?;
            let buf = right.raw_encode(buf, &None)?;
            Ok(buf)
        } else if right.len() == 0 {
            // 左边非空，右边为空，清空 SetNone
            let code: u8 = DiffOpCode::SetNone.into();
            let buf = code.raw_encode(buf, &None)?;
            Ok(buf)
        } else {
            // 两边都非空
            let code: u8 = DiffOpCode::Add.into();
            let buf = code.raw_encode(buf, &None)?;
            let buf = ctx.change_list().raw_encode(buf, &None)?;
            Ok(buf)
        }
    }
}

impl<'de, T> RawPatch<'de> for Vec<T>
where
    T: RawDecode<'de> + RawEncode,
{
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (op, buf) = u8::raw_decode(buf)?;
        let code = op.into();

        let mut left = self;

        match code {
            DiffOpCode::None => {
                // 没有改变
                Ok((left, buf))
            }
            DiffOpCode::Set => {
                // 设置为right
                let (right, buf) = Vec::<T>::raw_decode(buf)?;
                Ok((right, buf))
            }
            DiffOpCode::SetNone => {
                // 设置为None
                Ok((Vec::new(), buf))
            }
            DiffOpCode::Add => {
                // 通过Add重建
                let (change_list, buf) = Vec::<ItemChange<T>>::raw_decode(buf)?;

                // 重建
                println!("change_list:{}", change_list.len());

                for change in change_list {
                    match change.code {
                        DiffOpCode::Add => {
                            if change.value.is_none() {
                                return Err(BuckyError::from("missing add value"));
                            }

                            println!("insert value at {}", change.index);
                            left.insert(change.index, change.value.unwrap());
                        }
                        DiffOpCode::Remove => {
                            println!("remove value at {}", change.index);
                            left.remove(change.index);
                        }
                        DiffOpCode::TrimEnd => {
                            println!("truncate value at {}", change.index);
                            left.truncate(change.index);
                        }
                        _ => {
                            return Err(BuckyError::from("missing add value"));
                        }
                    }
                }

                Ok((left, buf))
            }
            _ => Err(BuckyError::from(format!(
                "Vec<T> Type Can not patch by opcode:{:?}",
                code
            ))),
        }
    }
}

//--------------------------
// SizedOwnedData<T>
//--------------------------
impl<'v, T: From<usize> + RawFixedBytes + RawEncode> RawDiffWithContext<'v, VecDiffContext<'v, u8>>
    for SizedOwnedData<T>
{
    fn diff_measure(
        &self,
        right: &'v Self,
        ctx: &mut VecDiffContext<'v, u8>,
    ) -> BuckyResult<usize> {
        // TODO: 优化
        let data = self.as_ref();
        let r = right.as_ref();
        data.diff_measure(r, ctx)
    }

    fn diff<'d>(
        &self,
        right: &Self,
        buf: &'d mut [u8],
        ctx: &mut VecDiffContext<'v, u8>,
    ) -> BuckyResult<&'d mut [u8]> {
        // TODO: 优化
        self.as_ref().diff(right.as_ref(), buf, ctx)
    }
}

impl<'de, T: From<usize> + RawDecode<'de> + Into<usize>> RawPatch<'de> for SizedOwnedData<T> {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        // TODO: 优化
        let data: Vec<u8> = self.into();
        let (data, buf) = data.patch(buf)?;
        Ok((SizedOwnedData::<T>::from(data), buf))
    }
}

//--------------------------
// HashValue
//--------------------------

impl RawDiff for HashValue {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let data = self.as_ref();
        let r = right.as_ref();
        data.diff_measure(r)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        self.as_ref().diff(right.as_ref(), buf)
    }
}

impl<'de> RawPatch<'de> for HashValue {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let data: GenericArray<u8, U32> = self.into();
        let (data, buf) = data.patch(buf)?;
        Ok((HashValue::from(data), buf))
    }
}

//--------------------------
// AesKey
//--------------------------

impl RawDiff for AesKey {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let data = self.as_ref();
        let r = right.as_ref();
        data.diff_measure(r)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        self.as_ref().diff(right.as_ref(), buf)
    }
}

impl<'de> RawPatch<'de> for AesKey {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let data: GenericArray<u8, U48> = self.into();
        let (data, buf) = data.patch(buf)?;
        Ok((AesKey::from(data), buf))
    }
}

//--------------------------
// Unit Test
//--------------------------

#[cfg(test)]
mod test_diff {
    use crate::codec::{SizeU32, SizedOwnedData};
    use crate::crypto::{AesKey, HashValue};
    use super::*;
    use generic_array::typenum::{U32, U48};
    use generic_array::GenericArray;
    use crate::objects::raw_diff::RawDiff;

    #[test]
    fn test_u8_diff_patch() {
        let no = 10u8;
        let other = 11u8;

        let size = no.diff_measure(&other).unwrap();
        let mut buf = vec![0u8; size];

        let old = no;

        let _ = no.diff(&other, &mut buf).unwrap();
        let (new_no, _) = no.patch(&buf).unwrap();

        println!("old_no:{}, new_no:{}", old, new_no);

        assert!(new_no == other);
    }

    #[test]
    fn test_string_diff() {
        let left = String::from("Hello");
        let right = String::from("HelloWorld");

        let size = (&left).diff_measure(&right).unwrap();
        let mut buf = vec![0u8; size];

        let _ = (&left).diff(&right, &mut buf).unwrap();
        let (left, _) = left.patch(&buf).unwrap();

        assert_eq!(left, right);
    }

    #[test]
    fn test_str_diff() {
        let left = "Hello";
        let right = "HelloWorld";

        let size = left.diff_measure(&right).unwrap();
        let mut buf = vec![0u8; size];

        println!("size:{}", size);
        let _ = left.diff(&right, &mut buf).unwrap();
        let (left, _) = left.to_string().patch(&buf).unwrap();

        assert_eq!(left, right);
    }

    #[test]
    fn test_generic_array_diff() {
        let left = GenericArray::<u8, U32>::default();
        let right = GenericArray::<u8, U32>::default();

        let size = left.diff_measure(&right).unwrap();
        let mut buf = vec![0u8; size];

        println!("size:{}", size);
        let _ = left.diff(&right, &mut buf).unwrap();
        let (left, _) = left.patch(&buf).unwrap();

        assert_eq!(left, right);

        // assert!(false)
    }

    #[test]
    fn test_optoin_t_diff() {
        // some some
        {
            let left = Some(10u8);
            let right = Some(11u8);

            let size = left.diff_measure(&right).unwrap();
            let mut buf = vec![0u8; size];

            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left:{:?}, right:{:?}", left, right);

            assert_eq!(left, right);
        }

        {
            let left = Some("Hello World!".to_string());
            let right = Some("你好，世界".to_string());

            let size = left.diff_measure(&right).unwrap();
            let mut buf = vec![0u8; size];

            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left:{:?}, right:{:?}", left, right);

            assert_eq!(left, right);
        }

        // some none
        {
            let left = Some("Hello World".to_string());
            let right = None;

            let size = left.diff_measure(&right).unwrap();
            let mut buf = vec![0u8; size];

            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left:{:?}, right:{:?}", left, right);

            assert_eq!(left, right);
        }

        // none none
        {
            let left: Option<u128> = None;
            let right = None;

            let size = left.diff_measure(&right).unwrap();
            let mut buf = vec![0u8; size];

            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left:{:?}, right:{:?}", left, right);

            assert_eq!(left, right);
        }

        // none some
        {
            let left: Option<GenericArray<u8, U32>> = None;
            let right = Some(GenericArray::<u8, U32>::default());

            let size = left.diff_measure(&right).unwrap();
            let mut buf = vec![0u8; size];

            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left:{:?}, right:{:?}", left, right);

            assert_eq!(left, right);
        }

        // assert!(false)
    }

    #[test]
    fn test_vec_diff() {
        println!("\n\n");
        // left.len() == right.len()
        {
            let left = vec![1u8, 2u8, 3u8];
            let right = vec![1u8, 4u8, 3u8];

            println!("left before diff and patch:{:?}", left);
            println!("right before diff and patch:{:?}", right);

            let mut ctx = VecDiffContext::default();
            let size = left.diff_measure(&right, &mut ctx).unwrap();
            let mut buf = vec![0u8; size];
            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf, &mut ctx).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left after diff and patch:{:?}", left);

            assert_eq!(left, right);
        }

        println!("\n\n");
        // left.len() < right.len()
        {
            let left = vec![1u8, 2u8];
            let right = vec![1u8, 4u8, 3u8];

            println!("left before diff and patch:{:?}", left);
            println!("right before diff and patch:{:?}", right);

            let mut ctx = VecDiffContext::default();
            let size = left.diff_measure(&right, &mut ctx).unwrap();
            let mut buf = vec![0u8; size];
            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf, &mut ctx).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left after diff and patch:{:?}", left);

            // assert_eq!(left, right);
        }

        println!("\n\n");
        // left.len() > right.len()
        {
            let left = vec![1u8, 2u8, 3u8, 5u8];
            let right = vec![1u8, 4u8, 3u8];

            println!("left before diff and patch:{:?}", left);
            println!("right before diff and patch:{:?}", right);

            let mut ctx = VecDiffContext::default();
            let size = left.diff_measure(&right, &mut ctx).unwrap();
            let mut buf = vec![0u8; size];
            println!("size:{}", size);

            let _ = left.diff(&right, &mut buf, &mut ctx).unwrap();
            let (left, _) = left.patch(&buf).unwrap();

            println!("left after diff and patch:{:?}", left);

            assert_eq!(left, right);
        }

        println!("\n\n");

        // assert!(false)
    }

    #[test]
    fn test_sizedowneddata_diff() {
        let left = SizedOwnedData::<SizeU32>::from(vec![1u8, 2u8, 3u8]);
        let right = SizedOwnedData::<SizeU32>::from(vec![1u8, 4u8, 3u8]);

        println!("left before diff and patch:{:?}", left);
        println!("right before diff and patch:{:?}", right);

        let mut ctx = VecDiffContext::default();
        let size = left.diff_measure(&right, &mut ctx).unwrap();
        let mut buf = vec![0u8; size];
        println!("size:{}", size);

        let _ = left.diff(&right, &mut buf, &mut ctx).unwrap();
        let (left, _) = left.patch(&buf).unwrap();

        println!("left after diff and patch:{:?}", left);

        assert_eq!(left, right);
    }

    #[test]
    fn test_hashvalue_diff() {
        let left = HashValue::from(GenericArray::<u8, U32>::default());
        let right = HashValue::from(GenericArray::<u8, U32>::default());

        let size = left.diff_measure(&right).unwrap();
        let mut buf = vec![0u8; size];

        println!("size:{}", size);
        let _ = left.diff(&right, &mut buf).unwrap();
        let (left, _) = left.patch(&buf).unwrap();

        assert_eq!(left, right);

        // assert!(false)
    }

    #[test]
    fn test_aes_key_diff() {
        let left = AesKey::from(GenericArray::<u8, U48>::default());
        let right = AesKey::from(GenericArray::<u8, U48>::default());

        let size = left.diff_measure(&right).unwrap();
        let mut buf = vec![0u8; size];

        println!("size:{}", size);
        let _ = left.diff(&right, &mut buf).unwrap();
        let (left, _) = left.patch(&buf).unwrap();

        assert_eq!(left, right);

        // assert!(false)
    }
}
