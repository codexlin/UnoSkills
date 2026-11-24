#![cfg(target_os = "windows")]

use windows::{
    Win32::{
        Foundation::{FreeLibrary, HINSTANCE},
        System::LibraryLoader::{
            LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW,
        },
        UI::WindowsAndMessaging::LoadIconW,
    },
    core::{HSTRING, PCWSTR},
};

#[test]
fn application_icon_loads_from_resource_group_one() {
    let executable = HSTRING::from(env!("CARGO_BIN_EXE_unoskills-app"));
    // SAFETY: The path is NUL-terminated. The flags map the EXE's resources
    // without running any of its code.
    let module = unsafe {
        LoadLibraryExW(
            &executable,
            None,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    }
    .expect("map the compiled UnoSkills executable");

    // SAFETY: The module is mapped and the pointer encodes a Win32 resource ordinal.
    let icon = unsafe {
        LoadIconW(
            Some(HINSTANCE(module.0)),
            PCWSTR(std::ptr::without_provenance(1)),
        )
    };
    // SAFETY: The handle came from LoadLibraryExW and is no longer needed.
    unsafe { FreeLibrary(module) }.expect("unmap the executable");
    icon.expect("load RT_GROUP_ICON 1 from the compiled UnoSkills executable");
}
