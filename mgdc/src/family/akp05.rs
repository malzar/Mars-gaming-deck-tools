//! Familia AKP05 / Mirabox N4 (Mars Gaming MSD-PRO y otros).
//! Basado en opendeck-akp05 de ambiso, GPL-3.0:
//! https://github.com/ambiso/opendeck-akp05

use data_url::DataUrl;
use image::{DynamicImage, GenericImageView, load_from_memory_with_format};
use mirajazz::{
    device::{Device, DeviceQuery},
    error::MirajazzError,
    types::{DeviceInput, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};
use openaction::SetImageEvent;

pub const ROW_COUNT: usize = 2;
pub const COL_COUNT: usize = 5;
pub const KEY_COUNT: usize = 15;
pub const ENCODER_COUNT: usize = 4;
pub const DEVICE_TYPE: u8 = 7; // StreamDeckPlus

#[derive(Debug, Clone)]
pub enum Kind {
    Akp05E,
    Akp05EPro,
    Akp05CNPro,
    Akp05,
    N4E,
    N4,
    N4ProE,
    N4Pro,
    VsdN4,
    VsdN4Pro,
    MsdPro,
    Cn003,
    StreonorS10Pro,
    SS552,
}

pub const VSDINSIDE_VID: u16 = 0x5548;
pub const VSDINSIDE_N4_PID: u16 = 0x1004;
pub const VSD_N4_PRO_PID: u16 = 0x1023;

pub const AJAZZ_VID: u16 = 0x0300;
pub const AKP05E_PID: u16 = 0x3004;
pub const AKP05E_PRO_PID: u16 = 0x3013;
pub const AKP05_PID: u16 = 0x3006;
pub const AKP05CN_PRO_PID: u16 = 0x3014;

pub const MIRABOX_N4E_VID: u16 = 0x6603;
pub const N4E_PID: u16 = 0x1007;

pub const MIRABOX_N4_VID: u16 = 0x6602;
pub const N4_PID: u16 = 0x1001;

pub const MIRABOX_N4_PRO_E_VID: u16 = 0x5548;
pub const N4_PRO_E_PID: u16 = 0x1021;

pub const MIRABOX_N4_PRO_VID: u16 = 0x5548;
pub const N4_PRO_PID: u16 = 0x1008;

pub const MARS_GAMING_VID: u16 = 0x0B00;
pub const MSD_PRO_PID: u16 = 0x1003;

pub const SOOMFON_VID: u16 = 0x1500;
pub const CN003_PID: u16 = 0x3002;
pub const STREONOR_S10_PRO_PID: u16 = 0x3007;

pub const SS552_VID: u16 = 0x0200;
pub const SS552_PID: u16 = 0x3001;

// Map all queries to usage page 65440 and usage id 1 for now
pub const AKP05E_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP05E_PID);
pub const AKP05E_PRO_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP05E_PRO_PID);
pub const AKP05CN_PRO_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP05CN_PRO_PID);
pub const AKP05_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, AJAZZ_VID, AKP05_PID);
pub const N4E_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_N4E_VID, N4E_PID);
pub const N4_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_N4_VID, N4_PID);
pub const N4_PRO_E_QUERY: DeviceQuery =
    DeviceQuery::new(65440, 1, MIRABOX_N4_PRO_E_VID, N4_PRO_E_PID);
pub const N4_PRO_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MIRABOX_N4_PRO_VID, N4_PRO_PID);
pub const VSD_N4_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, VSDINSIDE_VID, VSDINSIDE_N4_PID);
pub const VSD_N4_PRO_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, VSDINSIDE_VID, VSD_N4_PRO_PID);
pub const MSD_PRO_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, MARS_GAMING_VID, MSD_PRO_PID);
pub const CN003_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, SOOMFON_VID, CN003_PID);
pub const STREONOR_S10_PRO: DeviceQuery =
    DeviceQuery::new(65440, 1, SOOMFON_VID, STREONOR_S10_PRO_PID);
pub const SS552_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, SS552_VID, SS552_PID);

