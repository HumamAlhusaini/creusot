use crate::*;

#[creusot::no_translate]
#[rustc_diagnostic_item = "fin"]
pub fn fin<T: ?Sized>(_: &mut T) -> Box<T> {
    panic!()
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "equal"]
pub fn equal<T: ?Sized>(_: T, _: T) -> bool {
    panic!();
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "neq"]
pub fn neq<T: ?Sized>(_: T, _: T) -> bool {
    panic!();
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "exists"]
pub fn exists<Tup: std::marker::Tuple, F: Fn<Tup, Output = bool>>(_: F) -> bool {
    panic!()
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "forall"]
pub fn forall<Tup: std::marker::Tuple, F: Fn<Tup, Output = bool>>(_: F) -> bool {
    panic!()
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "trigger"]
pub fn trigger<T, Trigger>(_: Trigger, _: T) -> T {
    panic!()
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "implication"]
pub fn implication(_: bool, _: bool) -> bool {
    panic!();
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "old"]
pub fn old<T: ?Sized>(_: T) -> Box<T> {
    panic!()
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "dead"]
pub fn dead<T: ?Sized>() -> Box<T> {
    panic!()
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "variant_check"]
pub fn variant_check<R: crate::well_founded::WellFounded>(_: R) -> R {
    panic!()
}

#[creusot::no_translate]
#[rustc_diagnostic_item = "closure_result_constraint"]
pub fn closure_result<R: ?Sized>(_: R, _: R) {}

#[crate::pure]
#[creusot::no_translate]
#[rustc_diagnostic_item = "snapshot_from_fn"]
pub fn snapshot_from_fn<T: ?Sized, F: Fn() -> crate::Snapshot<T>>(_: F) -> crate::Snapshot<T> {
    panic!()
}

#[logic]
#[trusted]
#[creusot::no_translate]
#[creusot::builtins = "identity"]
pub fn mapping_from_fn<A, B, F: FnOnce(A) -> B>(_: F) -> crate::logic::Mapping<A, B> {
    dead
}
