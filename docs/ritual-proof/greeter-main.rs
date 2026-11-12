use std::env;
use std::io::{self, ErrorKind, Write};
use std::process;

fn greeting(name: &str) -> String {
    format!("Hello, {name}!")
}

fn render(args: &[String]) -> String {
    let shout = args.iter().any(|a| a == "--shout");
    let name = args
        .iter()
        .find(|a| a.as_str() != "--shout")
        .map(String::as_str)
        .unwrap_or("world");
    let line = greeting(name);
    if shout { line.to_uppercase() } else { line }
}

/// Exit code for the write: a reader that left (`BrokenPipe`) is not a failure.
fn emit(out: &mut impl Write, line: &str) -> i32 {
    match writeln!(out, "{line}").and_then(|()| out.flush()) {
        Ok(()) => 0,
        Err(e) if e.kind() == ErrorKind::BrokenPipe => 0,
        Err(e) => {
            // `eprintln!` would panic if stderr also fails.
            let _ = writeln!(io::stderr(), "greeter: {e}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let line = render(&args);
    process::exit(emit(&mut io::stdout().lock(), &line));
}

#[cfg(test)]
mod tests {
    use super::{emit, render};
    use std::io::{self, ErrorKind, Write};

    struct Failing(ErrorKind);

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(self.0))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn emit_ok() {
        let mut buf = Vec::new();
        assert_eq!(emit(&mut buf, "x"), 0);
        assert_eq!(buf, b"x\n");
    }

    #[test]
    fn emit_broken_pipe() {
        assert_eq!(emit(&mut Failing(ErrorKind::BrokenPipe), "x"), 0);
    }

    #[test]
    fn emit_other_error() {
        assert_eq!(emit(&mut Failing(ErrorKind::Other), "x"), 1);
    }

    fn r(args: &[&str]) -> String {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        render(&owned)
    }

    #[test]
    fn default_name() {
        assert_eq!(r(&[]), "Hello, world!");
    }

    #[test]
    fn name_unchanged() {
        assert_eq!(r(&["Ada"]), "Hello, Ada!");
    }

    #[test]
    fn shout_before_name() {
        assert_eq!(r(&["--shout", "Ada"]), "HELLO, ADA!");
    }

    #[test]
    fn shout_after_name() {
        assert_eq!(r(&["Ada", "--shout"]), "HELLO, ADA!");
    }

    #[test]
    fn shout_unicode() {
        assert_eq!(r(&["--shout", "straße"]), "HELLO, STRASSE!");
    }

    #[test]
    fn shout_alone() {
        assert_eq!(r(&["--shout"]), "HELLO, WORLD!");
    }

    #[test]
    fn unknown_flag_is_name() {
        assert_eq!(r(&["--whisper"]), "Hello, --whisper!");
    }
}
