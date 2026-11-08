use std::process::Command;
use std::ptr;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ,
};
use windows::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceConfigW, QueryServiceStatus,
    QUERY_SERVICE_CONFIGW, SC_MANAGER_CONNECT, SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS,
    SERVICE_RUNNING, SERVICE_STATUS,
};

use crate::utils::{open_url_or_command, run_elevated_powershell, to_wide_chars};

static METERED_CACHE: Mutex<Option<(bool, String, String, Instant)>> = Mutex::new(None);

pub struct DataSaverManager;

impl DataSaverManager {
    // -------------------------------------------------------------
    // Step 1: Delivery Optimization (P2P Windows update uploads)
    // -------------------------------------------------------------
    pub fn get_delivery_optimization_status() -> (bool, String) {
        // 1. Check Policy Key in HKLM
        let subkey = to_wide_chars(r"SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization");
        let val_name = to_wide_chars("DODownloadMode");

        unsafe {
            let mut hkey = windows::Win32::System::Registry::HKEY(ptr::null_mut());
            if RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(subkey.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            )
            .is_ok()
            {
                let mut mode = 0u32;
                let mut len = std::mem::size_of::<u32>() as u32;

                let res = RegQueryValueExW(
                    hkey,
                    PCWSTR(val_name.as_ptr()),
                    None,
                    None,
                    Some(&mut mode as *mut u32 as *mut u8),
                    Some(&mut len),
                );
                let _ = RegCloseKey(hkey);

                if res.is_ok() {
                    if mode == 0 || mode == 99 {
                        return (true, "Blocked (P2P Off)".to_string());
                    } else if mode == 1 {
                        return (false, "LAN Only".to_string());
                    } else {
                        return (false, format!("Uploading Active (Mode {})", mode));
                    }
                }
            }
        }

        // 2. Check DoSvc service state
        let (is_running, start_type) = query_win_service("DoSvc");
        if !is_running || start_type == 4
        /* Disabled */
        {
            return (true, "Service Inactive".to_string());
        }

        (false, "Active (P2P Uploads On)".to_string())
    }

    pub fn disable_delivery_optimization() -> bool {
        let script = r#"
            New-Item -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization" -Force -ErrorAction SilentlyContinue | Out-Null;
            Set-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization" -Name "DODownloadMode" -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue;
            New-Item -Path "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization" -Force -ErrorAction SilentlyContinue | Out-Null;
            Set-ItemProperty -Path "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\DeliveryOptimization" -Name "DODownloadMode" -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue;
            Stop-Service -Name DoSvc -Force -ErrorAction SilentlyContinue;
            Set-Service -Name DoSvc -StartupType Manual -ErrorAction SilentlyContinue;
        "#;
        run_elevated_powershell(script)
    }

    pub fn open_delivery_optimization_settings() {
        open_url_or_command("ms-settings:delivery-optimization");
    }

    // -------------------------------------------------------------
    // Step 2: Metered Connection (stops background OS updates & telemetry)
    // -------------------------------------------------------------
    pub fn get_metered_status(force_refresh: bool) -> (bool, String, String) {
        if !force_refresh {
            if let Ok(guard) = METERED_CACHE.lock() {
                if let Some((is_metered, profile, cost, time)) = guard.as_ref() {
                    if time.elapsed() < Duration::from_secs(15) {
                        return (*is_metered, profile.clone(), cost.clone());
                    }
                }
            }
        }

        let script = r#"
            [Windows.Networking.Connectivity.NetworkInformation, Windows.Networking.Connectivity, ContentType=WindowsRuntime] | Out-Null;
            $p = [Windows.Networking.Connectivity.NetworkInformation]::GetInternetConnectionProfile();
            if ($p) {
                $c = $p.GetConnectionCost();
                Write-Output "PROFILE:$($p.ProfileName)";
                Write-Output "COST:$($c.NetworkCostType)";
            }
        "#;

        let output = Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", script])
            .output();

        let mut profile_name = "Active Connection".to_string();
        let mut cost_type = "Unrestricted".to_string();
        let mut is_metered = false;

        if let Ok(out) = output {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("PROFILE:") {
                    profile_name = rest.to_string();
                } else if let Some(rest) = trimmed.strip_prefix("COST:") {
                    cost_type = rest.to_string();
                    let lower = cost_type.to_lowercase();
                    if lower == "fixed" || lower == "variable" || lower == "2" || lower == "3" {
                        is_metered = true;
                    }
                }
            }
        }

        if let Ok(mut guard) = METERED_CACHE.lock() {
            *guard = Some((
                is_metered,
                profile_name.clone(),
                cost_type.clone(),
                Instant::now(),
            ));
        }

