use not_io::{AllowStd, Read, Write};

// Make sure that this includes the no-`alloc` subset of tests.
#[path = "no_std.rs"]
mod _std;

fn is_read<R: Read>() {}
fn is_write<W: Write>() {}

const XXX: () = {
    #[allow(dead_code)]
    fn generic_read<R: std::io::Read>() {
        let _ = is_read::<AllowStd<R>>;
    }
    #[allow(dead_code)]
    fn generic_write<W: std::io::Write>() {
        let _ = is_write::<AllowStd<W>>;
    }
};

#[test]
fn evaluate_consts() {
    let _: () = XXX;
}
