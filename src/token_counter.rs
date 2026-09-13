use std::fs::File;
use std::io::{self, Error, ErrorKind};
use std::path::Path;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use memmap2::Mmap;
use rayon::prelude::*;
use tiktoken::{CoreBpe, get_encoding};

// Files under 64 KB are faster to read directly into userspace buffer
// than paying the OS page-table setup overhead of mmap.
const MMAP_THRESHOLD_BYTES: u64 = 64 * 1024;

// Files larger than 256 KB benefit from multi-threaded parallel counting.
const PARALLEL_THRESHOLD_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerEncoding {
    Cl100kBase, // GPT-4, Claude, modern standard
    O200kBase,  // GPT-4o
}

impl TokenizerEncoding {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cl100kBase => "cl100k_base",
            Self::O200kBase => "o200k_base",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Cl100kBase => "cl100k_base (GPT-4 / Claude standard)",
            Self::O200kBase => "o200k_base (GPT-4o standard)",
        }
    }
}

// Global active encoding: 0 = Cl100kBase, 1 = O200kBase
static ACTIVE_ENCODING: AtomicU8 = AtomicU8::new(0);

static CL100K_TOKENIZER: LazyLock<&'static CoreBpe> =
    LazyLock::new(|| get_encoding("cl100k_base").expect("Failed to load cl100k_base encoding"));

static O200K_TOKENIZER: LazyLock<&'static CoreBpe> =
    LazyLock::new(|| get_encoding("o200k_base").expect("Failed to load o200k_base encoding"));

pub fn get_active_encoding() -> TokenizerEncoding {
    if ACTIVE_ENCODING.load(Ordering::Relaxed) == 1 {
        TokenizerEncoding::O200kBase
    } else {
        TokenizerEncoding::Cl100kBase
    }
}

pub fn set_active_encoding(encoding: TokenizerEncoding) {
    let val = match encoding {
        TokenizerEncoding::Cl100kBase => 0,
        TokenizerEncoding::O200kBase => 1,
    };
    ACTIVE_ENCODING.store(val, Ordering::Relaxed);
}

pub fn get_active_tokenizer() -> &'static CoreBpe {
    match get_active_encoding() {
        TokenizerEncoding::Cl100kBase => *CL100K_TOKENIZER,
        TokenizerEncoding::O200kBase => *O200K_TOKENIZER,
    }
}

/// High-performance token counter for any file size.
pub fn count_tokens_file<P: AsRef<Path>>(path: P) -> io::Result<usize> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_len = metadata.len();

    if file_len == 0 {
        return Ok(0);
    }

    let tokenizer = get_active_tokenizer();

    if file_len < MMAP_THRESHOLD_BYTES {
        // Fast path for small files (< 64 KB, e.g., 500–2,000 lines of code)
        use std::io::Read;
        let mut text = String::with_capacity(file_len as usize);
        let mut file = file;
        file.read_to_string(&mut text)?;
        Ok(tokenizer.count(&text))
    } else {
        // Large file path: Memory-map directly from kernel page cache
        let mmap = unsafe { Mmap::map(&file)? };

        let text = std::str::from_utf8(&mmap).map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!("File is not valid UTF-8: {e}"),
            )
        })?;

        if text.len() < PARALLEL_THRESHOLD_BYTES {
            // Single-threaded zero-allocation scan for medium files (64 KB - 256 KB)
            Ok(tokenizer.count(text))
        } else {
            // Multi-threaded parallel chunking across CPU cores for large files (> 256 KB)
            // Splitting strictly on newline boundaries prevents breaking multi-byte tokens.
            let total_tokens: usize = text.par_lines().map(|line| tokenizer.count(line)).sum();

            // par_lines() strips newline chars; add back the newline tokens
            let newline_tokens = text.bytes().filter(|&b| b == b'\n').count();
            Ok(total_tokens + newline_tokens)
        }
    }
}

/// Instant in-memory token counter for strings and editor buffer lines
pub fn count_tokens_str(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let tokenizer = get_active_tokenizer();

    if text.len() < PARALLEL_THRESHOLD_BYTES {
        tokenizer.count(text)
    } else {
        let total_tokens: usize = text.par_lines().map(|line| tokenizer.count(line)).sum();
        let newline_tokens = text.bytes().filter(|&b| b == b'\n').count();
        total_tokens + newline_tokens
    }
}

/// Formats a token count into a readable string (e.g. "1,420" or "45.2K")
pub fn format_token_count(tokens: usize) -> String {
    if tokens < 10_000 {
        let s = tokens.to_string();
        let mut result = String::new();
        let len = s.len();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (len - i).is_multiple_of(3) {
                result.push(',');
            }
            result.push(c);
        }
        result
    } else if tokens < 1_000_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{:.2}M", tokens as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_empty_file() {
        let temp_file = std::env::temp_dir().join("tomy_token_empty.py");
        std::fs::write(&temp_file, "").unwrap();

        let count = count_tokens_file(&temp_file).unwrap();
        assert_eq!(count, 0);

        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_count_tokens_small_file() {
        let temp_file = std::env::temp_dir().join("tomy_token_small.py");
        let content = "def hello_world():\n    print('Hello, Tomy Tiktoken!')\n";
        std::fs::write(&temp_file, content).unwrap();

        let count = count_tokens_file(&temp_file).unwrap();
        assert!(count > 0);
        assert_eq!(count, count_tokens_str(content));

        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_count_tokens_str_consistency() {
        let snippet =
            "import numpy as np\nimport torch\n\ndef forward(x):\n    return torch.relu(x)\n";
        let count = count_tokens_str(snippet);
        assert!(count >= 10);
    }

    #[test]
    fn test_encoding_toggle() {
        let current = get_active_encoding();
        assert_eq!(current, TokenizerEncoding::Cl100kBase);

        set_active_encoding(TokenizerEncoding::O200kBase);
        assert_eq!(get_active_encoding(), TokenizerEncoding::O200kBase);

        // Reset back to Cl100kBase
        set_active_encoding(TokenizerEncoding::Cl100kBase);
        assert_eq!(get_active_encoding(), TokenizerEncoding::Cl100kBase);
    }

    #[test]
    fn test_format_token_count() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(42), "42");
        assert_eq!(format_token_count(1420), "1,420");
        assert_eq!(format_token_count(12500), "12.5K");
        assert_eq!(format_token_count(1500000), "1.50M");
    }
}
