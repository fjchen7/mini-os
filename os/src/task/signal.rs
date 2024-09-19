use alloc::vec;
use bitflags::*;

pub const MAX_SIG: usize = 31;

bitflags! {
    pub struct SignalFlags: u32 {
        const SIGDEF = 1; // Default signal handling
        const SIGHUP = 1 << 1;
        const SIGINT = 1 << 2;
        const SIGQUIT = 1 << 3;
        const SIGILL = 1 << 4;
        const SIGTRAP = 1 << 5;
        const SIGABRT = 1 << 6;
        const SIGBUS = 1 << 7;
        const SIGFPE = 1 << 8;
        const SIGKILL = 1 << 9;
        const SIGUSR1 = 1 << 10;
        const SIGSEGV = 1 << 11;
        const SIGUSR2 = 1 << 12;
        const SIGPIPE = 1 << 13;
        const SIGALRM = 1 << 14;
        const SIGTERM = 1 << 15;
        const SIGSTKFLT = 1 << 16;
        const SIGCHLD = 1 << 17;
        const SIGCONT = 1 << 18;
        const SIGSTOP = 1 << 19;
        const SIGTSTP = 1 << 20;
        const SIGTTIN = 1 << 21;
        const SIGTTOU = 1 << 22;
        const SIGURG = 1 << 23;
        const SIGXCPU = 1 << 24;
        const SIGXFSZ = 1 << 25;
        const SIGVTALRM = 1 << 26;
        const SIGPROF = 1 << 27;
        const SIGWINCH = 1 << 28;
        const SIGIO = 1 << 29;
        const SIGPWR = 1 << 30;
        const SIGSYS = 1 << 31;
    }
}

impl SignalFlags {
    pub fn check_error(&self) -> Option<(i32, &'static str)> {
        let errors = vec![
            (Self::SIGINT, -2, "Killed, SIGINT=2"),
            (Self::SIGILL, -4, "Illegal Instruction, SIGILL=4"),
            (Self::SIGABRT, -6, "Aborted, SIGABRT=6"),
            (Self::SIGFPE, -8, "Erroneous Arithmetic Operation, SIGFPE=8"),
            (Self::SIGKILL, -9, "Killed, SIGKILL=9"),
            (Self::SIGSEGV, -11, "Segmentation Fault, SIGSEGV=11"),
        ];
        for (flag, code, msg) in errors {
            if self.contains(flag) {
                return Some((code, msg));
            }
        }
        None
    }
}
