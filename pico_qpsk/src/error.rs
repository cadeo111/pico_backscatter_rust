// use anyhow::Error;
// use core::fmt::Write;
// use defmt::{write as dmf_write, Format, Formatter};
// use heapless::String;
// 
// pub struct DefmtErrorString(anyhow::Error);
// 
// impl From<anyhow::Error> for DefmtErrorString {
//     fn from(value: Error) -> Self {
//         Self(value)
//     }
// }
// 
// 
// impl DefmtErrorString {
//     fn get_str(&self) -> (Option<String<100>>, Option<String<500>>, Option<String<5000>>) {
//         {
//             let mut str = String::<100>::new();
//             let err = core::write!(str, "{:?}", self.0);
//             if let Err(_) = err {
//                 let mut str = String::<500>::new();
//                 let err = core::write!(str, "{:?}", self.0);
//                 if let Err(_) = err {
//                     let mut str = String::<5000>::new();
//                     core::write!(str, "{:?}", self.0).unwrap();
//                     return (None, None, Some(str.into()));
//                 }
//                 return (None, Some(str), None);
//             }
//             return (Some(str), None, None);
//         }
//     }
// 
// }
// impl Format for DefmtErrorString {
//     
//     fn format(&self, fmt: Formatter) {
//         let (short, long, long_long) = self.get_str();
//         if let Some(short) = short {
//             dmf_write!(fmt, "{}", short);
//             return;
//         }
//         if let Some(long) = long {
//             dmf_write!(fmt, "{}", long);
//             return;
//         }
//         if let Some(long_long) = long_long {
//             dmf_write!(fmt, "{}", long_long);
//             return;
//         }
//         dmf_write!(fmt, "error longer than 5000 bytes");
//     }
// }
