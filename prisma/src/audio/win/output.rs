//! Salidas de audio (altavoces, auriculares…): cuáles hay, cuál usa Windows
//! y cambiarla. Para cambiarla Windows no tiene API pública: se usa
//! IPolicyConfig, la interfaz sin documentar que usan el propio panel de
//! sonido y herramientas como NirCmd o SoundSwitch.

use std::ffi::c_void;

use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{DEVICE_STATE_ACTIVE, ERole, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCommunications, eConsole, eMultimedia, eRender},
        System::Com::{
            CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ,
            StructuredStorage::{PropVariantToBSTR, PropVariantToUInt32},
        },
    },
    core::{GUID, HRESULT, HSTRING, IUnknown, Interface, PCWSTR},
};

use crate::audio::{Output, OutputKind};

/// Nombre que se ve en Windows («Altavoces (Realtek…)»)
const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };

/// Qué es (EndpointFormFactor: altavoces, auriculares, HDMI…)
const PKEY_AUDIO_ENDPOINT_FORM_FACTOR: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x1da5d803_d492_4edd_8c23_e0c0ffee7f0e), pid: 0 };

/// IPolicyConfig (sin documentar) y su objeto
const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
/// Lugar de SetDefaultEndpoint en su tabla (3 de IUnknown + 10 antes)
const SET_DEFAULT_ENDPOINT: usize = 13;
type SetDefaultEndpoint = unsafe extern "system" fn(*mut c_void, PCWSTR, ERole) -> HRESULT;

fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    super::com();
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn id_of(d: &IMMDevice) -> Option<String> {
    unsafe {
        let p = d.GetId().ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const c_void));
        s
    }
}

fn name_of(d: &IMMDevice) -> Option<String> {
    unsafe {
        let v = d.OpenPropertyStore(STGM_READ).ok()?.GetValue(&PKEY_DEVICE_FRIENDLY_NAME).ok()?;
        PropVariantToBSTR(&v).ok().map(|b| b.to_string())
    }
}

/// Valores de EndpointFormFactor
fn kind_of(d: &IMMDevice) -> OutputKind {
    let v = unsafe { d.OpenPropertyStore(STGM_READ).and_then(|s| s.GetValue(&PKEY_AUDIO_ENDPOINT_FORM_FACTOR)).and_then(|v| PropVariantToUInt32(&v)) };
    match v {
        Ok(3) => OutputKind::Headphones,
        Ok(5) | Ok(6) => OutputKind::Headset,
        Ok(9) => OutputKind::Display,
        Ok(7) | Ok(8) => OutputKind::Digital,
        _ => OutputKind::Speakers,
    }
}

/// Qué es la salida que usa Windows ahora
pub fn default_output_kind() -> Option<OutputKind> {
    let d = unsafe { enumerator().ok()?.GetDefaultAudioEndpoint(eRender, eConsole) }.ok()?;
    Some(kind_of(&d))
}

/// Salidas activas, en el orden de Windows, con la que se usa marcada
pub fn outputs() -> Vec<Output> {
    let Ok(e) = enumerator() else { return vec![] };
    let default = unsafe { e.GetDefaultAudioEndpoint(eRender, eConsole) }.ok().and_then(|d| id_of(&d));
    let Ok(list) = (unsafe { e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }) else { return vec![] };
    let n = unsafe { list.GetCount() }.unwrap_or(0);
    (0..n)
        .filter_map(|i| unsafe { list.Item(i) }.ok())
        .filter_map(|d| {
            let id = id_of(&d)?;
            let name = name_of(&d).unwrap_or_else(|| id.clone());
            Some(Output { default: default.as_deref() == Some(id.as_str()), kind: kind_of(&d), id, name })
        })
        .collect()
}

/// Pone la salida `id` como la de Windows (para todo: juegos, música y llamadas)
pub fn set_default_output(id: &str) -> Result<(), String> {
    super::com();
    unsafe {
        let unk: IUnknown = CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        let mut p: *mut c_void = std::ptr::null_mut();
        unk.query(&IID_POLICY_CONFIG, &mut p).ok().map_err(|e| e.to_string())?;
        // Se libera al acabar
        let _policy = IUnknown::from_raw(p);
        let table = *(p as *const *const SetDefaultEndpoint);
        let set = *table.add(SET_DEFAULT_ENDPOINT);
        let wide = HSTRING::from(id);
        for role in [eConsole, eMultimedia, eCommunications] {
            set(p, PCWSTR(wide.as_ptr()), role).ok().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
