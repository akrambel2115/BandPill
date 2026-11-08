use bandpill::ipc::IpcCommand;
use std::sync::mpsc::channel;

#[test]
fn test_ipc_command_variants() {
    let (tx, rx) = channel::<IpcCommand>();

    tx.send(IpcCommand::Wake).expect("send Wake");
    tx.send(IpcCommand::ShowTab("datasaver".to_string()))
        .expect("send ShowTab");
    tx.send(IpcCommand::TestAlert).expect("send TestAlert");
    tx.send(IpcCommand::Reset).expect("send Reset");

    assert!(matches!(rx.recv().unwrap(), IpcCommand::Wake));
    if let IpcCommand::ShowTab(tab) = rx.recv().unwrap() {
        assert_eq!(tab, "datasaver");
    } else {
        panic!("expected ShowTab");
    }
    assert!(matches!(rx.recv().unwrap(), IpcCommand::TestAlert));
    assert!(matches!(rx.recv().unwrap(), IpcCommand::Reset));
}
