#[allow(unused_imports)]
use std::ascii::AsciiExt;

// header value is byte sequence
// we need case insensitive comparison and strip out of the whitespace
pub fn is_close(val: &[u8]) -> bool {
    if val.len() < "close".len() {
        return false;
    }
    let mut iter = val.iter();
    for (idx, &ch) in iter.by_ref().enumerate() {
        match ch {
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            b'c' | b'C' => {
                if idx + "close".len() > val.len() {
                    return false;
                }
                break;
            }
            _ => return false,
        }
    }
    for (idx, ch) in iter.by_ref().take(4).enumerate() {
        if b"lose"[idx] != ch.to_ascii_lowercase() {
            return false;
        }
    }
    for &ch in iter {
        if !matches!(ch, b'\r' | b'\n' | b' ' | b'\t') {
            return false;
        }
    }
    return true;
}

// header value is byte sequence
// we need case insensitive comparison and strip out of the whitespace
pub fn is_chunked(val: &[u8]) -> bool {
    if val.len() < "chunked".len() {
        return false;
    }
    let mut iter = val.iter();
    for (idx, &ch) in iter.by_ref().enumerate() {
        match ch {
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            b'c' | b'C' => {
                if idx + "chunked".len() > val.len() {
                    return false;
                }
                break;
            }
            _ => return false,
        }
    }
    for (idx, ch) in iter.by_ref().take(6).enumerate() {
        if b"hunked"[idx] != ch.to_ascii_lowercase() {
            return false;
        }
    }
    for &ch in iter {
        if !matches!(ch, b'\r' | b'\n' | b' ' | b'\t') {
            return false;
        }
    }
    return true;
}

// header value is byte sequence
// we need case insensitive comparison and strip out of the whitespace
pub fn is_continue(val: &[u8]) -> bool {
    if val.len() < "100-continue".len() {
        return false;
    }
    let mut iter = val.iter();
    for (idx, &ch) in iter.by_ref().enumerate() {
        match ch {
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            b'1' => {
                if idx + "100-continue".len() > val.len() {
                    return false;
                }
                break;
            }
            _ => return false,
        }
    }
    for (idx, ch) in iter.by_ref().take(11).enumerate() {
        if b"00-continue"[idx] != ch.to_ascii_lowercase() {
            return false;
        }
    }
    for &ch in iter {
        if !matches!(ch, b'\r' | b'\n' | b' ' | b'\t') {
            return false;
        }
    }
    return true;
}

#[cfg(test)]
mod test {
    use super::{is_chunked, is_close, is_continue};

    #[test]
    fn test_chunked() {
        assert!(is_chunked(b"chunked"));
        assert!(is_chunked(b"Chunked"));
        assert!(is_chunked(b"chuNKED"));
        assert!(is_chunked(b"CHUNKED"));
        assert!(is_chunked(b"   CHUNKED"));
        assert!(is_chunked(b"   CHUNKED  "));
        assert!(is_chunked(b"chunked  "));
        assert!(is_chunked(b"   CHUNKED"));
        assert!(!is_chunked(b"   CHUNKED 1 "));
    }

    #[test]
    fn test_close() {
        assert!(is_close(b"close"));
        assert!(is_close(b"Close"));
        assert!(is_close(b"clOSE"));
        assert!(is_close(b"CLOSE"));
        assert!(is_close(b" CLOSE"));
        assert!(is_close(b"   close   "));
        assert!(!is_close(b"Close  1 "));
        assert!(!is_close(b" xclose   "));
    }

    #[test]
    fn test_continue() {
        assert!(is_continue(b"100-continue"));
        assert!(is_continue(b"100-Continue"));
        assert!(is_continue(b"100-conTINUE"));
        assert!(is_continue(b"100-CONTINUE"));
        assert!(is_continue(b"  100-CONTINUE"));
        assert!(is_continue(b"   100-continue   "));
        assert!(!is_continue(b"100-continue y  "));
        assert!(!is_continue(b"100-coztinue   "));
    }
}
