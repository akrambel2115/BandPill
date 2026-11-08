use bandpill::net::adapter::is_relevant_adapter;

#[test]
fn test_loopback_filtered_in_all_modes() {
    assert!(!is_relevant_adapter(
        "Loopback Pseudo-Interface 1",
        "Software Loopback Interface 1",
        "physical"
    ));
    assert!(!is_relevant_adapter(
        "Loopback Pseudo-Interface 1",
        "Software Loopback Interface 1",
        "all"
    ));
}

#[test]
fn test_virtual_adapters_filtered_in_physical_mode() {
    assert!(!is_relevant_adapter(
        "vEthernet (WSL)",
        "Hyper-V Virtual Ethernet Adapter",
        "physical"
    ));
    assert!(!is_relevant_adapter(
        "DockerNAT",
        "Hyper-V Virtual Ethernet Adapter",
        "physical"
    ));
    assert!(!is_relevant_adapter(
        "Tailscale",
        "Tailscale Tunnel",
        "physical"
    ));
    assert!(!is_relevant_adapter(
        "VMware Network Adapter VMnet1",
        "VMware Virtual Ethernet Adapter",
        "physical"
    ));
    assert!(!is_relevant_adapter(
        "WireGuard Tunnel",
        "WireGuard Virtual Network Adapter",
        "physical"
    ));
}

#[test]
fn test_physical_adapters_allowed() {
    assert!(is_relevant_adapter(
        "Wi-Fi",
        "Intel(R) Wi-Fi 6 AX201 160MHz",
        "physical"
    ));
    assert!(is_relevant_adapter(
        "Ethernet",
        "Realtek Gaming 2.5GbE Family Controller",
        "physical"
    ));
    assert!(is_relevant_adapter(
        "Cellular",
        "Qualcomm Snapdragon X55 5G Modem",
        "physical"
    ));
}

#[test]
fn test_virtual_adapters_allowed_in_all_mode() {
    // Virtual adapters (other than loopback/pseudo) are permitted when track_mode is "all"
    assert!(is_relevant_adapter(
        "vEthernet (WSL)",
        "Hyper-V Virtual Ethernet Adapter",
        "all"
    ));
    assert!(is_relevant_adapter(
        "Docker0",
        "Docker Virtual Interface",
        "all"
    ));
}