pub const QUERIES: &[DeviceQuery] = &[
    AKP05E_QUERY,
    AKP05E_PRO_QUERY,
    AKP05CN_PRO_QUERY,
    AKP05_QUERY,
    N4E_QUERY,
    N4_QUERY,
    N4_PRO_E_QUERY,
    N4_PRO_QUERY,
    VSD_N4_QUERY,
    VSD_N4_PRO_QUERY,
    MSD_PRO_QUERY,
    CN003_QUERY,
    STREONOR_S10_PRO,
    SS552_QUERY,
];

impl Kind {
    /// Matches devices VID+PID pairs to correct kinds
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        const { assert!(MIRABOX_N4_PRO_VID == VSDINSIDE_VID) };
        const { assert!(MIRABOX_N4_PRO_VID == MIRABOX_N4_PRO_E_VID) };
        match vid {
            AJAZZ_VID => match pid {
                AKP05E_PID => Some(Kind::Akp05E),
                AKP05E_PRO_PID => Some(Kind::Akp05EPro),
                AKP05CN_PRO_PID => Some(Kind::Akp05CNPro),
                AKP05_PID => Some(Kind::Akp05),
                _ => None,
            },

            MIRABOX_N4E_VID => match pid {
                N4E_PID => Some(Kind::N4E),
                _ => None,
            },

            MIRABOX_N4_VID => match pid {
                N4_PID => Some(Kind::N4),
                _ => None,
            },

            MIRABOX_N4_PRO_VID => match pid {
                VSD_N4_PRO_PID => Some(Kind::VsdN4Pro),
                VSDINSIDE_N4_PID => Some(Kind::VsdN4),
                N4_PRO_PID => Some(Kind::N4Pro),
                N4_PRO_E_PID => Some(Kind::N4ProE),
                _ => None,
            },

            MARS_GAMING_VID => match pid {
                MSD_PRO_PID => Some(Kind::MsdPro),
                _ => None,
            },

            SOOMFON_VID => match pid {
                CN003_PID => Some(Kind::Cn003),
                STREONOR_S10_PRO_PID => Some(Kind::StreonorS10Pro),
                _ => None,
            },
            SS552_VID => match pid {
                SS552_PID => Some(Kind::SS552),
                _ => None,
            },

            _ => None,
        }
    }

    /// There is no point relying on manufacturer/device names reported by the USB stack,
    /// so we return custom names for all the kinds of devices
    pub fn human_name(&self) -> String {
        match &self {
            // Ajazz devices
            Self::Akp05E => "Ajazz AKP05E",
            Self::Akp05EPro => "Ajazz AKP05E Pro",
            Self::Akp05CNPro => "Ajazz AKP05CN Pro",
            Self::Akp05 => "Ajazz AKP05",
            // Mirabox devices
            Self::N4 => "Mirabox N4",
            Self::N4E => "Mirabox N4E",
            Self::N4ProE => "Mirabox N4 Pro E",
            Self::N4Pro => "Mirabox N4 Pro",
            // VSDInside devices
            Self::VsdN4 => "VSDInside N4",
            Self::VsdN4Pro => "VSDInside N4 Pro",
            // Mars Gaming devices
            Self::MsdPro => "Mars Gaming MSD-Pro",
            // Soomfon devices
            Self::Cn003 => "Soomfon CN003",
            Self::StreonorS10Pro => "Streonor S10 Pro",
            // Redragon Devices
            Self::SS552 => "Redragon SS552",
        }
        .to_string()
    }

    /// Returns protocol version for device
    pub fn protocol_version(&self) -> usize {
        #[allow(clippy::match_single_binding)]
        match self {
            _ => 3,
        }
    }

    pub fn image_format(&self) -> ImageFormat {
        if self.protocol_version() == 3 {
            ImageFormat {
                mode: ImageMode::JPEG,
                size: (112, 112),
                rotation: ImageRotation::Rot180,
                mirror: ImageMirroring::None,
            }
        } else {
            ImageFormat {
                mode: ImageMode::JPEG,
                size: (60, 60),
                rotation: ImageRotation::Rot0,
                mirror: ImageMirroring::None,
            }
        }
    }
    pub fn touch_image_format(&self) -> ImageFormat {
        if self.protocol_version() == 3 {
            ImageFormat {
                mode: ImageMode::JPEG,
                size: (176, 112), // from https://github.com/MiraboxSpace/StreamDock-Device-SDK/blob/31d887551de556bd0776bf4982233999d58e49d1/CPP-SDK/src/HotspotDevice/StreamDockN4/streamdockN4.cpp#L57
                rotation: ImageRotation::Rot180,
                mirror: ImageMirroring::None,
            }
        } else {
            ImageFormat {
                mode: ImageMode::JPEG,
                size: (60, 60),
                rotation: ImageRotation::Rot0,
                mirror: ImageMirroring::None,
            }
        }
    }

    pub fn supports_both_encoder_states(&self) -> bool {
        false
    }

    /// Whether the device firmware exposes the `QUCMD` config command with an
    /// `EnableVibration` slot. Confirmed by USB capture on N4 Pro E; other
    /// N4Pro-family devices share the same firmware config layout per
    /// Mirabox's official SDK (`streamdockN4Pro.h`).
    pub fn supports_vibration(&self) -> bool {
        matches!(self, Self::N4Pro | Self::N4ProE | Self::VsdN4Pro)
    }
}

