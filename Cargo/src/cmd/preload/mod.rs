//! `sys.azenith-service preload -t [-v] [-m <size>] <path>...` — pull files
//! into the page cache.
//!
//! Rust rewrite of the old `preloadbin` binary, trimmed down to the single
//! mode the AZenith daemon's `GamePreload()` runs:
//!
//! ```text
//! sys.azenith-service preload -v -t -m 500M "/data/app/~~.../lib/arm64"
//! ```
//!
//! so only `-t` (touch), `-v` (print every file that was touched) and
//! `-m <size>` (skip files larger than the budget) exist. Eviction, mlock,
//! daemon/wait mode, batch lists and the `-i`/`-I` filters left with the C
//! code — nothing in the module ever invoked them.
//!
//! The stdout shape is kept identical to vmtouch, because `GamePreload()`
//! parses it: one path per line while verbose, then a summary whose
//! `   Touched Pages: <n> (<size>)` line is `sscanf`'d by the caller. Log
//! lines go through [`crate::common::log`] and never touch stdout.

use std::collections::HashSet;
use std::ffi::CString;
use std::io::{self, BufWriter, Write};
use std::os::raw::c_int;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::common::log::{comp, log_write, Lv};

/// The subcommand name this module answers to.
pub(crate) const PRELOAD: &str = "preload";

/// Recursion guard so a pathological tree cannot blow the stack (vmtouch uses
/// 1024; module directories are never deeper than a handful of levels).
const MAX_CRAWL_DEPTH: usize = 128;

/// Accepted options, everything else is rejected.
struct Options {
    verbose: bool,
    max_file_size: u64,
    paths: Vec<PathBuf>,
}

#[derive(Default)]
struct Stats {
    total_pages: i64,
    total_files: i64,
    total_dirs: i64,
}

/// Entry point for `sys.azenith-service preload ...`.
pub(crate) fn run(args: &[String]) {
    let opts = match parse_args(args) {
        Ok(Some(opts)) => opts,
        Ok(None) => { usage(); std::process::exit(1); }
        Err(e) => {
            eprintln!("preload: {e}");
            usage();
            std::process::exit(1);
        }
    };

    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        eprintln!("preload: cannot determine page size");
        std::process::exit(1);
    }

    let start = Instant::now();
    let mut stats = Stats::default();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut seen: HashSet<(u64, u64)> = HashSet::new();

    for path in &opts.paths {
        crawl(path, page_size as usize, &opts, &mut out, &mut stats, &mut seen, 0);
    }

    let _ = out.flush();
    let elapsed = start.elapsed().as_secs_f64();
    let total_size = stats.total_pages * page_size;

    // Same block vmtouch prints; `GamePreload()` only reads Touched Pages.
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(out, "           Files: {}", stats.total_files);
    let _ = writeln!(out, "     Directories: {}", stats.total_dirs);
    let _ = writeln!(out, "   Touched Pages: {} ({})", stats.total_pages, pretty_print_size(total_size));
    let _ = writeln!(out, "         Elapsed: {:.5} seconds", elapsed);
}

/// Parse `-t`, `-v` and `-m <size>`; `Ok(None)` means "show the usage".
fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut opts = Options { verbose: false, max_file_size: u64::MAX, paths: Vec::new() };
    let mut touch = false;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-t" => touch = true,
            "-v" => opts.verbose = true,
            "-m" => {
                i += 1;
                let Some(size) = args.get(i) else { return Err("-m needs a size".into()) };
                opts.max_file_size = parse_size(size)?;
            }
            "-h" | "--help" => return Ok(None),
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unsupported option {other} (only -t, -v and -m are built)"));
            }
            path => opts.paths.push(PathBuf::from(path)),
        }
        i += 1;
    }

    if !touch {
        return Err("only touch mode is built, pass -t".into());
    }
    if opts.paths.is_empty() {
        return Err("no files or directories specified".into());
    }

    Ok(Some(opts))
}

