//! EN: zapret: files in a folder (winws.exe, lists, general*.bat strategies) and the `zapret` Windows service.
//! RU: zapret: файлы в папке (winws.exe, списки, стратегии general*.bat) и служба Windows `zapret`.

pub mod control;
pub mod files;
pub mod host;
pub mod fsutil;
pub mod pick;
pub mod release;
pub mod service;
pub mod services;
pub mod strategy;
pub mod update;
