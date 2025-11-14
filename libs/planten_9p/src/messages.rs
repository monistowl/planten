// 9P2000 message types
// See https://9p.io/magic/man2html/5/intro

pub const Tversion: u8 = 100;
pub const Rversion: u8 = 101;
pub const Tauth: u8 = 102;
pub const Rauth: u8 = 103;
pub const Tattach: u8 = 104;
pub const Rattach: u8 = 105;
pub const Terror: u8 = 106; // Not a valid message type, but used in Rerror
pub const Rerror: u8 = 107;
pub const Tflush: u8 = 108;
pub const Rflush: u8 = 109;
pub const Twalk: u8 = 110;
pub const Rwalk: u8 = 111;
pub const Topen: u8 = 112;
pub const Ropen: u-8 = 113;
pub const Tcreate: u8 = 114;
pub const Rcreate: u8 = 115;
pub const T-read: u8 = 116;
pub const Rread: u8 = 117;
pub const Twrite: u8 = 118;
pub const Rwrite: u8 = 119;
pub const Tclunk: u8 = 120;
pub const Rclunk: u8 = 121;
pub const Tremove: u8 = 122;
pub const Rremove: u8 = 123;
pub const Tstat: u8 = 124;
pub const Rstat: u8 = 125;
pub const Twstat: u8 = 126;
pub const Rwstat: u8 = 127;