        (is_metered, profile_name, cost_type)
    }

    pub fn toggle_metered_connection(enable: bool) -> bool {
        let (_, profile_name, _) = Self::get_metered_status(false);
        let cost_val = if enable { "Fixed" } else { "Unrestricted" };

        if !profile_name.is_empty() && profile_name != "Active Connection" {
            let _ = Command::new("netsh")
                .args([
                    "wlan",
                    "set",
                    "profileparameter",
                    &format!("name={}", profile_name),
                    &format!("cost={}", cost_val),
                ])
                .output();
        }

        let media_val = if enable { 2 } else { 1 };
        let script = format!(
            r#"
            $path = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\NetworkList\DefaultMediaCost";
            Set-ItemProperty -Path $path -Name "WiFi" -Value {} -Type DWord -ErrorAction SilentlyContinue;
            Set-ItemProperty -Path $path -Name "Ethernet" -Value {} -Type DWord -ErrorAction SilentlyContinue;
            Set-ItemProperty -Path $path -Name "Default" -Value {} -Type DWord -ErrorAction SilentlyContinue;
            "#,
            media_val, media_val, media_val
        );

        let res = run_elevated_powershell(&script);
        if let Ok(mut guard) = METERED_CACHE.lock() {
            *guard = None;
        }
        res
    }

    pub fn open_network_settings() {
        open_url_or_command("ms-settings:network");
    }

    // -------------------------------------------------------------
    // Step 3: BITS Service (Background Intelligent Transfer Service)
    // -------------------------------------------------------------
    pub fn get_bits_status() -> (bool, String, String) {
        let (is_running, start_type) = query_win_service("BITS");
        let start_type_str = match start_type {
            2 => "Automatic",
            3 => "Manual",
            4 => "Disabled",
            _ => "Unknown",
        };

        let is_restricted = (start_type == 3 || start_type == 4) && !is_running;
        let text = if is_restricted {
            format!("Stopped ({})", start_type_str)
        } else if is_running {
            format!("Running ({})", start_type_str)
        } else {
            format!("Stopped ({})", start_type_str)
        };

        (is_restricted, text, start_type_str.to_lowercase())
    }

    pub fn kill_and_restrict_bits() -> bool {
        let script = r#"
            Stop-Service -Name BITS -Force -ErrorAction SilentlyContinue;
            Set-Service -Name BITS -StartupType Manual -ErrorAction SilentlyContinue;
            sc.exe config BITS start=demand;
            net stop BITS /y;
        "#;
        run_elevated_powershell(script)
    }

    pub fn open_services_msc() {
        open_url_or_command("services.msc");
    }

    // -------------------------------------------------------------
    // Master Action: Apply All 3 Optimizations in One Click
    // -------------------------------------------------------------
    pub fn apply_all_protections() -> bool {
        let _ = Self::toggle_metered_connection(true);

        let script = r#"
            # 1. Disable Delivery Optimization uploads
            New-Item -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization" -Force -ErrorAction SilentlyContinue | Out-Null;
            Set-ItemProperty -Path "HKLM:\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization" -Name "DODownloadMode" -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue;
            Stop-Service -Name DoSvc -Force -ErrorAction SilentlyContinue;
            Set-Service -Name DoSvc -StartupType Manual -ErrorAction SilentlyContinue;

            # 2. Kill and set BITS to Manual
            Stop-Service -Name BITS -Force -ErrorAction SilentlyContinue;
            Set-Service -Name BITS -StartupType Manual -ErrorAction SilentlyContinue;
            sc.exe config BITS start=demand;
            net stop BITS /y;

            # 3. Default Media Cost
            $path = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\NetworkList\DefaultMediaCost";
            Set-ItemProperty -Path $path -Name "WiFi" -Value 2 -Type DWord -ErrorAction SilentlyContinue;
            Set-ItemProperty -Path $path -Name "Ethernet" -Value 2 -Type DWord -ErrorAction SilentlyContinue;
        "#;

        run_elevated_powershell(script)
    }
}

fn query_win_service(service_name: &str) -> (bool, u32) {
    let wide_name = to_wide_chars(service_name);
    unsafe {
        let scm = OpenSCManagerW(PCWSTR(ptr::null()), PCWSTR(ptr::null()), SC_MANAGER_CONNECT);
        if scm.is_err() {
            return (false, 0);
        }
        let scm_handle = scm.unwrap();

        let svc = OpenServiceW(
            scm_handle,
            PCWSTR(wide_name.as_ptr()),
            SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG,
        );
        if svc.is_err() {
            let _ = CloseServiceHandle(scm_handle);
            return (false, 0);
        }
        let svc_handle = svc.unwrap();

        let mut status = SERVICE_STATUS::default();
        let running = if QueryServiceStatus(svc_handle, &mut status).is_ok() {
            status.dwCurrentState == SERVICE_RUNNING
        } else {
            false
        };

        let mut bytes_needed = 0u32;
        let _ = QueryServiceConfigW(svc_handle, None, 0, &mut bytes_needed);
        let mut start_type = 0u32;

        if bytes_needed > 0 {
            let mut buf = vec![0u8; bytes_needed as usize];
            let config_ptr = buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
            if QueryServiceConfigW(
                svc_handle,
                Some(config_ptr),
                bytes_needed,
                &mut bytes_needed,
            )
            .is_ok()
            {
                start_type = (*config_ptr).dwStartType.0;
            }
        }

        let _ = CloseServiceHandle(svc_handle);
        let _ = CloseServiceHandle(scm_handle);

        (running, start_type)
    }
}
