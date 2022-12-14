use core::ffi::{c_char, c_int};
use std::cell::UnsafeCell;
use std::ffi::{c_void, CStr};
use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, Arc};

use esp_idf_hal::serial::Uart;
use esp_idf_svc::nvs::EspDefaultNvs;
use esp_idf_sys::{
    ble_gap_adv_params, ble_gap_adv_set_fields, ble_gap_adv_start, ble_gap_conn_desc,
    ble_gap_conn_find, ble_gap_event, ble_gatt_access_ctxt, ble_gatt_chr_def,
    ble_gatt_register_ctxt, ble_gatt_svc_def, ble_gattc_notify_custom, ble_gatts_add_svcs,
    ble_gatts_count_cfg, ble_hs_adv_fields, ble_hs_cfg, ble_hs_id_copy_addr, ble_hs_id_infer_auto,
    ble_hs_mbuf_from_flat, ble_hs_mbuf_to_flat, ble_hs_util_ensure_addr, ble_store_util_status_rr,
    ble_uuid128_t, ble_uuid_t, ble_uuid_to_str, esp, esp_nimble_hci_and_controller_init,
    nimble_port_freertos_deinit, nimble_port_freertos_init, nimble_port_init, nimble_port_run,
    os_mbuf, strlen, uart_config_t, uart_driver_install, uart_event_t,
    uart_hw_flowcontrol_t_UART_HW_FLOWCTRL_RTS, uart_param_config,
    uart_parity_t_UART_PARITY_DISABLE, uart_set_pin, uart_stop_bits_t_UART_STOP_BITS_1,
    uart_word_length_t_UART_DATA_8_BITS, xQueueReceive, QueueHandle_t, TickType_t,
    BLE_GAP_CONN_MODE_UND, BLE_GAP_DISC_MODE_GEN, BLE_GAP_EVENT_ADV_COMPLETE,
    BLE_GAP_EVENT_CONNECT, BLE_GAP_EVENT_CONN_UPDATE, BLE_GAP_EVENT_DISCONNECT, BLE_GAP_EVENT_MTU,
    BLE_GATT_ACCESS_OP_READ_CHR, BLE_GATT_ACCESS_OP_WRITE_CHR, BLE_GATT_CHR_F_NOTIFY,
    BLE_GATT_CHR_F_READ, BLE_GATT_CHR_F_WRITE, BLE_GATT_REGISTER_OP_CHR, BLE_GATT_REGISTER_OP_DSC,
    BLE_GATT_REGISTER_OP_SVC, BLE_GATT_SVC_TYPE_PRIMARY, BLE_HS_ADV_F_BREDR_UNSUP,
    BLE_HS_ADV_F_DISC_GEN, BLE_HS_ADV_TX_PWR_LVL_AUTO, BLE_UUID_STR_LEN, BLE_UUID_TYPE_128,
    CONFIG_BT_NIMBLE_MAX_CONNECTIONS, UART_PIN_NO_CHANGE,
};
use once_cell::sync::Lazy;
use tracing::{error, info};

pub static CURRENT_MESSAGE: Lazy<Mutex<String>> = Lazy::new(|| {
    let msg = if let Ok(msg) = std::fs::read_to_string("/message.txt") {
        msg
    } else {
        "Hello World".to_owned()
    };

    Mutex::new(msg)
});
pub static MESSAGE_UPDATED: AtomicBool = AtomicBool::new(false);

const BLE_UUID_TYPE_128_: ble_uuid_t = ble_uuid_t {
    type_: BLE_UUID_TYPE_128 as u8,
};

const fn inv(
    [a_0, a_1, a_2, a_3, b_0, b_1, b_2, b_3, c_0, c_1, c_2, c_3, d_0, d_1, d_2, d_3]: [u8; 16],
) -> [u8; 16] {
    [
        d_3, d_2, d_1, d_0, c_3, c_2, c_1, c_0, b_3, b_2, b_1, b_0, a_3, a_2, a_1, a_0,
    ]
}

