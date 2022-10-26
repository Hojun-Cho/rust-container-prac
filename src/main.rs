extern crate clap;

use clap::{Arg, App};

mod runtime;

fn main() {
	let args: Vec<&str> = vec!["-c","bash"];
	let _program = args[0].clone();
	let command="/bin/sh";
	let rootfs: &str;

	let mut app = App::new("vas-quod")
		.version("1.0")
		.about("Linux Container runtime")
		.arg(Arg::with_name("rootfs")
			.short("r")
			.long("rootfs")
			.value_name("rootfs")
			.takes_value(true)
		);

	let matches = app.get_matches();

    if let Some(rootfs_v) = matches.value_of("rootfs"){
        rootfs = rootfs_v ;
    }else {
        println!("please enter rootfs");
        return 
    }
runtime::run_container(&rootfs, command, args);
}

