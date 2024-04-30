#![feature(test)]
extern crate test;

use std::any::Any;
use test::test_main_static_abort;

#[no_mangle]
pub fn rustyrts_runner(tests: &[&test::TestDescAndFn]) {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("{}", info);
        let payload = info
            .payload()
            .downcast_ref::<String>()
            .map(|e| unsafe { std::mem::transmute(&**e) })
            .or_else(|| info.payload().downcast_ref::<&'static str>().copied())
            .unwrap_or("panic ocurred");

        // This effectively bypasses the panic hook
        std::panic::resume_unwind(Box::new(payload) as Box<dyn Any + Send>);
    }));

    // When panic=abort is set (which would normally lead to a call to test_main_static_abort), a separate process is
    // forked for every test case.
    // Additionally, a panic hook is set that aborts instead of unwinding. We can revoke this by resuming unwinding in
    // an additional panic hook.
    test_main_static_abort(tests);
}