static mut BLE_LE_NRF_SERVICE: ble_uuid128_t = ble_uuid128_t {
    u: BLE_UUID_TYPE_128_,
    value: inv(*uuid::uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e").as_bytes()),
};

static mut BLE_LE_NRF_CHAR_RW2: ble_uuid128_t = ble_uuid128_t {
    u: BLE_UUID_TYPE_128_,
    value: inv(*uuid::uuid!("6e400002-b5a3-f393-e0a9-e50e24dcca9e").as_bytes()),
};

static mut BLE_LE_NRF_CHAR_RW3: ble_uuid128_t = ble_uuid128_t {
    u: BLE_UUID_TYPE_128_,
    value: inv(*uuid::uuid!("6e400003-b5a3-f393-e0a9-e50e24dcca9e").as_bytes()),
};

static mut BLE_LE_NRF_CHAR_RX_HANDLE: UnsafeCell<u16> = UnsafeCell::new(0);
static mut BLE_LE_NRF_CHAR_TX_HANDLE: UnsafeCell<u16> = UnsafeCell::new(0);

static mut GATT_SERVICES: [esp_idf_sys::ble_gatt_svc_def; 2] = unsafe {
    [
        ble_gatt_svc_def {
            type_: BLE_GATT_SVC_TYPE_PRIMARY as u8,
            uuid: &BLE_LE_NRF_SERVICE.u,
            characteristics: &[
                ble_gatt_chr_def {
                    uuid: &BLE_LE_NRF_CHAR_RW2.u,
                    access_cb: Some(ble_svc_gatt_handler),
                    val_handle: BLE_LE_NRF_CHAR_TX_HANDLE.get(),
                    flags: BLE_GATT_CHR_F_WRITE as u16,
                    arg: std::ptr::null_mut(),
                    descriptors: std::ptr::null_mut(),
                    min_key_size: 0,
                },
                ble_gatt_chr_def {
                    uuid: &BLE_LE_NRF_CHAR_RW3.u,
                    access_cb: Some(ble_svc_gatt_handler),
                    val_handle: BLE_LE_NRF_CHAR_RX_HANDLE.get(),
                    flags: (BLE_GATT_CHR_F_READ | BLE_GATT_CHR_F_NOTIFY) as u16,
                    arg: std::ptr::null_mut(),
                    descriptors: std::ptr::null_mut(),
                    min_key_size: 0,
                },
                const_zero::const_zero!(ble_gatt_chr_def),
            ] as *const _,
            includes: std::ptr::null_mut(),
        },
        const_zero::const_zero!(ble_gatt_svc_def),
    ]
};

unsafe extern "C" fn ble_spp_server_on_reset(reason: c_int) {
    info!(reason, "Resetting ble");
}

static mut OWN_ADDR_TYPE: u8 = 0;

unsafe extern "C" fn ble_spp_server_on_sync() {
    let rc = ble_hs_util_ensure_addr(0);
    assert_eq!(rc, 0, "ble_hs_util_ensure_addr");

    let rc = ble_hs_id_infer_auto(0, &mut OWN_ADDR_TYPE as *mut _);
    if rc != 0 {
        error!(rc, "Failed to determine address type");
        return;
    }

    let mut addr_val = [0u8; 6];
    ble_hs_id_copy_addr(OWN_ADDR_TYPE, &mut addr_val as *mut _, std::ptr::null_mut());

    info!(device_address = ?addr_val, "Found device address");

    ble_spp_server_advertise();
}

unsafe extern "C" fn ble_spp_server_host_task(_param: *mut c_void) {
    info!("BLE host task started");

    nimble_port_run();
    nimble_port_freertos_deinit();
}

unsafe extern "C" fn ble_svc_gatt_handler(
    conn_handle: u16,
    attr_handle: u16,
    ctxt: *mut ble_gatt_access_ctxt,
    _arg: *mut c_void,
) -> i32 {
    let ctxt_ = *ctxt;
    match ctxt_.op as u32 {
        BLE_GATT_ACCESS_OP_READ_CHR => {
            info!("Callback for read");
        }
        BLE_GATT_ACCESS_OP_WRITE_CHR => {
            let len = (*ctxt_.om).om_len;

            info!(
                conn_handle,
                attr_handle, len, "Data received in write event"
            );

            let mut buf = [0u8; 256];
            let mut out_len = 0u16;
            let rc = ble_hs_mbuf_to_flat(
                ctxt_.om,
                &mut buf as *mut u8 as *mut _,
                std::mem::size_of::<[u8; 256]>() as u16,
                &mut out_len as *mut _,
            );
            if rc != 0 {
                error!("Couldn't fetch mbuf in write handler");
                return 0;
            }

            let s = match std::str::from_utf8(&buf[..out_len as usize]) {
                Ok(s) => s,
                Err(e) => {
                    error!(?e, "Failed decoding string as utf8");
                    return 0;
                }
            };

            CURRENT_MESSAGE.lock().unwrap().replace_range(.., s);
            MESSAGE_UPDATED.store(true, std::sync::atomic::Ordering::SeqCst);
            info!(s, "Updated message");
        }
        _ => {}
    }

    0
}

unsafe fn ble_spp_server_advertise() {
    let name = ble_svc_gap_device_name();
    let mut fields = ble_hs_adv_fields {
        flags: (BLE_HS_ADV_F_DISC_GEN | BLE_HS_ADV_F_BREDR_UNSUP) as u8,
        tx_pwr_lvl: BLE_HS_ADV_TX_PWR_LVL_AUTO as i8,
        name: name as *const _,
        name_len: strlen(name) as u8,
        adv_itvl: 1000,
        ..Default::default()
    };

    fields.set_tx_pwr_lvl_is_present(1);
    fields.set_name_is_complete(1);
    fields.set_adv_itvl_is_present(1);

    let rc = ble_gap_adv_set_fields(&fields);
    if rc != 0 {
        error!(rc, "error setting advertisement data");
        return;
    }

    let adv_params = ble_gap_adv_params {
        conn_mode: BLE_GAP_CONN_MODE_UND as u8,
        disc_mode: BLE_GAP_DISC_MODE_GEN as u8,
        ..Default::default()
    };

    const BLE_HS_FOREVER: i32 = 2147483647;
    let rc = ble_gap_adv_start(
        OWN_ADDR_TYPE,
        std::ptr::null(),
        BLE_HS_FOREVER,
        &adv_params,
        Some(ble_spp_server_gap_event),
        std::ptr::null_mut(),
    );
    if rc != 0 {
        error!(rc, "error enabling advertisement");
    }
}

unsafe extern "C" fn ble_spp_server_gap_event(event: *mut ble_gap_event, _arg: *mut c_void) -> i32 {
    let event_ = *event;
    let mut desc = ble_gap_conn_desc::default();

    match event_.type_ as u32 {
        BLE_GAP_EVENT_CONNECT => {
            let connect = event_.__bindgen_anon_1.connect;
            info!(
                status = connect.status,
                "connection {}",
                if connect.status == 0 {
                    "established"
                } else {
                    "failed"
                }
            );

            if connect.status == 0 {
                let rc = ble_gap_conn_find(connect.conn_handle, &mut desc as *mut _);
                assert_eq!(rc, 0, "ble_gap_conn_find");
                info!(handle = connect.conn_handle, ?desc, "Conn desc");
                CONNECTION_HANDLES[connect.conn_handle as usize] = connect.conn_handle;
            }

            if connect.status != 0 || CONFIG_BT_NIMBLE_MAX_CONNECTIONS > 1 {
                ble_spp_server_advertise();
            }
        }

        BLE_GAP_EVENT_DISCONNECT => {
            let disconnect = event_.__bindgen_anon_1.disconnect;
            info!(reason = disconnect.reason, "Disconnect");

            ble_spp_server_advertise();
        }

        BLE_GAP_EVENT_CONN_UPDATE => {
            let conn_update = event_.__bindgen_anon_1.conn_update;
            info!(status = conn_update.status, "Connection update");
            let rc = ble_gap_conn_find(conn_update.conn_handle, &mut desc as *mut _);
            assert_eq!(rc, 0, "ble_gap_conn_find");
            info!(?desc, "Conn desc");
        }

        BLE_GAP_EVENT_ADV_COMPLETE => {
            let adv_complete = event_.__bindgen_anon_1.adv_complete;
            info!(reason = adv_complete.reason, "advertise complete");
            ble_spp_server_advertise();
        }

        BLE_GAP_EVENT_MTU => {
            let mtu = event_.__bindgen_anon_1.mtu;
            info!(
                conn_handle = mtu.conn_handle,
                channel_id = mtu.channel_id,
                value = mtu.value,
                "mtu update"
            );
        }

        _ => {}
    }

    0
}
// sepples moment

unsafe extern "C" fn gatt_svr_register_cb(ctxt: *mut ble_gatt_register_ctxt, _arg: *mut c_void) {
    let mut buf = [0 as c_char; BLE_UUID_STR_LEN as usize];
    let ctxt_ = *ctxt;

    match ctxt_.op as u32 {
        BLE_GATT_REGISTER_OP_SVC => {
            let svc = ctxt_.__bindgen_anon_1.svc;
            let uuid = CStr::from_ptr(ble_uuid_to_str((*svc.svc_def).uuid, &mut buf as *mut _));
            info!(uuid = ?uuid, handle = ?svc.handle, "Registering gatt service");
        }
        BLE_GATT_REGISTER_OP_CHR => {
            let chr = ctxt_.__bindgen_anon_1.chr;
            let uuid = CStr::from_ptr(ble_uuid_to_str((*chr.chr_def).uuid, &mut buf as *mut _));
            info!(uuid = ?uuid, def_handle = ?chr.def_handle, val_handle = ?chr.val_handle, "Registering characteristic");
        }
        BLE_GATT_REGISTER_OP_DSC => {
            let dsc = ctxt_.__bindgen_anon_1.dsc;
            let uuid = CStr::from_ptr(ble_uuid_to_str((*dsc.dsc_def).uuid, &mut buf as *mut _));
            info!(uuid = ?uuid, handle = ?dsc.handle, "Registering gatt service");
        }
        _ => {}
    }
}

static mut SPP_COMMON_QUEUE_HANDLE: QueueHandle_t = std::ptr::null_mut();
static mut CONNECTION_HANDLES: [u16; CONFIG_BT_NIMBLE_MAX_CONNECTIONS as usize] =
    [0; CONFIG_BT_NIMBLE_MAX_CONNECTIONS as usize];

fn ble_uart_task<U: Uart>(_uart: U) {
    info!("Starting ble uart task");

    let mut event: uart_event_t = uart_event_t::default();

    loop {
        // yummy
        if unsafe {
            xQueueReceive(
                SPP_COMMON_QUEUE_HANDLE,
                &mut event as *mut uart_event_t as *mut _,
                TickType_t::MAX,
            )
        } == 1
            && event.type_ == esp_idf_sys::uart_event_type_t_UART_DATA
            && event.size > 0
        {
            static mut NTF: [u8; 1] = [0];
            unsafe {
                NTF[0] = 90;
            }

            for &handle in unsafe { &CONNECTION_HANDLES } {
                if handle == 0 {
                    continue;
                }

                let txom: *mut os_mbuf = unsafe {
                    ble_hs_mbuf_from_flat(
                        &NTF as *const u8 as *const _,
                        std::mem::size_of::<[u8; 1]>() as u16,
                    )
                };

                let rc = unsafe {
                    ble_gattc_notify_custom(handle, *BLE_LE_NRF_CHAR_RX_HANDLE.get_mut(), txom)
                };

                if rc == 0 {
                    info!("Notification sent");
                } else {
                    error!(rc, "Erorr sending notif");
                }
            }
        }
    }
}

unsafe fn ble_uart_init<U: Uart + Send + 'static>(uart: U) -> color_eyre::Result<()> {
    let uart_config = uart_config_t {
        baud_rate: 115200,
        data_bits: uart_word_length_t_UART_DATA_8_BITS,
        parity: uart_parity_t_UART_PARITY_DISABLE,
        stop_bits: uart_stop_bits_t_UART_STOP_BITS_1,
        flow_ctrl: uart_hw_flowcontrol_t_UART_HW_FLOWCTRL_RTS,
        rx_flow_ctrl_thresh: 122,
        ..uart_config_t::default()
    };

    esp!(uart_param_config(U::port(), &uart_config))?;
    esp!(uart_set_pin(
        U::port(),
        UART_PIN_NO_CHANGE,
        UART_PIN_NO_CHANGE,
        UART_PIN_NO_CHANGE,
        UART_PIN_NO_CHANGE
    ))?;

    esp!(uart_driver_install(
        U::port(),
        4096,
        8192,
        10,
        &mut SPP_COMMON_QUEUE_HANDLE,
        0
    ))?;

    std::thread::spawn(move || ble_uart_task(uart));

    Ok(())
}