fn usage() {
    eprintln!(
        "usage: sys.azenith-service preload -t [-v] [-m <size>] <path>...\n\
         \n\
         \x20 -t           touch pages into memory (the only mode built)\n\
         \x20 -v           print every file as it is touched\n\
         \x20 -m <size>    skip files larger than <size> (suffix K, M or G)"
    );
}

/// `500M` / `1.5k` / `4096` -> bytes, as vmtouch's `parse_size()` does.
fn parse_size(input: &str) -> Result<u64, String> {
    if input.is_empty() {
        return Err("bad size format".into());
    }

    let (number, mult) = match input.as_bytes().last().unwrap().to_ascii_lowercase() {
        b'k' => (&input[..input.len() - 1], 1024f64),
        b'm' => (&input[..input.len() - 1], 1024f64 * 1024.0),
        b'g' => (&input[..input.len() - 1], 1024f64 * 1024.0 * 1024.0),
        _ => (input, 1f64),
    };

    let value: f64 = number
        .parse()
        .map_err(|_| format!("bad size format: {input}"))?;

    if !value.is_finite() || value < 0.0 {
        return Err(format!("bad size format: {input}"));
    }

    let bytes = value * mult;
    if bytes > u64::MAX as f64 {
        return Err(format!("size too large: {input}"));
    }
    Ok(bytes as u64)
}

/// `1234567` -> `1.1M`, matching vmtouch's `pretty_print_size()`.
fn pretty_print_size(bytes: i64) -> String {
    const UNITS: [&str; 4] = ["", "K", "M", "G"];
    let mut value = bytes;
    let mut unit = 0;

    while value >= 1024 && unit < UNITS.len() - 1 {
        value /= 1024;
        unit += 1;
    }
    format!("{value}{}", UNITS[unit])
}

/// vmtouch prints its warnings on stderr; they also go to the log so a module
/// run can be diagnosed after the fact.
fn warn(message: impl AsRef<str>) {
    let message = message.as_ref();
    eprintln!("preload: WARNING: {message}");
    log_write(Lv::Warn, comp::PRELOAD, message);
}

/// Walk `path` and touch every regular file inside it (vmtouch's crawl,
/// without symlinks, filters or cross-filesystem rules).
fn crawl(
    path: &Path,
    page_size: usize,
    opts: &Options,
    out: &mut impl Write,
    stats: &mut Stats,
    seen: &mut HashSet<(u64, u64)>,
    depth: usize,
) {
    use std::os::unix::fs::MetadataExt;

    let bytes = path.as_os_str().as_bytes();
    let clean: PathBuf = if bytes.len() > 1 && bytes.last() == Some(&b'/') {
        PathBuf::from(std::ffi::OsStr::from_bytes(&bytes[..bytes.len() - 1]))
    } else {
        path.to_path_buf()
    };

    let meta = match std::fs::symlink_metadata(&clean) {
        Ok(meta) => meta,
        Err(e) => {
            warn(format!("unable to stat {} ({e}), skipping", clean.display()));
            return;
        }
    };

    if meta.file_type().is_symlink() {
        warn(format!("not following symbolic link {}", clean.display()));
        return;
    }

    // Hardlinked copies are touched once, like vmtouch does by default.
    if meta.nlink() > 1 && !seen.insert((meta.dev(), meta.ino())) {
        return;
    }

    if meta.is_dir() {
        if depth >= MAX_CRAWL_DEPTH {
            warn(format!("maximum directory crawl depth reached: {}", clean.display()));
            return;
        }

        let entries = match std::fs::read_dir(&clean) {
            Ok(entries) => entries,
            Err(e) => {
                warn(format!("unable to opendir {} ({e}), skipping", clean.display()));
                return;
            }
        };

        stats.total_dirs += 1;
        for entry in entries.flatten() {
            crawl(&entry.path(), page_size, opts, out, stats, seen, depth + 1);
        }
    } else if meta.is_file() {
        stats.total_files += 1;
        touch_file(&clean, page_size, opts, out, stats);
    } else {
        warn(format!("skipping non-regular file: {}", clean.display()));
    }
}

