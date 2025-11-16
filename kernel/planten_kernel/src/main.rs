#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(all(feature = "panic-handler", not(test)))]
use core::panic::PanicInfo;

mod vga_buffer;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    loop {}
}

#[cfg(test)]
fn main() {}

/// This function is called on panic.
#[cfg(all(feature = "panic-handler", not(test)))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