pub fn init_ble<U: Uart + Send + 'static>(uart: U, _nvs: Arc<EspDefaultNvs>) -> color_eyre::Result<()> {
    unsafe {
        info!("Initializing bluetooth");

        esp!(esp_nimble_hci_and_controller_init())?;

        nimble_port_init();

        ble_uart_init(uart)?;

        ble_hs_cfg.reset_cb = Some(ble_spp_server_on_reset);
        ble_hs_cfg.sync_cb = Some(ble_spp_server_on_sync);
        ble_hs_cfg.gatts_register_cb = Some(gatt_svr_register_cb);
        ble_hs_cfg.store_status_cb = Some(ble_store_util_status_rr);
        ble_hs_cfg.sm_io_cap = 3;
        ble_hs_cfg.set_sm_bonding(1);
        ble_hs_cfg.set_sm_sc(1);
        ble_hs_cfg.sm_our_key_dist = 1;
        ble_hs_cfg.sm_their_key_dist = 1;

        // gatt_svr_init
        ble_svc_gap_init();
        ble_svc_gatt_init();
        esp!(ble_gatts_count_cfg(&GATT_SERVICES as *const _))?;
        esp!(ble_gatts_add_svcs(&GATT_SERVICES as *const _))?;
        esp!(ble_svc_gap_device_name_set(
            b"D21 Scrolling Text\0" as *const u8 as *const _
        ))?;
        ble_store_config_init();
        nimble_port_freertos_init(Some(ble_spp_server_host_task));
    }

    Ok(())
}

extern "C" {
    fn ble_svc_gap_init();
    fn ble_svc_gatt_init();
    fn ble_svc_gap_device_name() -> *const c_char;
    fn ble_svc_gap_device_name_set(name: *const c_char) -> c_int;
    fn ble_store_config_init();
}