// ---------------------------------------------------------------- entradas --

pub fn process_input(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    log::debug!("Processing input: {input}=0x{input:02x}=0b{input:08b}, {state}");

    match input {
        (1..=10) => read_button_press(input, state),
        0xa0 | 0xa1 | 0x50 | 0x51 | 0x90 | 0x91 | 0x70 | 0x71 => read_encoder_value(input),
        0x37 | 0x35 | 0x33 | 0x36 | 0x00 | 0x40 | 0x41 | 0x42 | 0x43 | 0x38 | 0x39 => {
            read_encoder_press(input, state)
        }
        _ => Err(MirajazzError::BadData),
    }
}

fn read_button_states(states: &[u8]) -> Vec<bool> {
    let mut bools = vec![];

    for i in 0..KEY_COUNT {
        bools.push(states[i + 1] != 0);
    }

    bools
}

fn read_button_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    let mut button_states = vec![0x01];
    button_states.extend(vec![0u8; KEY_COUNT + 1]);

    if input == 0 {
        return Ok(DeviceInput::ButtonStateChange(read_button_states(
            &button_states,
        )));
    }

    let pressed_index: usize = match input {
        (1..=10) => input as usize,
        // Three buttons without displays
        0x00 => 11,
        0x41 => 12,
        0x42 => 13,
        0x43 => 14,
        0x38 => 15,
        _ => return Err(MirajazzError::BadData),
    };
    if pressed_index >= 11 {
        button_states[pressed_index] = 1;
    } else {
        button_states[pressed_index] = state;
    }

    Ok(DeviceInput::ButtonStateChange(read_button_states(
        &button_states,
    )))
}

fn read_encoder_value(input: u8) -> Result<DeviceInput, MirajazzError> {
    let mut encoder_values = vec![0i8; ENCODER_COUNT];

    let (encoder, value): (usize, i8) = match input {
        // Encoder 1 (left most)
        0xa0 => (0, -1),
        0xa1 => (0, 1),
        // Encoder 2
        0x50 => (1, -1),
        0x51 => (1, 1),
        // Encoder 3
        0x90 => (2, -1),
        0x91 => (2, 1),
        // Encoder 4 (right most)
        0x70 => (3, -1),
        0x71 => (3, 1),
        _ => return Err(MirajazzError::BadData),
    };

    encoder_values[encoder] = value;
    Ok(DeviceInput::EncoderTwist(encoder_values))
}

fn read_encoder_press(input: u8, _state: u8) -> Result<DeviceInput, MirajazzError> {
    let mut encoder_states = vec![false; ENCODER_COUNT];

    let encoder: usize = match input {
        0x37 | 0x00 | 0x40 => 0, // Left most
        0x35 | 0x41 => 1,
        0x33 | 0x42 => 2,
        0x36 | 0x43 => 3, // Right most
        // Ignore swipe for now because they are unreliabe/detected incorrectly
        // 0x38 => 4,
        // 0x39 => 5,
        _ => return Err(MirajazzError::BadData),
    };

    encoder_states[encoder] = true;
    Ok(DeviceInput::EncoderStateChange(encoder_states))
}

