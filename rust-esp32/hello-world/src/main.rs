use esp_idf_svc::http::client::EspHttpConnection;
use serde_json::json;
use std::sync::{Arc, Mutex};

use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
    utils::io,
    wifi::{AuthMethod, ClientConfiguration, Configuration},
};
use esp32_nimble::{enums::AdvType, BLEDevice, BLEScan};
use esp_idf_svc::hal::task::block_on;
use esp_idf_svc::hal::{
    prelude::Peripherals,
    rmt::{config::TransmitConfig, TxRmtDriver},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::server::EspHttpServer,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};

use embedded_svc::http::client::Client as HttpClient;
use log::{error, info};
pub mod base64;
pub mod rgb;
pub mod rmt_neopixel;

use rgb::Rgb;
use rmt_neopixel::neopixel;
use serde::{Deserialize, Serialize};

use ble_decode::Decryptor;

#[macro_use]
extern crate dotenv_codegen;

const SSID: &str = dotenv!("WIFI_SSID");
const PASSWORD: &str = dotenv!("WIFI_PASS");
static INDEX_HTML: &str = include_str!("http_server_page.html");

// Max payload length
const MAX_LEN: usize = 128;

// Need lots of stack to parse JSON
const STACK_SIZE: usize = 10240;

#[derive(Deserialize)]
struct FormData<'a> {
    first_name: &'a str,
    age: u32,
    birthplace: &'a str,
    color: &'a str,
}

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

    neopixel(Rgb::new(0, 0, 10), &mut tx)?;

    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    connect_wifi(&mut wifi)?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    info!("Wifi DHCP info: {:?}", ip_info);

    let mut server = create_server()?;

    server.fn_handler("/", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(INDEX_HTML.as_bytes())
            .map(|_| ())
    })?;

    let rgb_handler: Arc<Mutex<TxRmtDriver<'static>>> = Arc::new(Mutex::new(tx));

    server.fn_handler::<anyhow::Error, _>("/post", Method::Post, move |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;

        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;

        dbg!(String::from_utf8_lossy(&buf));

        if let Ok(form) = serde_json::from_slice::<FormData>(&buf) {
            let mut tx = rgb_handler.lock().unwrap();
            neopixel(Rgb::from_hexa(&form.color)?, &mut tx)?;

            write!(
                resp,
                "Hello, {}-year-old {} from {} ({})!",
                form.age, form.first_name, form.birthplace, form.color
            )?;
        } else {
            resp.write_all("JSON error".as_bytes())?;
        }

        Ok(())
    })?;

    // Keep wifi and the server running beyond when main() returns (forever)
    // Do not call this if you ever want to stop or access them later.
    // Otherwise you can either add an infinite loop so the main task
    // never returns, or you can move them to another thread.
    // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
    core::mem::forget(wifi);
    core::mem::forget(server);

    info!("Start BLE scan!");

    loop {
        block_on(async {
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

                            info!(
                                "mac : {:?} payload : {:?}",
                                device.addr(),
                                data.payload()
                                    .into_iter()
                                    .map(|x| format!("{:02X}", x))
                                    .collect::<Vec<String>>()
                                    .join(" ")
                            );

                            let decryptor = Decryptor::new();

                            if let Some(temp) = decryptor.decode_frame_data(data.payload()) {
                                info!("Temperature {:?} : {}°C", room_option, temp);

                                let mut client = HttpClient::wrap(
                                    EspHttpConnection::new(&Default::default()).unwrap(),
                                );
                                let _ = post_request(
                                    &mut client,
                                    BLEAdvertisedData {
                                        name: room_option.unwrap().to_string(),
                                        temperature: temp,
                                        mac: device.addr().to_string(),
                                        payload: data.payload().to_vec(),
                                    },
                                );
                            }
                        }

                        None::<()>
                    },
                )
                .await;

            info!("Scan end");
        })
    }

    Ok(())
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

    let res = wifi.scan();
    info!("Scan result: {:?}", res);

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(())
}

fn create_server() -> anyhow::Result<EspHttpServer<'static>> {
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    Ok(EspHttpServer::new(&server_configuration)?)
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
    info!("-> POST {}", url);
    let mut response = request.submit()?;

    // Process response
    let status = response.status();
    info!("<- {}", status);
    let mut buf = [0u8; 1024];
    let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;
    info!("Read {} bytes", bytes_read);
    match std::str::from_utf8(&buf[0..bytes_read]) {
        Ok(body_string) => info!(
            "Response body (truncated to {} bytes): {:?}",
            buf.len(),
            body_string
        ),
        Err(e) => error!("Error decoding response body: {}", e),
    };

    Ok(())
}
