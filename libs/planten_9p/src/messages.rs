// 9P2000 message types
// See https://9p.io/magic/man2html/5/intro

pub const TVERSION: u8 = 100;
pub const RVERSION: u8 = 101;
pub const TAUTH: u8 = 102;
pub const RAUTH: u8 = 103;
pub const TATTACH: u8 = 104;
pub const RATTACH: u8 = 105;
pub const TERROR: u8 = 106; // Not a valid message type, but used in RERROR
pub const RERROR: u8 = 107;
pub const TFLUSH: u8 = 108;
pub const RFLUSH: u8 = 109;
pub const TWALK: u8 = 110;
pub const RWALK: u8 = 111;
pub const TOPEN: u8 = 112;
pub const ROPEN: u8 = 113;
pub const TCREATE: u8 = 114;
pub const RCREATE: u8 = 115;
pub const TREAD: u8 = 116;
pub const RREAD: u8 = 117;
pub const TWRITE: u8 = 118;
pub const RWRITE: u8 = 119;
pub const TCLUNK: u8 = 120;
pub const RCLUNK: u8 = 121;
pub const TREMOVE: u8 = 122;
pub const RREMOVE: u8 = 123;
pub const TSTAT: u8 = 124;
pub const RSTAT: u8 = 125;
pub const TWSTAT: u8 = 126;
pub const RWSTAT: u8 = 127;
pub const TCLONE: u8 = 128;
pub const RCLONE: u8 = 129;