// ------------------------------------------------------------------ imagen --

pub fn map_position(mut position: u8, is_encoder: bool) -> Result<u8, MirajazzError> {
    if is_encoder {
        position += 10;
    }
    let position = match position {
        0 => 10,
        1 => 11,
        2 => 12,
        3 => 13,
        4 => 14,
        5 => 5,
        6 => 6,
        7 => 7,
        8 => 8,
        9 => 9,
        10 => 0,
        11 => 1,
        12 => 2,
        13 => 3,
        _ => {
            log::error!("Invalid key position");
            return Err(MirajazzError::BadData);
        }
    };
    Ok(position)
}

/// Handles different combinations of "set image" event, including clearing the specific buttons and whole device
pub async fn set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
    let is_encoder = evt.controller.as_deref() == Some("Encoder");
    match (evt.position, evt.image) {
        (Some(position), Some(image)) => {
            log::debug!("Setting image for button {}", position);
            let position = map_position(position, is_encoder)?;

            // OpenDeck sends image as a data url, so parse it using a library
            let url = DataUrl::process(image.as_str()).unwrap(); // Isn't expected to fail, so unwrap it is
            let (body, _fragment) = url.decode_to_vec().unwrap(); // Same here

            // Allow only image/jpeg mime for now
            if url.mime_type().subtype != "jpeg" {
                log::error!("Incorrect mime type: {}", url.mime_type());

                return Ok(()); // Not a fatal error, enough to just log it
            }

            let image = load_from_memory_with_format(body.as_slice(), image::ImageFormat::Jpeg)?;

            let kind = Kind::from_vid_pid(device.vid, device.pid).unwrap();
            let format = if is_encoder {
                kind.touch_image_format()
            } else {
                kind.image_format()
            };
            // The panel conversion stretches into `format.size`, which distorts
            // dial images (OpenDeck renders 200x100, the N4 strip is 176x112):
            // pre-fit encoder images preserving aspect ratio, so that stretch
            // becomes a no-op.
            let image = if is_encoder {
                fit_into(image, format.size)
            } else {
                image
            };

            device.set_button_image(position, format, image).await?;
            device.flush().await?;
        }
        (Some(position), None) => {
            let position = map_position(position, is_encoder)?;
            device.clear_button_image(position).await?;
            device.flush().await?;
        }
        (None, None) => {
            device.clear_all_button_images().await?;
            device.flush().await?;
        }
        _ => {}
    }

    Ok(())
}

/// Fit `image` inside `size` preserving aspect ratio, centered on black.
/// `set_button_image` stretches into the panel format, which distorts images
/// whose aspect differs (e.g. OpenDeck's 200x100 dial image on the 176x112
/// touch strip); pre-fitting here keeps shapes intact at the cost of small
/// letterbox bars.
fn fit_into(image: DynamicImage, size: (usize, usize)) -> DynamicImage {
    let (ws, hs) = (size.0 as u32, size.1 as u32);
    let (w, h) = image.dimensions();
    if w == ws && h == hs {
        return image;
    }
    let scale = (ws as f32 / w as f32).min(hs as f32 / h as f32);
    let nw = ((w as f32 * scale).round() as u32).clamp(1, ws);
    let nh = ((h as f32 * scale).round() as u32).clamp(1, hs);
    let fitted = image
        .resize_exact(nw, nh, image::imageops::FilterType::Nearest)
        .into_rgb8();
    let mut canvas = image::RgbImage::from_pixel(ws, hs, image::Rgb([0, 0, 0]));
    image::imageops::overlay(
        &mut canvas,
        &fitted,
        ((ws - nw) / 2) as i64,
        ((hs - nh) / 2) as i64,
    );
    DynamicImage::ImageRgb8(canvas)
}
