/*!

1. Rust通过命名管道进行进程间通信(IPC)
1. stat,mkfifo,__errno_location,strerror_r等系统调用的详细讲解和错误调试方法
2. nix和标准库是怎么处理FFI调用失败，并解释标准库std::io::Error源码
3. mkfifo, errno命令的使用场景

Rust管道进程间通信源码: https://github.com/pymongo/mkfifo_named_pipe
飞书录像: TODO
b站录像: TODO
YouTube录像: https://youtu.be/O0jOXlkMBLM
*/
#![feature(rustc_private)]
extern crate libc;

#[allow(dead_code)]
const PATH: &str = "/home/w/temp/my_pipe";

/**
## errno文档解读
https://man7.org/linux/man-pages/man3/errno.3.html

> system calls and some library functions in the event of an error to indicate what went wrong
> 
> -1 from most system calls; -1 or NULL from most library functions

大意是一些库或系统调用返回-1或NULL调试调用出错，系统调用通常返回-1表示调用失败，这时候可以找errno查看错误码确定错误原因

> error numbers using the errno(1) command(part of the moreutils package)

补充说明，出错时可以调用`__errno_location()`函数获取最近一次系统调用的错误码

可以用errno命令解读错误码数字的详细含义，也可以用strerror_r将errno转换为错误信息的字符串

> errno is thread-local

## errno错误码示例
```text
[w@ww libc]$ errno 2
ENOENT 2 No such file or directory
```

errno=2: No such file or directory
可能是路径中其中一个文件夹不存在，也可能是**C语言的字符串没加\0作为终止符**
*/
#[allow(dead_code)]
fn my_mkfifo() {
    let path_with_nul = format!("{}\0", PATH);
    if std::path::Path::new(PATH).exists() {
        // or use std::fs::File::metadata(&self)
        let mut file_stat: libc::stat = unsafe {std::mem::zeroed()};
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
        let errno = unsafe { *libc::__errno_location() };
        let err_msg_buf  = [0u8; 64];
        unsafe { libc::strerror_r(errno, err_msg_buf.as_ptr() as _,128) };
        let err_msg_buf_len = err_msg_buf.iter().position(|&x| x == b'\0').unwrap();
        let err_msg = unsafe { String::from_utf8_unchecked(err_msg_buf[..err_msg_buf_len].to_vec()) };
        panic!("system call mkfifo failed, errno={}, err_msg={}", errno, err_msg);
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
