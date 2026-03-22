use std::{fmt::Display, num::ParseIntError};

#[allow(dead_code)]
#[derive(Debug)]
pub struct FramebufferSpyConfig {
    pub address: usize,
    pub width: u32,
    pub height: u32,
    pub r#type: u32,
    pub bpl: u32,
    pub requires_reload: bool,
}

#[derive(Debug)]
pub struct FramebufferSpyConfigParsingError;
impl From<ParseIntError> for FramebufferSpyConfigParsingError {
    fn from(_: ParseIntError) -> Self {
        Self
    }
}
impl std::error::Error for FramebufferSpyConfigParsingError {}
impl Display for FramebufferSpyConfigParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FramebufferSpyParsingError")
    }
}
impl FramebufferSpyConfig {
    pub fn parse(string: &str) -> Result<Self, FramebufferSpyConfigParsingError> {
        let tokens = string.split(",").collect::<Vec<_>>();
        if tokens.len() != 6 {
            Err(FramebufferSpyConfigParsingError)
        } else {
            let s_fb_addr = tokens[0];
            let s_width = tokens[1];
            let s_height = tokens[2];
            let s_type = tokens[3];
            let s_bpl = tokens[4];
            let s_requires_reload = tokens[5];
            if !s_fb_addr.starts_with("0x") {
                Err(FramebufferSpyConfigParsingError)
            } else {
                Ok(Self {
                    address: usize::from_str_radix(&s_fb_addr[2..s_fb_addr.len()], 16)?,
                    width: s_width.parse()?,
                    height: s_height.parse()?,
                    r#type: s_type.parse()?,
                    bpl: s_bpl.parse()?,
                    requires_reload: s_requires_reload == "1",
                })
            }
        }
    }
}
