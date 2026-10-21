use std::fs::File;
use std::io::{BufWriter, Write};

/// Writes the trace file produced by `/debug function`, mirroring vanilla's
/// `DebugCommand.Tracer`: one `[F]` line per function call and one `[C]` line
/// per executed command, indented by call depth.
pub struct FunctionTrace {
    output: BufWriter<File>,
    traced_functions: usize,
}

impl FunctionTrace {
    #[must_use]
    pub fn new(file: File) -> Self {
        Self {
            output: BufWriter::new(file),
            traced_functions: 0,
        }
    }

    fn print_indent(&mut self, depth: usize) {
        // Vanilla's `INDENT_OFFSET` prints one level more than the call depth.
        for _ in 0..=depth {
            let _ = write!(self.output, "    ");
        }
    }

    pub fn on_command(&mut self, depth: usize, command: &str) {
        self.print_indent(depth);
        let _ = writeln!(self.output, "[C] {command}");
    }

    pub fn on_call(&mut self, depth: usize, function: &str, size: usize) {
        self.print_indent(depth);
        let _ = writeln!(self.output, "[F] {function} size={size}");
        // Only the top-level calls count as "functions" in the success
        // message; nested calls see a deeper command depth.
        if depth == 0 {
            self.traced_functions += 1;
        }
    }

    #[must_use]
    pub const fn traced_functions(&self) -> usize {
        self.traced_functions
    }
}

#[cfg(test)]
mod tests {
    use super::FunctionTrace;

    #[test]
    fn writes_indented_function_and_command_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("debug-trace.txt");
        let file = std::fs::File::create(&path).expect("create trace file");

        let mut trace = FunctionTrace::new(file);
        trace.on_call(0, "test:outer", 2);
        trace.on_command(1, "say hi");
        trace.on_call(1, "test:inner", 1);
        trace.on_command(2, "say inner");
        assert_eq!(trace.traced_functions(), 1);
        drop(trace);

        let content = std::fs::read_to_string(&path).expect("read trace file");
        assert_eq!(
            content,
            "    [F] test:outer size=2\n        [C] say hi\n        [F] test:inner size=1\n            [C] say inner\n"
        );
    }
}
