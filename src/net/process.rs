use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::size_of;
use std::process::Command;
use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCPTABLE_OWNER_PID, MIB_UDPTABLE_OWNER_PID,
    TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::AF_INET;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetProcessIoCounters, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, IO_COUNTERS,
    PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

use crate::utils::to_wide_chars;

pub const CRITICAL_PROCESSES: &[&str] = &[
    "system",
    "system idle process",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "services.exe",
    "lsass.exe",
    "fontdrvhost.exe",
    "dwm.exe",
    "registry",
];

pub fn get_common_app_names() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("chrome", "Google Chrome");
    m.insert("brave", "Brave Browser");
    m.insert("msedge", "Microsoft Edge");
    m.insert("firefox", "Firefox");
    m.insert("opera", "Opera Browser");
    m.insert("discord", "Discord");
    m.insert("spotify", "Spotify");
    m.insert("steam", "Steam");
    m.insert("steamservice", "Steam Service");
    m.insert("telegram", "Telegram");
    m.insert("code", "VS Code");
    m.insert("chatgpt", "ChatGPT");
    m.insert("slack", "Slack");
    m.insert("zoom", "Zoom");
    m.insert("teams", "Microsoft Teams");
    m.insert("python", "Python");
    m.insert("pythonw", "Python");
    m.insert("svchost", "Windows Services");
    m.insert("system", "Windows System");
    m.insert("searchhost", "Windows Search");
    m.insert("startmenuexperiencehost", "Windows Start Menu");
    m.insert("lockapp", "Windows Lockscreen");
    m.insert("runtimebroker", "Runtime Broker");
    m.insert("explorer", "Windows Explorer");
    m.insert("onedrive", "OneDrive");
    m.insert("dropbox", "Dropbox");
    m
}

pub fn get_active_network_pids() -> HashSet<u32> {
    let mut pids = HashSet::new();

    // 1. Extended TCP Table (IPv4)
    unsafe {
        let mut size = 0u32;
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            true,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );

        if size > 0 {
            let mut buffer = vec![0u8; size as usize];
            if GetExtendedTcpTable(
                Some(buffer.as_mut_ptr() as *mut c_void),
                &mut size,
                true,
                AF_INET.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            ) == 0
            {
                let table = &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
                let num_entries = table.dwNumEntries as usize;
                let rows = std::slice::from_raw_parts(table.table.as_ptr(), num_entries);
                for row in rows {
                    let remote_addr = row.dwRemoteAddr;
                    // remote_addr is in network byte order: 127.0.0.1 is 0x0100007f
                    let is_loopback = (remote_addr & 0x000000ff) == 0x7f;
                    let is_zero = remote_addr == 0;
                    if !is_loopback && !is_zero && row.dwOwningPid > 0 {
                        pids.insert(row.dwOwningPid);
                    }
                }
            }
        }
    }

    // 2. Extended UDP Table (IPv4)
    unsafe {
        let mut size = 0u32;
        let _ = GetExtendedUdpTable(
            None,
            &mut size,
            true,
            AF_INET.0 as u32,
            UDP_TABLE_OWNER_PID,
            0,
        );

        if size > 0 {
            let mut buffer = vec![0u8; size as usize];
            if GetExtendedUdpTable(
                Some(buffer.as_mut_ptr() as *mut c_void),
                &mut size,
                true,
                AF_INET.0 as u32,
                UDP_TABLE_OWNER_PID,
                0,
            ) == 0
            {
                let table = &*(buffer.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
                let num_entries = table.dwNumEntries as usize;
                let rows = std::slice::from_raw_parts(table.table.as_ptr(), num_entries);
                for row in rows {
                    if row.dwOwningPid > 0 {
                        pids.insert(row.dwOwningPid);
                    }
                }
            }
        }
    }

    pids
}

pub fn get_process_io(pid: u32) -> Option<(u64, u64)> {
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_QUERY_INFORMATION,
            false,
            pid,
        );
        if let Ok(h) = handle {
            if !h.is_invalid() {
                let mut io = IO_COUNTERS::default();
                let res = GetProcessIoCounters(h, &mut io);
                let _ = CloseHandle(h);
                if res.is_ok() {
                    return Some((io.ReadTransferCount, io.WriteTransferCount));
                }
            }
        }
    }
    None
}

