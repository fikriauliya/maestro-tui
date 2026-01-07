use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Convert a crossterm KeyEvent to terminal escape bytes
pub fn key_to_bytes(key: &KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+letter -> ASCII 1-26
                let ctrl = (c.to_ascii_lowercase() as u8).wrapping_sub(b'a' - 1);
                vec![ctrl]
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![127],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => vec![],
        },
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn make_key_with_mods(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn test_char_keys() {
        assert_eq!(key_to_bytes(&make_key(KeyCode::Char('a'))), b"a");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Char('z'))), b"z");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Char('A'))), b"A");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Char('0'))), b"0");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Char(' '))), b" ");
    }

    #[test]
    fn test_ctrl_keys() {
        // Ctrl+A = 0x01, Ctrl+C = 0x03, Ctrl+Z = 0x1a
        assert_eq!(
            key_to_bytes(&make_key_with_mods(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL
            )),
            vec![1]
        );
        assert_eq!(
            key_to_bytes(&make_key_with_mods(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL
            )),
            vec![3]
        );
        assert_eq!(
            key_to_bytes(&make_key_with_mods(
                KeyCode::Char('z'),
                KeyModifiers::CONTROL
            )),
            vec![26]
        );
        // Ctrl works case-insensitively
        assert_eq!(
            key_to_bytes(&make_key_with_mods(
                KeyCode::Char('C'),
                KeyModifiers::CONTROL
            )),
            vec![3]
        );
    }

    #[test]
    fn test_special_keys() {
        assert_eq!(key_to_bytes(&make_key(KeyCode::Enter)), b"\r");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Backspace)), vec![127]);
        assert_eq!(key_to_bytes(&make_key(KeyCode::Tab)), b"\t");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Esc)), vec![0x1b]);
    }

    #[test]
    fn test_arrow_keys() {
        assert_eq!(key_to_bytes(&make_key(KeyCode::Up)), b"\x1b[A");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Down)), b"\x1b[B");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Right)), b"\x1b[C");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Left)), b"\x1b[D");
    }

    #[test]
    fn test_navigation_keys() {
        assert_eq!(key_to_bytes(&make_key(KeyCode::Home)), b"\x1b[H");
        assert_eq!(key_to_bytes(&make_key(KeyCode::End)), b"\x1b[F");
        assert_eq!(key_to_bytes(&make_key(KeyCode::PageUp)), b"\x1b[5~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::PageDown)), b"\x1b[6~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Delete)), b"\x1b[3~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::Insert)), b"\x1b[2~");
    }

    #[test]
    fn test_function_keys() {
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(1))), b"\x1bOP");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(2))), b"\x1bOQ");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(3))), b"\x1bOR");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(4))), b"\x1bOS");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(5))), b"\x1b[15~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(6))), b"\x1b[17~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(7))), b"\x1b[18~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(8))), b"\x1b[19~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(9))), b"\x1b[20~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(10))), b"\x1b[21~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(11))), b"\x1b[23~");
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(12))), b"\x1b[24~");
        // Unknown F keys return empty
        assert_eq!(key_to_bytes(&make_key(KeyCode::F(13))), Vec::<u8>::new());
    }

    #[test]
    fn test_unknown_keys() {
        assert_eq!(key_to_bytes(&make_key(KeyCode::Null)), Vec::<u8>::new());
    }

    #[test]
    fn test_unicode_chars() {
        assert_eq!(key_to_bytes(&make_key(KeyCode::Char('é'))), "é".as_bytes());
        assert_eq!(
            key_to_bytes(&make_key(KeyCode::Char('日'))),
            "日".as_bytes()
        );
    }
}
