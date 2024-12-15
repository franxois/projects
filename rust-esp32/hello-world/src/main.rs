use embedded_svc::{
    io::Write,
    utils::io,
    wifi::{AuthMethod, ClientConfiguration, Configuration},
};
use esp32_nimble::{enums::AdvType, BLEDevice, BLEScan};
use esp_idf_svc::hal::{
    prelude::Peripherals,
    rmt::{config::TransmitConfig, TxRmtDriver},
};
use esp_idf_svc::http::client::EspHttpConnection;
use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use esp_idf_svc::{
    hal::task::block_on,
    sys::{heap_caps_get_free_size, MALLOC_CAP_DEFAULT},
};
use http_server::create_http_server;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use embedded_svc::http::client::Client as HttpClient;
use log::{error, info};
pub mod base64;
mod http_server;
pub mod rgb;
pub mod rmt_neopixel;

use rgb::Rgb;
use rmt_neopixel::neopixel;
use serde::Serialize;

use ble_decode::Decryptor;

#[macro_use]
extern crate dotenv_codegen;

const SSID: &str = dotenv!("WIFI_SSID");
const PASSWORD: &str = dotenv!("WIFI_PASS");

#[derive(Serialize)]
struct BLEAdvertisedData {
    name: String,
    mac: String,
    temperature: f32,
    #[serde(with = "base64::base64")]
    payload: Vec<u8>,
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Debug);

    info!("Hello, world!");

    info!("Start WIFI test!");

    let peripherals = Peripherals::take()?;

    let led = peripherals.pins.gpio8;
    let channel = peripherals.rmt.channel0;
    let config = TransmitConfig::new().clock_divider(1);
    let mut tx = TxRmtDriver::new(channel, led, &config)?;

    neopixel(Rgb::new(5, 0, 5), &mut tx)?;

    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    connect_wifi(&mut wifi)?;

    neopixel(Rgb::new(0, 0, 10), &mut tx)?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    let _sntp = EspSntp::new_default()?;
    info!("SNTP initialized");

    let rgb_handler: Arc<Mutex<TxRmtDriver<'static>>> = Arc::new(Mutex::new(tx));
    let rgb_handler2 = rgb_handler.clone();

    let mut temp_history: HashMap<String, Vec<(i64, f32)>> = HashMap::new();

    let _http_server = create_http_server(rgb_handler)?;

    loop {
        let wifi_info = wifi.is_connected();

        match wifi_info {
            Ok(true) => info!("Wifi connected"),
            Ok(false) => {
                info!("Wifi not connected");
                let _ = connect_wifi(&mut wifi);
            }
            Err(e) => error!("Wifi error: {}", e),
        };

        block_on(run_ble_scan(&rgb_handler2, &mut temp_history));
    }

    Ok(())
}