pub fn get_process_info(pid: u32, name_cache: &mut HashMap<String, String>) -> (String, String) {
    let mut exe_name = String::new();
    let mut exe_path = String::new();

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if let Ok(h) = handle {
            if !h.is_invalid() {
                let mut path_buf = [0u16; 1024];
                let mut size = path_buf.len() as u32;
                if QueryFullProcessImageNameW(
                    h,
                    PROCESS_NAME_WIN32,
                    windows::core::PWSTR(path_buf.as_mut_ptr()),
                    &mut size,
                )
                .is_ok()
                {
                    exe_path = String::from_utf16_lossy(&path_buf[..size as usize]);
                    if let Some(pos) = exe_path.rfind('\\') {
                        exe_name = exe_path[pos + 1..].to_string();
                    } else {
                        exe_name = exe_path.clone();
                    }
                }
                let _ = CloseHandle(h);
            }
        }
    }

    // Fallback using Toolhelp snapshot if QueryFullProcessImageName failed
    if exe_name.is_empty() {
        unsafe {
            if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W {
                    dwSize: size_of::<PROCESSENTRY32W>() as u32,
                    ..Default::default()
                };
                if Process32FirstW(snap, &mut entry).is_ok() {
                    loop {
                        if entry.th32ProcessID == pid {
                            let len = entry
                                .szExeFile
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(entry.szExeFile.len());
                            exe_name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                            break;
                        }
                        if Process32NextW(snap, &mut entry).is_err() {
                            break;
                        }
                    }
                }
                let _ = CloseHandle(snap);
            }
        }
    }

    if exe_name.is_empty() {
        exe_name = format!("pid-{}", pid);
    }

    if let Some(cached) = name_cache.get(&exe_name) {
        return (cached.clone(), exe_name);
    }

    let mut friendly = String::new();

    // Try reading FileDescription from FileVersionInfo
    if !exe_path.is_empty() {
        let wide_path = to_wide_chars(&exe_path);
        unsafe {
            let mut handle = 0u32;
            let size = GetFileVersionInfoSizeW(PCWSTR(wide_path.as_ptr()), Some(&mut handle));
            if size > 0 {
                let mut buffer = vec![0u8; size as usize];
                if GetFileVersionInfoW(
                    PCWSTR(wide_path.as_ptr()),
                    0,
                    size,
                    buffer.as_mut_ptr() as *mut c_void,
                )
                .is_ok()
                {
                    // Try English language codepage
                    let sub_block = to_wide_chars(r"\StringFileInfo\040904b0\FileDescription");
                    let mut lp_buffer: *mut c_void = ptr::null_mut();
                    let mut len = 0u32;
                    if VerQueryValueW(
                        buffer.as_ptr() as *const c_void,
                        PCWSTR(sub_block.as_ptr()),
                        &mut lp_buffer,
                        &mut len,
                    )
                    .as_bool()
                        && !lp_buffer.is_null()
                        && len > 0
                    {
                        let slice =
                            std::slice::from_raw_parts(lp_buffer as *const u16, len as usize);
                        let s = String::from_utf16_lossy(slice)
                            .trim_matches('\0')
                            .trim()
                            .to_string();
                        if !s.is_empty() {
                            friendly = s;
                        }
                    }
                }
            }
        }
    }

    if friendly.is_empty() {
        let lower = exe_name.to_lowercase().replace(".exe", "");
        let common = get_common_app_names();
        if let Some(&name) = common.get(lower.as_str()) {
            friendly = name.to_string();
        } else {
            // Capitalize first letter
            let mut chars = lower.chars();
            friendly = match chars.next() {
                None => exe_name.clone(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            };
        }
    }

    name_cache.insert(exe_name.clone(), friendly.clone());
    (friendly, exe_name)
}

pub fn kill_process_by_pid(pid: u32, exe_name: &str) -> (bool, String) {
    let lower_exe = exe_name.to_lowercase();
    for &crit in CRITICAL_PROCESSES {
        if lower_exe == crit || lower_exe.starts_with(crit) {
            return (
                false,
                format!(
                    "Cannot terminate {}: Critical Windows system process.",
                    exe_name
                ),
            );
        }
    }

    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid);
        if let Ok(h) = handle {
            if !h.is_invalid() {
                let res = TerminateProcess(h, 1);
                let _ = CloseHandle(h);
                if res.is_ok() {
                    return (true, format!("Successfully stopped PID {}.", pid));
                }
            }
        }
    }

    // Fallback: taskkill /F /PID
    let output = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                (true, format!("Force-killed PID {}.", pid))
            } else {
                (
                    false,
                    "Access denied. Try running as Administrator.".to_string(),
                )
            }
        }
        Err(e) => (false, format!("Failed to execute taskkill: {}", e)),
    }
}

pub fn list_all_pids_for_names(
    target_names: &[String],
    target_exes: &[String],
) -> Vec<(u32, String)> {
    let mut matches = Vec::new();
    let lower_targets: Vec<String> = target_names.iter().map(|s| s.to_lowercase()).collect();
    let lower_exes: Vec<String> = target_exes.iter().map(|s| s.to_lowercase()).collect();

    unsafe {
        if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W {
                dwSize: size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    let len = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe = String::from_utf16_lossy(&entry.szExeFile[..len]);
                    let lower_exe = exe.to_lowercase();

                    let is_match = lower_exes.contains(&lower_exe)
                        || lower_targets
                            .iter()
                            .any(|t| lower_exe.contains(t) || t.contains(&lower_exe));

                    if is_match && !CRITICAL_PROCESSES.contains(&lower_exe.as_str()) {
                        matches.push((entry.th32ProcessID, exe));
                    }

                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
        }
    }

    matches
}
