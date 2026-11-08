use std::ffi::c_void;
use std::ptr;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};

pub const VIRTUAL_KEYWORDS: &[&str] = &[
    "loopback",
    "pseudo",
    "wsl",
    "hyper-v",
    "vethernet",
    "virtual",
    "vmware",
    "vbox",
    "docker",
    "tailscale",
    "zerotier",
    "wireguard",
];

#[derive(Debug, Clone)]
pub struct AdapterInfo {
    pub name: String,
    pub description: String,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub is_up: bool,
}

pub fn is_relevant_adapter(name: &str, description: &str, track_mode: &str) -> bool {
    let lower_name = name.to_lowercase();
    let lower_desc = description.to_lowercase();

    if lower_name.contains("loopback")
        || lower_desc.contains("loopback")
        || lower_name.contains("pseudo")
        || lower_desc.contains("pseudo")
    {
        return false;
    }

    if track_mode == "physical" {
        for &kw in VIRTUAL_KEYWORDS {
            if lower_name.contains(kw) || lower_desc.contains(kw) {
                return false;
            }
        }
    }

    true
}

pub fn get_network_adapters() -> Vec<AdapterInfo> {
    let mut adapters = Vec::new();
    let mut table_ptr: *mut MIB_IF_TABLE2 = ptr::null_mut();

    unsafe {
        if GetIfTable2(&mut table_ptr).is_err() || table_ptr.is_null() {
            return adapters;
        }

        let num_entries = (*table_ptr).NumEntries as usize;
        let table_slice = std::slice::from_raw_parts((*table_ptr).Table.as_ptr(), num_entries);

        for row in table_slice {
            let name = wide_buf_to_string(&row.Alias);
            let description = wide_buf_to_string(&row.Description);
            let is_up = row.OperStatus.0 == 1; // 1 = IfOperStatusUp

            adapters.push(AdapterInfo {
                name,
                description,
                bytes_sent: row.OutOctets,
                bytes_recv: row.InOctets,
                is_up,
            });
        }

        FreeMibTable(table_ptr as *const c_void);
    }

    adapters
}

fn wide_buf_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
