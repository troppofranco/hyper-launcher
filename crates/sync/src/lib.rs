//! Sincronizzazione delle impostazioni tra istanze. Per ora contiene solo il
//! parser di `options.txt`, che sarà la base per sincronizzare keybind e
//! impostazioni grafiche nelle fasi successive.

mod options;

pub use options::OptionsFile;
