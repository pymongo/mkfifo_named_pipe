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

`dircolors --print-database`或`echo $LS_COLORS`可以看到terminal中不同类型文件的颜色-
- socket文件的颜色是Magenta
- pipe/FIFO文件是「橙黄色」
- BLK外设文件「纯黄色」(例如`ls -a /dev/`)
- 可执行文件是绿色
- 软链接是cyan
- 失效的软链接是红色闪烁

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

### ENOENT 2 No such file or directory
可能的错误原因:
- 路径不存在
- 路径字符串不合法: **C语言的字符串没加\0作为终止符**

### ENOMEM 12 Cannot allocate memory
注意标准库的Error没有解析错误码12，所以标准库没有像C语言那样能处理内存分配失败的情况(失败就panic，C一般通过malloc的返回值是否为null处理内存申请失败)

可能的错误原因:
- io_uring not enough lockable memory, please increase memlock config in /etc/security/limits.conf
*/
fn errno_to_err_msg(errno: i32) -> String {
    let err_msg_buf = [0u8; 128];
    unsafe { libc::strerror_r(errno, err_msg_buf.as_ptr() as _, 128) };
    let err_msg_buf_len = err_msg_buf.iter().position(|&x| x == b'\0').unwrap();
    let err_msg = unsafe { String::from_utf8_unchecked(err_msg_buf[..err_msg_buf_len].to_vec()) };
    dbg!(std::io::Error::last_os_error());
    err_msg
}

#[test]
fn test_errno_no_such_file_or_directory() {
    let fd = unsafe { libc::open("/tmp/not_exist_file\0".as_ptr() as _, libc::O_RDONLY) };
    let errno = unsafe { *libc::__errno_location() };
    dbg!(fd, errno_to_err_msg(errno));
}

#[allow(dead_code)]
fn my_mkfifo() {
    let path_with_nul = format!("{}\0", PATH);
    if std::path::Path::new(PATH).exists() {
        // or use std::fs::File::metadata(&self)
        let mut file_stat: libc::stat = unsafe { std::mem::zeroed() };
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
        let err_msg = errno_to_err_msg(errno);
        panic!(
            "system call mkfifo failed, errno={}, err_msg={}",
            errno, err_msg
        );
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
