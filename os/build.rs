use std::fs::{read_dir, File};
use std::io::{Result, Write};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../user/src/");
    println!("cargo:rerun-if-changed={}", TARGET_PATH);
    build_user_app();
    insert_app_data().unwrap();
}

fn build_user_app() {
    // 切换到../user项目，编译程序的ELF二进制文件
    Command::new("make")
        .arg("build")
        .current_dir("../user")
        .output()
        .expect("Failed to execute command");
}

static TARGET_PATH: &str = "../user/target/riscv64gc-unknown-none-elf/release/";
// 生成src/link_app.S文件，用于链接用户程序
// 执行cargo build前，会执行本脚本
fn insert_app_data() -> Result<()> {
    let mut f = File::create("src/link_app.S").unwrap();
    let mut apps: Vec<_> = read_dir("../user/src/bin")
        .unwrap()
        .map(|dir_entry| {
            let mut name_with_ext = dir_entry.unwrap().file_name().into_string().unwrap();
            name_with_ext.drain(name_with_ext.find('.').unwrap()..name_with_ext.len());
            name_with_ext
        })
        .collect();
    apps.sort();

    writeln!(
        f,
        r#"
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad {}"#,
        apps.len()
    )?;

    for i in 0..apps.len() {
        writeln!(f, r#"    .quad app_{}_start"#, i)?;
    }
    writeln!(f, r#"    .quad app_{}_end"#, apps.len() - 1)?;

    writeln!(
        f,
        r#"
    /* 我们需要根据应用的名字来加载ELF到进程的地址空间，因此要记录该值。按app的顺序排列 */
    .global _app_names
_app_names:"#
    )?;
    for app in apps.iter() {
        writeln!(f, r#"    .string "{}""#, app)?;
    }

    writeln!(
        f,
        r#"
    /* 要注意：
        - align 3对齐到8字节。我们用的xmas-elf库在解析ELF时，可能不进行对齐。这样能保证不触发内存读写不对齐的异常。
        - 先前使用 incbin "*.bin"，放入裁剪过元数据的ELF二进制。而现在放入完整的ELF二进制。*/"#
    )?;
    for (idx, app) in apps.iter().enumerate() {
        println!("app_{}: {}", idx, app);
        writeln!(
            f,
            r#"    .section .data
    .global app_{0}_start
    .global app_{0}_end
    .align 3
app_{0}_start:
    .incbin "{2}{1}"
app_{0}_end:"#,
            idx, app, TARGET_PATH
        )?;
    }
    Ok(())
}
