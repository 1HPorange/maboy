pub trait CartridgeBattery {}

pub struct Battery;

impl CartridgeBattery for Battery {}

pub struct NoBattery;

impl CartridgeBattery for NoBattery {}
