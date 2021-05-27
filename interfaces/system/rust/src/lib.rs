#![feature(associated_type_defaults)]
#![feature(min_type_alias_impl_trait)]
#![allow(unused_imports)]
#![allow(clippy::ptr_arg)]
pub mod system {
    include!(concat!(env!("OUT_DIR"), "/system.rs"));
}
