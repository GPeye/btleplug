// btleplug Source Code File
//
// Copyright 2020 Nonpolynomial Labs LLC. All rights reserved.
//
// Licensed under the BSD 3-Clause license. See LICENSE file in the project root
// for full license information.
//
// Some portions of this file are taken and/or modified from Rumble
// (https://github.com/mwylde/rumble), using a dual MIT/Apache License under the
// following copyright:
//
// Copyright (c) 2014 The Rust Project Developers

use std::sync::Arc;

use crate::{Error, Result};
use windows::{
    Devices::Bluetooth::Advertisement::*,
    Devices::{Bluetooth::BluetoothConnectionStatus, Enumeration::DeviceInformation},
    Devices::{Bluetooth::BluetoothLEDevice, Enumeration::DeviceWatcher},
    Foundation::TypedEventHandler,
};

pub type DeviceWatchAddedEventHandler = Arc<dyn Fn(&BluetoothLEDevice) + Send + Sync>;

pub struct BLEWatcher {
    bt_ad_watcher: BluetoothLEAdvertisementWatcher,
    bt_device_watcher: DeviceWatcher,
}

impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Error {
        Error::Other(format!("{:?}", err).into())
    }
}

impl BLEWatcher {
    pub fn new() -> Self {
        let ad = BluetoothLEAdvertisementFilter::new().unwrap();
        let bt_ad_watcher = BluetoothLEAdvertisementWatcher::Create(&ad).unwrap();

        let aqs = BluetoothLEDevice::GetDeviceSelectorFromConnectionStatus(
            BluetoothConnectionStatus::Connected,
        )
        .unwrap();

        let bt_device_watcher = DeviceInformation::CreateWatcherAqsFilter(&aqs).unwrap();
        BLEWatcher {
            bt_ad_watcher,
            bt_device_watcher,
        }
    }

    pub fn start(&self, on_new_device: DeviceWatchAddedEventHandler) -> Result<()> {
        self.bt_ad_watcher
            .SetScanningMode(BluetoothLEScanningMode::Active)
            .unwrap();

        let bt_ad_handler: TypedEventHandler<
            BluetoothLEAdvertisementWatcher,
            BluetoothLEAdvertisementReceivedEventArgs,
        > = TypedEventHandler::new({
            let on_new_device = on_new_device.clone();
            move |_sender, args: &Option<BluetoothLEAdvertisementReceivedEventArgs>| {
                if let Some(args) = args {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    let bluetooth_address = args.BluetoothAddress().unwrap();
                    let d = rt.block_on(async {
                        BluetoothLEDevice::FromBluetoothAddressAsync(
                            bluetooth_address.try_into().unwrap(),
                        )
                        .unwrap()
                        .await
                    });
                    match d {
                        Ok(device) => {
                            on_new_device(&device);
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
        });

        let bt_device_handler: TypedEventHandler<DeviceWatcher, DeviceInformation> =
            TypedEventHandler::new({
                let on_new_device = on_new_device.clone();
                move |_sender, args: &Option<DeviceInformation>| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    if let Some(args) = args {
                        let id = args.Id().unwrap();
                        let d = rt.block_on(async {
                            BluetoothLEDevice::FromIdAsync(&id.try_into().unwrap())
                                .unwrap()
                                .await
                        });
                        match d {
                            Ok(device) => {
                                on_new_device(&device);
                            }
                            _ => {}
                        }
                    }
                    Ok(())
                }
            });

        self.bt_ad_watcher.Received(&bt_ad_handler)?;
        self.bt_ad_watcher.Start()?;

        self.bt_device_watcher.Added(&bt_device_handler)?;
        self.bt_device_watcher.Start()?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.bt_ad_watcher.Stop()?;
        self.bt_device_watcher.Stop()?;
        Ok(())
    }
}
