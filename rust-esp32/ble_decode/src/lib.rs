use aes::Aes128;
use ccm::{
    aead::{generic_array::GenericArray, Aead, KeyInit, Payload},
    consts::{U12, U4},
    Ccm,
};
use serde::Deserialize;
use std::{collections::HashMap, num::ParseIntError};

pub type Aes128Ccm = Ccm<Aes128, U4, U12>;

pub fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}

pub fn encode_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join(" ")
}

#[derive(Debug, Deserialize, Clone)]
struct Device {
    pub mac: String,
    pub key: String,
    pub room: String,
}

static DEVICES_JSON: &str = include_str!("devices.json");

pub struct Decryptor {
    devices: HashMap<String, Device>,
}

impl Decryptor {
    pub fn new() -> Self {
        let devices_list: Vec<Device> = serde_json::from_str(DEVICES_JSON).unwrap();

        let devices: HashMap<String, Device> =
            HashMap::from_iter(devices_list.iter().map(|d| (d.mac.clone(), d.clone())));

        Decryptor { devices }
    }

    pub fn decode_frame_data(&self, data: &[u8]) -> Option<f32> {
        if data.len() < 26 {
            return None;
        }

        let mac_string = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            data[17], data[16], data[15], data[14], data[13], data[12]
        );

        if let Some(device) = &self.devices.get(&mac_string) {
            let nonce: [u8; 12] = [
                data[12], data[13], data[14], data[15], data[16], data[17], // device mac
                data[9], data[10], // device type
                data[11], // frame cnt
                data[23], data[24], data[25], // ext.cnt
            ];

            let nonce: &GenericArray<u8, U12> = GenericArray::from_slice(&nonce);

            let key = decode_hex(&device.key).expect("Unable to decode key");

            let cipher = Aes128Ccm::new_from_slice(&key).unwrap();

            let encrypted_data: &[u8] = &data[18..23];
            let tag = &data[26..];

            let to_decrypt = [encrypted_data, tag].concat();

            let payload = Payload {
                msg: &to_decrypt,
                aad: &[0x11],
            };

            // println!("Nonce: {:?} ({:?})", encode_hex(&nonce), nonce.len());

            // println!(
            //     "To decrypt: {:?} ({:?})",
            //     encode_hex(&to_decrypt),
            //     to_decrypt.len()
            // );

            let plain_data = cipher.decrypt(nonce, payload);

            if let Ok(plain_data) = plain_data {
                let temp = (plain_data[4] as f32 * 16.0 + plain_data[3] as f32) / 10.0;

                if plain_data[0] == 4 {
                    println!("Je renvoie {:?}", temp);
                    return Some(temp);
                }

                println!(
                    "Decrypted: {:?} {:?} : {:?}",
                    encode_hex(&plain_data),
                    plain_data,
                    if plain_data[0] == 4 {
                        "TEMP"
                    } else {
                        "HUMIDITY"
                    },
                );

                println!(
                    "{:?} {:?} {:?} {:?}",
                    (plain_data[1] as f32 * 16.0 + plain_data[0] as f32) / 10.0,
                    (plain_data[2] as f32 * 16.0 + plain_data[1] as f32) / 10.0,
                    (plain_data[3] as f32 * 16.0 + plain_data[2] as f32) / 10.0,
                    (plain_data[4] as f32 * 16.0 + plain_data[3] as f32) / 10.0,
                );
                println!(
                    "{:?} {:?} {:?} {:?}",
                    (plain_data[0] as f32 * 16.0 + plain_data[1] as f32) / 10.0,
                    (plain_data[1] as f32 * 16.0 + plain_data[2] as f32) / 10.0,
                    (plain_data[2] as f32 * 16.0 + plain_data[3] as f32) / 10.0,
                    (plain_data[3] as f32 * 16.0 + plain_data[4] as f32) / 10.0,
                );
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn it_works() {
        /*
        26.7°C 93%

        idx 11 : increment each X frames

                  len Ad   Xiaomi Frame Device Frame  MAC (LE)
                      Type        ctrl  Type   cnt                        cypher             Ext.cnt      MAC tag
        "02 01 06 1A  16   95 FE  58 58 5B 05  5A     5C 2D 4E 38 C1 A4   F5 53 71 C4 93     7F 00 00     ED 75 FC 9D"

        */

        let frames = [
            "02 01 06 1A 16 95 FE 58 58 5B 05 0D 5C 2D 4E 38 C1 A4 71 34 2F 20 66 00 00 00 90 46 58 7B",
            "02 01 06 1A 16 95 FE 58 58 5B 05 0E 5C 2D 4E 38 C1 A4 5E AD E7 1D 88 00 00 00 A2 0F 96 67",
            "02 01 06 1A 16 95 FE 58 58 5B 05 0F 5C 2D 4E 38 C1 A4 D0 46 F4 9D CD 00 00 00 76 BE 05 74",
            "02 01 06 1A 16 95 FE 58 58 5B 05 37 5C 2D 4E 38 C1 A4 65 D4 44 43 00 00 00 00 7D 8A BB 74",
            "02 01 06 1A 16 95 FE 58 58 5B 05 38 5C 2D 4E 38 C1 A4 C7 E7 D8 37 6E 00 00 00 E6 2E 38 AC", // 23.3°C 56%
            "02 01 06 1A 16 95 FE 58 58 5B 05 39 5C 2D 4E 38 C1 A4 79 84 BE BA 5F 00 00 00 EF A6 D9 D9", // 23.3°C 56%
            "02 01 06 1A 16 95 FE 58 58 5B 05 41 5C 2D 4E 38 C1 A4 76 62 F3 1B 2F 00 00 00 EC 3B 07 A0", // 23.3°C 56%
            "02 01 06 1A 16 95 FE 58 58 5B 05 42 5C 2D 4E 38 C1 A4 96 80 70 95 4F 00 00 00 62 BE 40 48", // 23.3°C 56%
            "02 01 06 1A 16 95 FE 58 58 5B 05 43 5C 2D 4E 38 C1 A4 91 D0 C1 96 1C 00 00 00 CB 4D B2 FB", // 23.3°C 56%
            "02 01 06 1A 16 95 FE 58 58 5B 05 4B 5C 2D 4E 38 C1 A4 B4 CE F3 B0 70 00 00 00 83 BA 4C 0D", // 23.3°C 56%
            "02 01 06 1A 16 95 FE 58 58 5B 05 4C 5C 2D 4E 38 C1 A4 60 5A B5 F8 91 00 00 00 E7 AD FE 3B", // 23.3°C 56%
            "02 01 06 1A 16 95 FE 58 58 5B 05 4D 5C 2D 4E 38 C1 A4 77 E4 0D 0F FB 00 00 00 F1 00 1C CF", // 23.7°C 72%
            "02 01 06 1A 16 95 FE 58 58 5B 05 4E 5C 2D 4E 38 C1 A4 0B F1 43 48 F0 00 00 00 AA AC D6 80", // 23.6°C 64%
            "02 01 06 1A 16 95 FE 58 58 5B 05 4F 5C 2D 4E 38 C1 A4 48 86 C7 D7 A1 00 00 00 7A 54 16 8F", // 23.5°C 60%
        ];

        let decryptor = Decryptor::new();
        for frame in frames.iter() {
            let bytes = frame
                .split(" ")
                .map(|x| u8::from_str_radix(x, 16).unwrap())
                .collect::<Vec<u8>>();
            let result = decryptor.decode_frame_data(&bytes);
            dbg!(result);
            //assert!(result.is_some());
        }

        let bytes =
            decode_hex("0201061A1695FE58585B054F5C2D4E38C1A44886C7D7A10000007A54168F").unwrap();

        assert_eq!(decryptor.decode_frame_data(&bytes), Some(23.6));
    }
}