/// mmap the file read-only and read the first byte of every page, which is
/// what pulls the range into the page cache.
fn touch_file(path: &Path, page_size: usize, opts: &Options, out: &mut impl Write, stats: &mut Stats) {
    let Ok(cpath) = CString::new(path.as_os_str().as_bytes()) else { return };

    let fd = match open_readonly(&cpath) {
        Ok(fd) => fd,
        Err(e) => {
            warn(format!("unable to open {} ({e}), skipping", path.display()));
            return;
        }
    };

    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } != 0 {
        let e = io::Error::last_os_error();
        warn(format!("unable to fstat {} ({e}), skipping", path.display()));
        unsafe { libc::close(fd) };
        return;
    }

    // Block devices have no meaningful size here (vmtouch issues
    // BLKGETSIZE64 for them); an APK/lib tree never contains one.
    if st.st_mode & libc::S_IFMT == libc::S_IFBLK {
        warn(format!("skipping block device {}", path.display()));
        unsafe { libc::close(fd) };
        return;
    }

    let len = st.st_size;
    if len <= 0 || len as u64 > opts.max_file_size {
        if len as u64 > opts.max_file_size {
            warn(format!("file {} too large, skipping", path.display()));
        }
        unsafe { libc::close(fd) };
        return;
    }

    // mmap()'s length must fit in isize on 32-bit (armeabi-v7a builds).
    if len as u64 > isize::MAX as u64 {
        warn(format!("file {} too large, skipping", path.display()));
        unsafe { libc::close(fd) };
        return;
    }

    let mem = unsafe {
        libc::mmap(std::ptr::null_mut(), len as usize, libc::PROT_READ, libc::MAP_SHARED, fd, 0)
    };
    if mem == libc::MAP_FAILED {
        let e = io::Error::last_os_error();
        warn(format!("unable to mmap file {} ({e}), skipping", path.display()));
        unsafe { libc::close(fd) };
        return;
    }

    let pages_in_range = (len + page_size as i64 - 1) / page_size as i64;
    stats.total_pages += pages_in_range;

    if opts.verbose {
        let _ = writeln!(out, "{}", path.display());
    }

    // Volatile reads keep the compiler from dropping the touch loop; the
    // accumulator is folded away afterwards so it cannot be optimized out.
    let mut junk: u8 = 0;
    for page in 0..pages_in_range {
        let offset = (page * page_size as i64) as usize;
        if offset >= len as usize {
            break;
        }
        unsafe {
            let byte = (mem as *const u8).add(offset).read_volatile();
            junk = junk.wrapping_add(byte);
        }
    }
    std::hint::black_box(junk);

    unsafe {
        libc::munmap(mem, len as usize);
        libc::close(fd);
    }
}

/// `open(O_RDONLY | O_NOATIME)`, retrying without `O_NOATIME` on EPERM and
/// raising the file-descriptor limit once on EMFILE/ENFILE (vmtouch loops on
/// the latter forever; two attempts are enough to tell "limit" from "broken").
fn open_readonly(cpath: &CString) -> io::Result<c_int> {
    let mut flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOATIME;
    let mut bump_rlimit = false;

    loop {
        let fd = unsafe { libc::open(cpath.as_ptr(), flags, 0 as libc::mode_t) };
        if fd >= 0 {
            return Ok(fd);
        }

        let err = io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EPERM) if flags & libc::O_NOATIME != 0 => flags &= !libc::O_NOATIME,
            Some(libc::EMFILE) | Some(libc::ENFILE) if !bump_rlimit => {
                bump_rlimit = true;
                increment_nofile_rlimit();
            }
            _ => return Err(err),
        }
    }
}

/// Raise `RLIMIT_NOFILE` so a library-heavy APK tree can be walked in one go.
fn increment_nofile_rlimit() {
    unsafe {
        let mut r: libc::rlimit = std::mem::zeroed();
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) != 0 {
            return;
        }
        r.rlim_cur = r.rlim_max.saturating_add(1);
        r.rlim_max = r.rlim_max.saturating_add(1);
        let _ = libc::setrlimit(libc::RLIMIT_NOFILE, &r);
    }
}
