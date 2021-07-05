/*!
2021-06-17_Rust通过命名管道进行进程间通信直播录像:
1. stat,mkfifo,__errno_location,strerror_r等系统调用的详细讲解和错误调试方法
2. nix和标准库是怎么处理FFI调用失败，并解释标准库std::io::Error源码
3. mkfifo, errno命令的使用场景

Rust管道进程间通信源码: https://github.com/pymongo/mkfifo_named_pipe
飞书录像: https://meetings.feishu.cn/minutes/obcnindxxd27o5bm2t2cm6z2?from_source=finish_recording
b站录像: https://www.bilibili.com/video/BV1Lv411W7nX/
YouTube录像: https://youtu.be/O0jOXlkMBLM

说起pipe大家肯定遇到过这样一个报错: `broken pipe`

## data provided contains a nul byte的错误原因

> Error { kind: InvalidInput, message: "data provided contains a nul byte" }

标准库或nix库都会把Path/&str转成CStr，转换后会自动加上\0终止符，所以转换时强制要求File::open的路径字符串不得有\0从而避免重复的\0终止符

## pipe文件颜色

`dircolors --print-database`或`echo $LS_COLORS`可以看到terminal中不同类型文件的颜色

pipe/FIFO文件是「橙黄色」
*/
#![feature(rustc_private)]
extern crate libc;


#[cfg(any(target_os = "freebsd",
                 target_os = "ios",
                 target_os = "macos"))] 
        unsafe fn errno_location() -> *mut libc::c_int {
            libc::__error()
        }
 #[cfg(any(target_os = "android",
                        target_os = "netbsd",
                        target_os = "openbsd"))] 
        unsafe fn errno_location() -> *mut libc::c_int {
            libc::__errno()
        }
 #[cfg(any(target_os = "linux"))] 
        unsafe fn errno_location() -> *mut libc::c_int {
            libc::__errno_location()
        }
#[cfg(any(target_os = "illumos", target_os = "solaris"))] 
        unsafe fn errno_location() -> *mut libc::c_int {
            libc::___errno()
        }

#[allow(dead_code)]
const PATH: &str = "/home/w/temp/my_pipe";

fn errno_to_err_msg(errno: i32) -> String {
    let err_msg_buf  = [0u8; 128];
    unsafe { libc::strerror_r(errno, err_msg_buf.as_ptr() as _,128) };
    let err_msg_buf_len = err_msg_buf.iter().position(|&x| x == b'\0').unwrap();
    let err_msg = unsafe { String::from_utf8_unchecked(err_msg_buf[..err_msg_buf_len].to_vec()) };
    dbg!(std::io::Error::last_os_error());
    dbg!(errno, &err_msg);
    err_msg
}

#[test]
fn test_errno_no_such_file_or_directory() {
    let fd = unsafe { libc::open("/tmp/not_exist_file\0".as_ptr() as _, libc::O_RDONLY) };
    let errno = unsafe { *errno_location() };
    dbg!(fd, errno_to_err_msg(errno));
}

#[allow(dead_code)]
fn my_mkfifo() {
    let path_with_nul = format!("{}\0", PATH);
    if std::path::Path::new(PATH).exists() {
        // or use std::fs::File::metadata(&self)
        let mut file_stat = unsafe {std::mem::zeroed::<libc::stat>()};
        unsafe { libc::stat(path_with_nul.as_ptr() as _, &mut file_stat as *mut _) };
        // S_ISFIFO in /usr/include/sys/stat.h, https://www.gnu.org/software/libc/manual/html_node/Testing-File-Type.html
        // st_mode=4480=0b1000110000000=IS_FIFObit  and other bit
        assert!(file_stat.st_mode & libc::S_IFIFO != 0);
        return;
    }
    // https://users.rust-lang.org/t/named-pipes-in-rust/14721
    // https://docs.rs/nix/0.21.0/nix/unistd/fn.mkfifo.html
    // permission bit: https://www.gnu.org/software/libc/manual/html_node/Permission-Bits.html
    let mkfifo_res = unsafe { libc::mkfifo(PATH.as_ptr() as _, libc::S_IREAD | libc::S_IWRITE) };
    if mkfifo_res == -1 {
        let err_msg = errno_to_err_msg(unsafe { *errno_location() });
        panic!("syscall error = {}", err_msg);
    }
}

/**
## sender_process is refercence to these linux mkfifo command example:
- mkfifo_c_example: https://www.geeksforgeeks.org/named-pipe-fifo-example-c-program/
- mkfifo_command_example: https://www.howtoforge.com/linux-mkfifo-command/
```text
process_1$ mkfifo my_pipe
process_2$ cat < my_pipe
process_1$ echo "hello" > my_pipe
```
sender/receiver process would blocking on open syscall until sender and receiver both connect to pipe, or use non-blocking file open flag
*/
#[test]
fn sender_process() {
    my_mkfifo();
    // Non-Blocking open: std::os::unix::fs::OpenOptionsExt, https://docs.rs/unix-named-pipe/0.2.0/src/unix_named_pipe/lib.rs.html#91
    let mut pipe = std::fs::OpenOptions::new().write(true).open(PATH).unwrap();
    let msg = b"hello\n\0";
    std::io::Write::write_all(&mut pipe, msg).unwrap();
}

#[test]
fn receiver_process() {
    my_mkfifo();
    let mut pipe = std::fs::File::open(PATH).unwrap();
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut pipe, &mut buf).unwrap();
    dbg!(buf);
}
