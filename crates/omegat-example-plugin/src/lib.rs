use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn omegat_plugin_abi() -> *const c_char {
    b"{\"id\":\"example\",\"name\":\"Example\",\"version\":\"1.0.0\",\"kind\":\"filter\"}\0".as_ptr() as *const c_char
}