async fn run_ble_scan(
    rgb_handler: &Arc<Mutex<TxRmtDriver<'static>>>,
    history: &mut HashMap<String, Vec<(i64, f32)>>,
) {
    info!("Start BLE scan!");

    let ble_device = BLEDevice::take();
    let mut ble_scan = BLEScan::new();
    ble_scan.active_scan(true).interval(100).window(99);

    let _ = ble_scan
        .start(
            ble_device,
            120000,
            |device: &esp32_nimble::BLEAdvertisedDevice,
             data: esp32_nimble::BLEAdvertisedData<&[u8]>| {
                // info!("Advertised Device: ({:?}, {:?})", device, data);

                let room_option = match device.addr().to_string().as_str() {
                    "A4:C1:38:4E:2D:5C" => Some("Salon"),
                    "A4:C1:38:CD:F2:86" => Some("Chambre"),
                    "A4:C1:38:D7:70:32" => Some("Bébé"),
                    _ => None,
                };

                if device.adv_type() == AdvType::Ind {
                    // info!("\\o/ Found device: {:?} {:?} {:?}", room, device, data);

                    // 0xFE95 Xiaomi Inc.
                    // First byte of payload
                    //  Asynchronous Data	0x02

                    // info!(
                    //     "mac : {:?} payload : {:?}",
                    //     device.addr(),
                    //     data.payload()
                    //         .into_iter()
                    //         .map(|x| format!("{:02X}", x))
                    //         .collect::<Vec<String>>()
                    //         .join(" ")
                    // );

                    let decryptor = Decryptor::new();

                    if let Some(temp) = decryptor.decode_frame_data(data.payload()) {
                        info!("Temperature {:?} : {}°C", room_option, temp);
                        info!("Current time: {:?}", std::time::SystemTime::now());

                        let room_name = room_option.unwrap().to_string();

                        if !history.contains_key(&room_name) {
                            history.insert(room_name.clone(), Vec::new());
                        }

                        if history.contains_key(&room_name) {
                            let mut history_entry = history.get_mut(&room_name).unwrap();
                            let unix_timestamp = std::time::SystemTime::now()
                                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                .expect("Unable to get unixtimestamp")
                                .as_secs() as i64;

                            let mut is_new_value = true;
                            let history_len = history_entry.len();

                            if history_len >= 3 {
                                if history_entry[history_len - 1].1 == temp
                                    && history_entry[history_len - 2].1 == temp
                                {
                                    info!("Temperature is the same, just update the timestamp");
                                    history_entry[history_len - 1].0 = unix_timestamp;
                                    is_new_value = false;
                                }
                            }

                            if is_new_value {
                                history_entry.push((unix_timestamp, temp));
                            }

                            info!("History size {:?}: {:#?}", room_name, history_entry.len());
                        }

                        unsafe {
                            let free_heap = heap_caps_get_free_size(MALLOC_CAP_DEFAULT);
                            info!("Free heap: {}", free_heap);
                        }

                        // let mut client =
                        //     HttpClient::wrap(EspHttpConnection::new(&Default::default()).unwrap());

                        // let mut tx = rgb_handler.lock().unwrap();
                        // neopixel(Rgb::new(0, 5, 0), &mut tx).unwrap();

                        // let send = post_request(
                        //     &mut client,
                        //     BLEAdvertisedData {
                        //         name: room_option.unwrap().to_string(),
                        //         temperature: temp,
                        //         mac: device.addr().to_string(),
                        //         payload: data.payload().to_vec(),
                        //     },
                        // );

                        // neopixel(Rgb::new(0, 0, 5), &mut tx).unwrap();

                        // if send.is_err() {
                        //     error!(
                        //         "Unable to send temperature {} : {}°C",
                        //         room_option.unwrap().to_string(),
                        //         temp
                        //     );
                        // }
                    }
                }

                None::<()>
            },
        )
        .await;

    info!("Scan end");
}

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: PASSWORD.try_into().unwrap(),
        channel: None,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started");

    // let res = wifi.scan();
    // info!("Scan result: {:?}", res);

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(())
}

fn post_request(
    client: &mut HttpClient<EspHttpConnection>,
    data: BLEAdvertisedData,
) -> anyhow::Result<()> {
    // Prepare payload
    let binding = json!(data).to_string();
    let payload = binding.as_bytes();

    // Prepare headers and URL
    let content_length_header = format!("{}", payload.len());
    let headers = [
        ("content-type", "application/json"),
        ("content-length", &*content_length_header),
    ];
    let url = "http://192.168.1.129:8080/frame";

    // Send request
    let mut request = client.post(url, &headers)?;
    request.write_all(payload)?;
    request.flush()?;
    // info!("-> POST {}", url);
    let mut response = request.submit()?;

    // Process response
    // let status = response.status();
    // info!("<- {}", status);
    let mut buf = [0u8; 1024];
    let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;
    // info!("Read {} bytes", bytes_read);
    match std::str::from_utf8(&buf[0..bytes_read]) {
        Ok(body_string) => {
            // info!(
            //     "Response body (truncated to {} bytes): {:?}",
            //     buf.len(),
            //     body_string
            // )
        }
        Err(e) => error!("Error decoding response body: {}", e),
    };

    Ok(())
}
